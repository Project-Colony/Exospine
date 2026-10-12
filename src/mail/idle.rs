//! IMAP IDLE support for push notifications of new mail.
//!
//! These functions are kept for future use when background mail checking
//! is wired into the application.

use std::time::Duration;

use anyhow::{Context, Result};

use crate::mail::imap::ImapSession;

/// Start an IDLE session on the given folder.
#[allow(dead_code)]
pub async fn idle_wait(
    session: ImapSession,
    folder: &str,
    timeout_secs: u64,
) -> Result<(bool, ImapSession)> {
    let mut session = session;

    session
        .select(folder)
        .await
        .with_context(|| format!("IDLE: failed to select folder: {}", folder))?;

    let mut idle_handle = session.idle();

    idle_handle
        .init()
        .await
        .context("IDLE: failed to initialize IDLE command")?;

    let duration = Duration::from_secs(timeout_secs);
    let (future, _stop_source) = idle_handle.wait_with_timeout(duration);
    let response = future.await.context("IDLE: error while waiting")?;

    let new_mail = matches!(
        response,
        async_imap::extensions::idle::IdleResponse::NewData(_)
    );

    let session = idle_handle
        .done()
        .await
        .context("IDLE: failed to send DONE")?;

    Ok((new_mail, session))
}

/// Poll for new mail by comparing the mailbox EXISTS count.
#[allow(dead_code)]
pub async fn poll_for_new_mail(
    session: &mut ImapSession,
    folder: &str,
    known_count: u32,
) -> Result<bool> {
    let mailbox = session
        .select(folder)
        .await
        .with_context(|| format!("Poll: failed to select folder: {}", folder))?;

    Ok(mailbox.exists > known_count)
}
