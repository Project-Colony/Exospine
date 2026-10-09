use std::sync::Arc;

use anyhow::{Context, Result};
use futures::TryStreamExt;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use crate::app_state::{Account, MailEntry};

/// Timeout for XOAUTH2 authentication.
const OAUTH_REFRESH_TIMEOUT_SECS: u64 = 15;

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

    use tokio::io::AsyncBufReadExt;
    let mut buf_stream = tokio::io::BufReader::new(tls_stream);
    let mut greeting = String::new();
    buf_stream
        .read_line(&mut greeting)
        .await
        .context("Failed to read IMAP greeting")?;
    tracing::debug!("IMAP server greeting: {}", greeting.trim());
    let tls_stream = buf_stream.into_inner();

    let client = async_imap::Client::new(tls_stream);

    let session = client
        .login(&account.username, password)
        .await
        .map_err(|e| anyhow::anyhow!("Login failed: {:?}", e.0))?;

    Ok(session)
}

/// XOAUTH2 authenticator for use with `async_imap::Client::authenticate`.
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
        self.response.clone()
    }
}

/// Connect to an IMAP server with TLS and authenticate using XOAUTH2.
pub async fn connect_oauth2(account: &Account, access_token: &str) -> Result<ImapSession> {
    tracing::info!(
        "IMAP OAuth2: connecting to {}:{}",
        account.imap_host,
        account.imap_port
    );
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

    use tokio::io::AsyncBufReadExt;
    let mut buf_stream = tokio::io::BufReader::new(tls_stream);
    let mut greeting = String::new();
    buf_stream
        .read_line(&mut greeting)
        .await
        .context("Failed to read IMAP greeting")?;
    tracing::info!("IMAP OAuth2: server greeting: {}", greeting.trim());
    let tls_stream = buf_stream.into_inner();

    let client = async_imap::Client::new(tls_stream);

    let authenticator = XOAuth2Authenticator::new(&account.username, access_token);

    let session = tokio::time::timeout(
        std::time::Duration::from_secs(OAUTH_REFRESH_TIMEOUT_SECS),
        client.authenticate("XOAUTH2", authenticator),
    )
    .await
    .map_err(|_| anyhow::anyhow!("XOAUTH2 authentication timed out after 15s"))?
    .map_err(|e| anyhow::anyhow!("XOAUTH2 login failed: {:?}", e.0))?;

    tracing::info!("IMAP OAuth2: authenticated successfully");
    Ok(session)
}

/// Extract is_read (Seen) and is_starred (Flagged) from IMAP flags.
fn extract_mail_flags(flags: &[async_imap::types::Flag<'_>]) -> (bool, bool) {
    let is_read = flags.iter().any(|f| matches!(f, async_imap::types::Flag::Seen));
    let is_starred = flags.iter().any(|f| matches!(f, async_imap::types::Flag::Flagged));
    (is_read, is_starred)
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
                        entry.uid = uid;
                    }
                    let flags: Vec<_> = msg.flags().collect();
                    let (is_read, is_starred) = extract_mail_flags(&flags);
                    entry.is_read = is_read;
                    entry.is_starred = is_starred;
                    entries.push(entry);
                }
                Err(e) => {
                    tracing::warn!("Failed to parse message: {}", e);
                }
            }
        }
    }

    entries.reverse();
    Ok(entries)
}

