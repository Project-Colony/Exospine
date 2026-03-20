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

/// Move a message to a different folder by copying then deleting.
pub async fn move_message(session: &mut ImapSession, uid: &str, target: &str) -> Result<()> {
    session
        .uid_copy(uid, target)
        .await
        .context("Failed to copy message")?;
    delete_message(session, uid).await?;
    Ok(())
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
