use std::sync::Arc;

use anyhow::{Context, Result};
use futures::TryStreamExt;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use crate::state::{Account, Folder, FolderType, MailEntry};

/// A connected IMAP session over TLS.
pub type ImapSession = async_imap::Session<tokio_rustls::client::TlsStream<TcpStream>>;

/// Connect to an IMAP server with TLS and authenticate.
pub async fn connect(account: &Account, password: &str) -> Result<ImapSession> {
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store())
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(tls_config));

    let addr = format!("{}:{}", account.imap_host, account.imap_port);
    let tcp = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("Failed to connect to {}", addr))?;

    let server_name: rustls::pki_types::ServerName<'static> = account
        .imap_host
        .clone()
        .try_into()
        .context("Invalid server name")?;
    let tls_stream = connector
        .connect(server_name, tcp)
        .await
        .context("TLS handshake failed")?;

    let client = async_imap::Client::new(tls_stream);

    let session = client
        .login(&account.username, password)
        .await
        .map_err(|e| anyhow::anyhow!("Login failed: {:?}", e.0))?;

    Ok(session)
}

/// XOAUTH2 authenticator for use with `async_imap::Client::authenticate`.
///
/// Implements the SASL XOAUTH2 mechanism: on the first (empty) server
/// challenge it returns the pre-built XOAUTH2 token string.
struct XOAuth2Authenticator {
    response: Vec<u8>,
}

impl XOAuth2Authenticator {
    fn new(user: &str, access_token: &str) -> Self {
        let raw = format!("user={}\x01auth=Bearer {}\x01\x01", user, access_token);
        Self {
            response: raw.into_bytes(),
        }
    }
}

impl async_imap::Authenticator for XOAuth2Authenticator {
    type Response = Vec<u8>;

    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        // Return the XOAUTH2 SASL initial response.
        self.response.clone()
    }
}

/// Connect to an IMAP server with TLS and authenticate using XOAUTH2.
///
/// Use this instead of [`connect`] for accounts that use OAuth2.
pub async fn connect_oauth2(account: &Account, access_token: &str) -> Result<ImapSession> {
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store())
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(tls_config));

    let addr = format!("{}:{}", account.imap_host, account.imap_port);
    let tcp = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("Failed to connect to {}", addr))?;

    let server_name: rustls::pki_types::ServerName<'static> = account
        .imap_host
        .clone()
        .try_into()
        .context("Invalid server name")?;
    let tls_stream = connector
        .connect(server_name, tcp)
        .await
        .context("TLS handshake failed")?;

    let client = async_imap::Client::new(tls_stream);

    let authenticator = XOAuth2Authenticator::new(&account.username, access_token);

    let session = client
        .authenticate("XOAUTH2", authenticator)
        .await
        .map_err(|e| anyhow::anyhow!("XOAUTH2 login failed: {:?}", e.0))?;

    Ok(session)
}

/// Fetch the list of folders / mailboxes for the account.
pub async fn fetch_folders(session: &mut ImapSession) -> Result<Vec<Folder>> {
    let mailboxes: Vec<_> = session
        .list(None, Some("*"))
        .await
        .context("Failed to list mailboxes")?
        .try_collect::<Vec<_>>()
        .await
        .context("Failed to collect mailboxes")?;

    let mut folders = Vec::new();
    for mb in &mailboxes {
        let name = mb.name().to_string();
        let folder_type = classify_folder(&name);
        folders.push(Folder {
            name,
            unread_count: 0,
            total_count: 0,
            folder_type,
        });
    }

    Ok(folders)
}

/// Fetch recent messages from a given folder.
pub async fn fetch_messages(
    session: &mut ImapSession,
    folder: &str,
    limit: u32,
) -> Result<Vec<MailEntry>> {
    let mailbox = session
        .select(folder)
        .await
        .with_context(|| format!("Failed to select folder: {}", folder))?;

    let total = mailbox.exists;
    if total == 0 {
        return Ok(Vec::new());
    }

    let start = if total > limit { total - limit + 1 } else { 1 };
    let range = format!("{}:{}", start, total);

    let messages: Vec<_> = session
        .fetch(&range, "(UID FLAGS ENVELOPE BODY.PEEK[])")
        .await
        .context("Failed to fetch messages")?
        .try_collect::<Vec<_>>()
        .await
        .context("Failed to collect messages")?;

    let mut entries = Vec::new();
    for msg in &messages {
        if let Some(body) = msg.body() {
            match crate::mail::parser::parse_email(body, "", folder) {
                Ok(mut entry) => {
                    if let Some(uid) = msg.uid {
                        entry.id = uid.to_string();
                    }
                    // Check flags for \Seen
                    let flags: Vec<_> = msg.flags().collect();
                    entry.is_read = flags
                        .iter()
                        .any(|f| matches!(f, async_imap::types::Flag::Seen));
                    entry.is_starred = flags
                        .iter()
                        .any(|f| matches!(f, async_imap::types::Flag::Flagged));
                    entries.push(entry);
                }
                Err(e) => {
                    tracing::warn!("Failed to parse message: {}", e);
                }
            }
        }
    }

    entries.reverse(); // newest first
    Ok(entries)
}

/// Mark a message as read (add \Seen flag).
pub async fn mark_as_read(session: &mut ImapSession, uid: &str) -> Result<()> {
    session
        .uid_store(uid, "+FLAGS (\\Seen)")
        .await
        .context("Failed to mark as read")?
        .try_collect::<Vec<_>>()
        .await
        .context("Failed to collect store response")?;
    Ok(())
}