/// Fetch new messages with UID greater than `since_uid` (delta sync).
pub async fn fetch_new_mails(
    session: &mut ImapSession,
    folder: &str,
    account_id: &str,
    since_uid: u32,
) -> Result<Vec<MailEntry>> {
    session
        .select(folder)
        .await
        .with_context(|| format!("Failed to select folder: {}", folder))?;

    let search_query = format!("UID {}:*", since_uid + 1);
    let uids: Vec<u32> = session
        .uid_search(&search_query)
        .await
        .context("UID SEARCH failed")?
        .into_iter()
        .collect();

    if uids.is_empty() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for chunk in uids.chunks(100) {
        let range = chunk
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let messages: Vec<_> = session
            .uid_fetch(&range, "(UID FLAGS ENVELOPE BODY.PEEK[])")
            .await
            .context("Failed to UID FETCH")?
            .try_collect::<Vec<_>>()
            .await
            .context("Failed to collect UID FETCH")?;

        for msg in &messages {
            if let Some(body) = msg.body() {
                match crate::mail::parser::parse_email(body, account_id, folder) {
                    Ok(mut entry) => {
                        if let Some(uid) = msg.uid {
                            entry.id = uid.to_string();
                            entry.uid = uid;
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
                        tracing::warn!("Failed to parse delta message: {}", e);
                    }
                }
            }
        }
    }

    entries.sort_by_key(|a| std::cmp::Reverse(a.date));
    Ok(entries)
}

/// Fetch ALL messages in a folder in batches (for initial full sync).
pub async fn fetch_all_mails_batched(
    session: &mut ImapSession,
    folder: &str,
    account_id: &str,
) -> Result<Vec<MailEntry>> {
    let mailbox = session
        .select(folder)
        .await
        .with_context(|| format!("Failed to select folder: {}", folder))?;

    let total = mailbox.exists;
    if total == 0 {
        return Ok(Vec::new());
    }

    let all_uids: Vec<u32> = session
        .uid_search("ALL")
        .await
        .context("UID SEARCH ALL failed")?
        .into_iter()
        .collect();

    if all_uids.is_empty() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for chunk in all_uids.chunks(100) {
        let range = chunk
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let messages: Vec<_> = session
            .uid_fetch(&range, "(UID FLAGS ENVELOPE BODY.PEEK[])")
            .await
            .context("Failed to UID FETCH batch")?
            .try_collect::<Vec<_>>()
            .await
            .context("Failed to collect UID FETCH batch")?;

        for msg in &messages {
            if let Some(body) = msg.body() {
                match crate::mail::parser::parse_email(body, account_id, folder) {
                    Ok(mut entry) => {
                        if let Some(uid) = msg.uid {
                            entry.id = uid.to_string();
                            entry.uid = uid;
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
                        tracing::warn!("Failed to parse batch message: {}", e);
                    }
                }
            }
        }
    }

    entries.sort_by_key(|a| std::cmp::Reverse(a.date));
    Ok(entries)
}

/// Fetch all UIDs in a folder.
pub async fn fetch_uids_for_folder(session: &mut ImapSession, folder: &str) -> Result<Vec<u32>> {
    session
        .select(folder)
        .await
        .with_context(|| format!("Failed to select folder: {}", folder))?;

    let uids: Vec<u32> = session
        .uid_search("ALL")
        .await
        .context("UID SEARCH ALL failed")?
        .into_iter()
        .collect();

    Ok(uids)
}

/// Fetch a specific set of UIDs and return parsed mail entries.
pub async fn fetch_batch_by_uids(
    session: &mut ImapSession,
    uids: &[u32],
    account_id: &str,
    folder: &str,
) -> Result<Vec<MailEntry>> {
    if uids.is_empty() {
        return Ok(Vec::new());
    }

    let range = uids
        .iter()
        .map(|u| u.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let messages: Vec<_> = session
        .uid_fetch(&range, "(UID FLAGS ENVELOPE BODY.PEEK[])")
        .await
        .context("Failed to UID FETCH batch")?
        .try_collect::<Vec<_>>()
        .await
        .context("Failed to collect UID FETCH batch")?;

    let mut entries = Vec::new();
    for msg in &messages {
        if let Some(body) = msg.body() {
            match crate::mail::parser::parse_email(body, account_id, folder) {
                Ok(mut entry) => {
                    if let Some(uid) = msg.uid {
                        entry.id = uid.to_string();
                        entry.uid = uid;
                    }
                    let flags: Vec<_> = msg.flags().collect();
                    let (is_read, is_starred) = extract_mail_flags(&flags);
                    entry.is_read = is_read;
                    entry.is_starred = is_starred;
                    entries.push(entry);
                }
                Err(e) => {
                    tracing::warn!("Failed to parse batch message: {}", e);
                }
            }
        }
    }

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

fn root_store() -> rustls::RootCertStore {
    let mut store = rustls::RootCertStore::empty();
    let native = rustls_native_certs::load_native_certs();
    if native.certs.is_empty() {
        store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    } else {
        for cert in native.certs {
            let _ = store.add(cert);
        }
    }
    store
}

