//! IMAP IDLE support for push notifications of new mail.
//!
//! Provides two strategies:
//! - [`idle_wait`]: Uses the IMAP IDLE extension (RFC 2177) for real-time push
//!   notifications. The session is consumed during IDLE and returned afterward.
//! - [`poll_for_new_mail`]: A simpler fallback that compares the mailbox EXISTS
//!   count against a previously known value.

use std::time::Duration;

use anyhow::{Context, Result};

use crate::mail::imap::ImapSession;

/// Start an IDLE session on the given folder.
///
/// This consumes the `ImapSession` because `async-imap` requires ownership
/// for the IDLE handle. The session is returned on success so the caller
/// can continue using it.
///
/// Returns `(new_mail_detected, session)`:
/// - `true` if the server sent new data (e.g. an EXISTS response indicating new mail).
/// - `false` if the wait timed out with no new activity.
///
/// The folder is SELECTed before entering IDLE. If the server does not
/// support IDLE, this will return an error and the caller should fall back
/// to [`poll_for_new_mail`].
pub async fn idle_wait(
    session: ImapSession,
    folder: &str,
    timeout_secs: u64,
) -> Result<(bool, ImapSession)> {
    // We need a mutable session to select, but idle() consumes it.
    let mut session = session;

    session
        .select(folder)
        .await
        .with_context(|| format!("IDLE: failed to select folder: {}", folder))?;

    // idle() consumes the session and returns a Handle.
    let mut idle_handle = session.idle();

    // Send the IDLE command to the server.
    idle_handle
        .init()
        .await
        .context("IDLE: failed to initialize IDLE command")?;

    // Wait for server responses or timeout.
    let duration = Duration::from_secs(timeout_secs);
    let (future, _stop_source) = idle_handle.wait_with_timeout(duration);
    let response = future.await.context("IDLE: error while waiting")?;

    // Determine if new data arrived.
    let new_mail = matches!(
        response,
        async_imap::extensions::idle::IdleResponse::NewData(_)
    );

    // Send DONE to exit IDLE and recover the session.
    let session = idle_handle
        .done()
        .await
        .context("IDLE: failed to send DONE")?;

    Ok((new_mail, session))
}

/// Poll for new mail by comparing the mailbox EXISTS count against a known value.
///
/// This is a simpler fallback for servers that do not support IDLE. It
/// SELECTs the folder and checks the total message count.
///
/// Returns `true` if `mailbox.exists > known_count`, indicating new messages
/// have arrived since the last check.
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