/// Delete a message (add \Deleted flag and expunge).
pub async fn delete_message(session: &mut ImapSession, uid: &str) -> Result<()> {
    session
        .uid_store(uid, "+FLAGS (\\Deleted)")
        .await
        .context("Failed to mark for deletion")?
        .try_collect::<Vec<_>>()
        .await
        .context("Failed to collect store response")?;
    session
        .expunge()
        .await
        .context("Failed to expunge")?
        .try_collect::<Vec<_>>()
        .await
        .context("Failed to collect expunge response")?;
    Ok(())
}

/// Mark a message as unread (remove \Seen flag).
pub async fn mark_as_unread(session: &mut ImapSession, uid: &str) -> Result<()> {
    session
        .uid_store(uid, "-FLAGS (\\Seen)")
        .await
        .context("Failed to mark as unread")?
        .try_collect::<Vec<_>>()
        .await
        .context("Failed to collect store response")?;
    Ok(())
}

/// Add the \Flagged flag (star) to a message.
pub async fn set_flagged(session: &mut ImapSession, uid: &str) -> Result<()> {
    session
        .uid_store(uid, "+FLAGS (\\Flagged)")
        .await
        .context("Failed to set flagged")?
        .try_collect::<Vec<_>>()
        .await
        .context("Failed to collect store response")?;
    Ok(())
}

/// Remove the \Flagged flag (un-star) from a message.
pub async fn remove_flagged(session: &mut ImapSession, uid: &str) -> Result<()> {
    session
        .uid_store(uid, "-FLAGS (\\Flagged)")
        .await
        .context("Failed to remove flagged")?
        .try_collect::<Vec<_>>()
        .await
        .context("Failed to collect store response")?;
    Ok(())
}

/// Append a raw RFC-822 message to a folder (used for saving drafts).
pub async fn append_message(
    session: &mut ImapSession,
    folder: &str,
    message: &[u8],
) -> Result<()> {
    session
        .append(folder, None, None, message)
        .await
        .context("Failed to append message")?;
    Ok(())
}

/// Move a message to a different folder by copying then deleting.
pub async fn move_message(session: &mut ImapSession, uid: &str, target: &str) -> Result<()> {
    session
        .uid_copy(uid, target)
        .await
        .context("Failed to copy message")?;
    delete_message(session, uid).await?;
    Ok(())
}

/// Search messages on the server using IMAP SEARCH.
///
/// Builds an IMAP SEARCH query that matches `query` against FROM, SUBJECT,
/// and BODY fields, then fetches the matching messages (up to `limit`).
pub async fn search_messages(
    session: &mut ImapSession,
    folder: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<MailEntry>> {
    session
        .select(folder)
        .await
        .with_context(|| format!("Failed to select folder: {}", folder))?;

    // Build IMAP SEARCH criteria: OR FROM "query" OR SUBJECT "query" BODY "query"
    let escaped = query.replace('\\', "\\\\").replace('"', "\\\"");
    let query_string = format!(
        "OR FROM \"{}\" OR SUBJECT \"{}\" BODY \"{}\"",
        escaped, escaped, escaped
    );

    let search_results: Vec<_> = session
        .search(&query_string)
        .await
        .with_context(|| format!("IMAP SEARCH failed for query: {}", query))?
        .into_iter()
        .collect();

    if search_results.is_empty() {
        return Ok(Vec::new());
    }

    // Take only the last `limit` sequence numbers (most recent).
    let results: Vec<u32> = if search_results.len() as u32 > limit {
        search_results[search_results.len() - limit as usize..].to_vec()
    } else {
        search_results
    };

    // Build a comma-separated list of sequence numbers for FETCH.
    let range = results
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let messages: Vec<_> = session
        .fetch(&range, "(UID FLAGS ENVELOPE BODY.PEEK[])")
        .await
        .context("Failed to fetch search results")?
        .try_collect::<Vec<_>>()
        .await
        .context("Failed to collect search results")?;

    let mut entries = Vec::new();
    for msg in &messages {
        if let Some(body) = msg.body() {
            match crate::mail::parser::parse_email(body, "", folder) {
                Ok(mut entry) => {
                    if let Some(uid) = msg.uid {
                        entry.id = uid.to_string();
                    }
                    let flags: Vec<_> = msg.flags().collect();
                    entry.is_read = flags
                        .iter()
                        .any(|f| matches!(f, async_imap::types::Flag::Seen));
                    entry.is_starred = flags
                        .iter()
                        .any(|f| matches!(f, async_imap::types::Flag::Flagged));
                    entries.push(entry);
                }
                Err(e) => {
                    tracing::warn!("Failed to parse search result message: {}", e);
                }
            }
        }
    }

    entries.reverse(); // newest first
    Ok(entries)
}

fn root_store() -> rustls::RootCertStore {
    let mut store = rustls::RootCertStore::empty();
    store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    store
}

fn classify_folder(name: &str) -> FolderType {
    let lower = name.to_lowercase();
    if lower == "inbox" {
        FolderType::Inbox
    } else if lower.contains("sent") {
        FolderType::Sent
    } else if lower.contains("draft") {
        FolderType::Drafts
    } else if lower.contains("trash") || lower.contains("deleted") {
        FolderType::Trash
    } else if lower.contains("starred") || lower.contains("flagged") {
        FolderType::Starred
    } else {
        FolderType::Custom
    }
}
