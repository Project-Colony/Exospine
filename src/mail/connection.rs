//! Connection manager with exponential backoff for IMAP connections.

use anyhow::Result;

use crate::mail::imap::ImapSession;
use crate::state::Account;

/// Connect to an IMAP server with automatic retry and exponential backoff.
///
/// On failure, retries up to `max_retries` times with delays doubling
/// from 2 seconds (2s, 4s, 8s, 16s, ...). Logs each retry attempt
/// via `tracing::warn!`. Returns the last error after all retries
/// are exhausted.
pub async fn connect_with_retry(
    account: &Account,
    password: &str,
    max_retries: u32,
) -> Result<ImapSession> {
    let mut delay_secs = 2u64;

    for attempt in 0..=max_retries {
        match crate::mail::imap::connect(account, password).await {
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

    // Unreachable, but satisfies the compiler.
    unreachable!()
}
