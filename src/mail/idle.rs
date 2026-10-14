//! IMAP IDLE: the app hears about new mail in each INBOX as it arrives.

use std::time::Duration;

use anyhow::{Context, Result};

use crate::app_state::{lock_or_recover, Account, AppState};
use crate::mail::imap::ImapSession;

/// Watch the INBOX of every account until shutdown, emitting `new-mail` to
/// the UI when something arrives.
pub fn start_all(app: &tauri::AppHandle) {
    use tauri::Manager;
    let state = app.state::<AppState>();
    let accounts = lock_or_recover(&state.accounts).clone();
    for acct in accounts {
        let handle = app.clone();
        let shutdown_rx = state.shutdown_tx.subscribe();
        tauri::async_runtime::spawn(watch_inbox(handle, acct, shutdown_rx));
    }
}

async fn watch_inbox(
    handle: tauri::AppHandle,
    acct: Account,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) {
    let acct_id = acct.id.clone();
    let timeout_secs: u64 = 29 * 60; // 29 minutes per RFC 2177
    loop {
        // Check for shutdown before each cycle
        if shutdown_rx.try_recv().is_ok() {
            tracing::info!("IDLE task for {} received shutdown", acct.email);
            break;
        }

        let password = match crate::app_state::fetch_password(&acct_id).await {
            Ok(pw) => pw,
            Err(e) => {
                tracing::error!("IDLE setup: no password for {}: {}", acct.email, e);
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(120)) => {}
                    _ = shutdown_rx.recv() => { break; }
                }
                continue;
            }
        };
        let session = match super::imap::connect(&acct, &password).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("IDLE: connect failed for {}: {}", acct.email, e);
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                    _ = shutdown_rx.recv() => { break; }
                }
                continue;
            }
        };
        match idle_wait(session, "INBOX", timeout_secs).await {
            Ok((new_mail, mut session)) => {
                if new_mail {
                    tracing::info!("IDLE: new mail for {}", acct.email);
                    use tauri::Emitter;
                    let _ = handle.emit("new-mail", serde_json::json!({
                        "count": 1,
                        "folder": "INBOX",
                        "accountId": acct_id,
                    }));
                }
                let _ = session.logout().await;
            }
            Err(e) => {
                tracing::error!("IDLE: error for {}: {}", acct.email, e);
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(30)) => {}
                    _ = shutdown_rx.recv() => { break; }
                }
            }
        }
    }
}

/// Start an IDLE session on the given folder.
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
