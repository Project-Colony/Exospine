//! Connection pool and manager for IMAP sessions.
//!
//! Maintains a global pool of live IMAP sessions keyed by account_id so that
//! consecutive operations reuse an existing TLS connection instead of paying
//! the 3-5s handshake cost every time.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use anyhow::Result;

use crate::app_state::Account;
use crate::mail::imap::ImapSession;

/// Pooled session entry: the IMAP session plus the currently SELECT'd folder (if any).
struct PoolEntry {
    session: ImapSession,
    selected_folder: Option<String>,
}

/// Global IMAP session pool: account_id -> PoolEntry.
static POOL: LazyLock<Mutex<HashMap<String, PoolEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Obtain an IMAP session for the given account.
///
/// If the pool contains a session for this account, it is removed from the pool
/// and a lightweight NOOP is issued to verify the connection is still alive.
/// If the NOOP fails the stale session is dropped and a fresh one is created.
///
/// When the caller is done with the session, it should call [`return_session`]
/// instead of logging out.
#[allow(dead_code)]
pub async fn get_session(
    account_id: &str,
    account: &Account,
    password: &str,
) -> Result<ImapSession> {
    // Try to grab a pooled session
    let pooled = {
        let mut pool = crate::app_state::lock_or_recover(&POOL);
        pool.remove(account_id)
    };

    if let Some(mut entry) = pooled {
        // Verify the connection is still alive with a NOOP
        match entry.session.noop().await {
            Ok(_) => return Ok(entry.session),
            Err(e) => {
                tracing::debug!(
                    account = %account.email,
                    error = %e,
                    "Pooled IMAP session stale, creating new one"
                );
                // Session is dead — drop it and fall through to create a new one
            }
        }
    }

    // No pooled session (or it was stale) — create a fresh one with retry
    connect_with_retry(account, password, 3).await
}

/// Obtain an IMAP session with the given folder already SELECT'd.
///
/// If the pool contains a session that already has this folder selected, the
/// SELECT command is skipped entirely (~500ms saved). Otherwise SELECT is issued.
#[allow(dead_code)]
pub async fn get_session_for_folder(
    account_id: &str,
    account: &Account,
    password: &str,
    folder: &str,
) -> Result<ImapSession> {
    // Try to grab a pooled session
    let pooled = {
        let mut pool = crate::app_state::lock_or_recover(&POOL);
        pool.remove(account_id)
    };

    if let Some(mut entry) = pooled {
        // Verify the connection is still alive with a NOOP
        match entry.session.noop().await {
            Ok(_) => {
                if entry.selected_folder.as_deref() == Some(folder) {
                    // Already SELECT'd on this folder — skip the SELECT command
                    tracing::debug!(
                        account = %account.email,
                        folder = %folder,
                        "Reusing pooled session with folder already selected"
                    );
                    return Ok(entry.session);
                }
                // Different folder (or none) — SELECT it
                entry.session.select(folder).await?;
                return Ok(entry.session);
            }
            Err(e) => {
                tracing::debug!(
                    account = %account.email,
                    error = %e,
                    "Pooled IMAP session stale, creating new one"
                );
            }
        }
    }

    // No pooled session (or it was stale) — create a fresh one + SELECT
    let mut session = connect_with_retry(account, password, 3).await?;
    session.select(folder).await?;
    Ok(session)
}

/// Return an IMAP session to the pool for later reuse.
///
/// If returning fails for any reason the session is silently dropped.
#[allow(dead_code)]
pub fn return_session(account_id: &str, session: ImapSession) {
    let mut pool = crate::app_state::lock_or_recover(&POOL);
    pool.insert(account_id.to_owned(), PoolEntry {
        session,
        selected_folder: None,
    });
}

/// Return an IMAP session to the pool, remembering which folder is SELECT'd.
#[allow(dead_code)]
pub fn return_session_with_folder(account_id: &str, session: ImapSession, folder: String) {
    let mut pool = crate::app_state::lock_or_recover(&POOL);
    pool.insert(account_id.to_owned(), PoolEntry {
        session,
        selected_folder: Some(folder),
    });
}

/// Remove and logout all pooled sessions (e.g. when an account is removed).
#[allow(dead_code)]
pub async fn evict_session(account_id: &str) {
    let entry = {
        let mut pool = crate::app_state::lock_or_recover(&POOL);
        pool.remove(account_id)
    };
    if let Some(mut e) = entry {
        let _ = e.session.logout().await;
    }
}

/// Connect to an IMAP server with automatic retry and exponential backoff.
pub async fn connect_with_retry(
    account: &Account,
    password: &str,
    max_retries: u32,
) -> Result<ImapSession> {
    let use_oauth2 = matches!(account.auth_method, crate::app_state::AuthMethod::OAuth2 { .. });
    let mut delay_secs = 2u64;

    for attempt in 0..=max_retries {
        let result = if use_oauth2 {
            crate::mail::imap::connect_oauth2(account, password).await
        } else {
            crate::mail::imap::connect(account, password).await
        };
        match result {
            Ok(session) => return Ok(session),
            Err(e) => {
                if attempt == max_retries {
                    return Err(e);
                }
                tracing::warn!(
                    attempt = attempt + 1,
                    max_retries,
                    delay_secs,
                    error = %e,
                    account = %account.email,
                    "IMAP connection failed, retrying in {}s",
                    delay_secs,
                );
                tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                delay_secs *= 2;
            }
        }
    }

    unreachable!()
}
