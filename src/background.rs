//! Housekeeping that runs for as long as the app does: the unread count in the
//! window title, and the weekly VACUUM of the mail database.

use std::time::Duration;

use tauri::Manager;

use crate::app_state::{lock_or_recover, AppState};

pub fn start(app: &tauri::AppHandle) {
    let shutdown_tx = &app.state::<AppState>().shutdown_tx;
    tauri::async_runtime::spawn(update_title(app.clone(), shutdown_tx.subscribe()));
    tauri::async_runtime::spawn(vacuum_weekly(app.clone(), shutdown_tx.subscribe()));
}

/// Show the unread count in the main window's title, every 15 seconds.
async fn update_title(app: tauri::AppHandle, mut shutdown_rx: tokio::sync::broadcast::Receiver<()>) {
    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(15)) => {}
            _ = shutdown_rx.recv() => {
                tracing::info!("Title updater: shutdown received");
                break;
            }
        }
        let count = {
            let state = app.state::<AppState>();
            let db_guard = lock_or_recover(&state.db);
            db_guard.as_ref().map_or(0, |db| db.count_unread().unwrap_or(0))
        };
        if let Some(window) = app.get_webview_window("main") {
            let title = match count {
                0 => "Exospine".to_string(),
                1 => crate::i18n::tr("Exospine (1 unread)", "Exospine (1 non lu)").to_string(),
                n => crate::i18n::tr("Exospine ({count} unread)", "Exospine ({count} non lus)")
                    .replace("{count}", &n.to_string()),
            };
            let _ = window.set_title(&title);
        }
    }
}

/// Reclaim disk space once a week: checked at startup, then every 6 hours.
async fn vacuum_weekly(app: tauri::AppHandle, mut shutdown_rx: tokio::sync::broadcast::Receiver<()>) {
    loop {
        {
            let state = app.state::<AppState>();
            let db_guard = lock_or_recover(&state.db);
            if let Some(ref db) = *db_guard {
                if db.should_vacuum() {
                    tracing::info!("Running weekly VACUUM...");
                    match db.vacuum() {
                        Ok(()) => tracing::info!("VACUUM completed successfully"),
                        Err(e) => tracing::error!("VACUUM failed: {}", e),
                    }
                }
            }
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(6 * 3600)) => {}
            _ = shutdown_rx.recv() => {
                tracing::info!("VACUUM task: shutdown received");
                break;
            }
        }
    }
}
