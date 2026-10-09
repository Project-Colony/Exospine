// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

mod accounts;
mod app_state;
pub mod auth;
mod commands;
mod config;
mod mail;
pub mod notifications;
mod search;
mod storage;
pub mod url_handlers;
pub mod window_state;

use app_state::{Account, AppSettings};
use config::Config;
use storage::db::Database;
use url_handlers::{parse_exospine_url, parse_mailto, MailtoData, PENDING_MAILTO};
use window_state::{restore_window_state, save_window_state};

/// Global Tauri-managed application state.
pub struct AppState {
    pub accounts: Mutex<Vec<Account>>,
    pub db: Mutex<Option<Database>>,
    pub config: Mutex<Config>,
    pub settings: Mutex<AppSettings>,
    /// Broadcast channel for graceful shutdown signaling.
    pub shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

/// Tauri command: get pending mailto data (if launched via mailto: link).
#[tauri::command]
fn get_pending_mailto() -> Option<MailtoData> {
    let mut guard = PENDING_MAILTO.lock().unwrap_or_else(|e| e.into_inner());
    guard.take()
}

fn main() {
    // Check if launched with a mailto: or exospine:// argument
    for arg in std::env::args().skip(1) {
        if arg.starts_with("mailto:") {
            let data = parse_mailto(&arg);
            tracing::info!("Launched with mailto: to={}", data.to);
            let mut guard = PENDING_MAILTO.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(data);
            break;
        } else if arg.starts_with("exospine://") {
            let data = parse_exospine_url(&arg);
            tracing::info!("Launched with exospine:// to={}", data.to);
            let mut guard = PENDING_MAILTO.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(data);
            break;
        }
    }

    // Install rustls CryptoProvider for IMAP TLS connections
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Load config from disk
    let config = Config::load();

    // Build initial settings from config
    let settings = AppSettings {
        theme: config.theme.clone(),
        font_size: config.font_size,
        check_interval_secs: config.check_interval_secs,
        show_notifications: config.show_notifications,
        reading_pane: config.reading_pane.clone(),
        density: config.density.clone(),
        language: config.language.clone(),
    };

    // Load persisted accounts
    let accounts = config::load_accounts();

    // Restore credentials from keyring into in-memory store
    for acct in &accounts {
        if let Ok(pw) = crate::accounts::keyring_store::get_password(&acct.id) {
            app_state::store_credential(&acct.id, &pw);
        }
    }

    // OAuth2 token refresh data: collected here, spawned in .setup() to avoid blocking startup.
    let oauth_refresh_tasks: Vec<_> = accounts
        .iter()
        .filter_map(|acct| {
            if let app_state::AuthMethod::OAuth2 { ref refresh_token } = acct.auth_method {
                let oauth_cfg = accounts::provider::oauth2_config_for_account(acct, &config)?;
                Some((acct.id.clone(), acct.email.clone(), refresh_token.clone(), oauth_cfg))
            } else {
                None
            }
        })
        .collect();

    // Open SQLite database
    let db = Database::open().ok();

    // Create shutdown broadcast channel
    let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);

    let state = AppState {
        accounts: Mutex::new(accounts),
        db: Mutex::new(db),
        config: Mutex::new(config),
        settings: Mutex::new(settings),
        shutdown_tx: shutdown_tx.clone(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .setup(move |app| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(debug_assertions)]
                window.open_devtools();
                // Restore window position/size from saved state
                restore_window_state(&window);
                // Save window state on close and trigger graceful shutdown
                let win = window.clone();
                let shutdown = shutdown_tx.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        save_window_state(&win);
                        // Pop-out message windows close with the main window,
                        // so the app does not keep running without it.
                        for (label, popout) in win.webview_windows() {
                            if label.starts_with("mail-") {
                                let _ = popout.close();
                            }
                        }
                        // Signal all background tasks to stop
                        let _ = shutdown.send(());
                        // Brief grace period for background tasks
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                });
            }

            // Refresh OAuth2 tokens in background (non-blocking)
            for (acct_id, acct_email, rt_token, oauth_cfg) in oauth_refresh_tasks {
                tauri::async_runtime::spawn(async move {
                    match crate::accounts::oauth2::refresh_token(&oauth_cfg, &rt_token).await {
                        Ok(token_resp) => {
                            tracing::info!(
                                "Refreshed OAuth2 token for account {} (access_token len={})",
                                acct_email,
                                token_resp.access_token.len()
                            );
                            app_state::store_credential(&acct_id, &token_resp.access_token);
                            if let Some(ref new_rt) = token_resp.refresh_token {
                                if *new_rt != rt_token {
                                    tracing::info!(
                                        "OAuth2 refresh token rotated for account {}",
                                        acct_email
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to refresh OAuth2 token for account {}: {}",
                                acct_email,
                                e
                            );
                        }
                    }
                });
            }

            // Periodically update window title with unread count
            {
                let app_handle = app.handle().clone();
                let mut shutdown_rx = shutdown_tx.subscribe();
                tauri::async_runtime::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = tokio::time::sleep(std::time::Duration::from_secs(15)) => {}
                            _ = shutdown_rx.recv() => {
                                tracing::info!("Title updater: shutdown received");
                                break;
                            }
                        }
                        let count = {
                            use tauri::Manager;
                            let state: tauri::State<'_, AppState> = app_handle.state();
                            let db_guard = app_state::lock_or_recover(&state.db);
                            if let Some(ref db) = *db_guard {
                                db.count_unread().unwrap_or(0)
                            } else {
                                0
                            }
                        };
                        use tauri::Manager;
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let title = if count > 0 {
                                format!("Exospine ({} unread)", count)
                            } else {
                                "Exospine".to_string()
                            };
                            let _ = window.set_title(&title);
                        }
                    }
                });
            }

            // Periodic VACUUM: run once per week to reclaim disk space
            {
                let app_handle2 = app.handle().clone();
                let mut shutdown_rx2 = shutdown_tx.subscribe();
                tauri::async_runtime::spawn(async move {
                    // Check on startup, then every 6 hours
                    loop {
                        {
                            use tauri::Manager;
                            let state: tauri::State<'_, AppState> = app_handle2.state();
                            let db_guard = app_state::lock_or_recover(&state.db);
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
                            _ = tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)) => {}
                            _ = shutdown_rx2.recv() => {
                                tracing::info!("VACUUM task: shutdown received");
                                break;
                            }
                        }
                    }
                });
            }

            // Start IMAP IDLE for each account in the background
            let app_handle = app.handle().clone();
            let idle_accounts: Vec<crate::app_state::Account> = {
                let state: tauri::State<'_, AppState> = app.state();
                let accts = app_state::lock_or_recover(&state.accounts).clone();
                accts
            };
            for acct in idle_accounts {
                let handle = app_handle.clone();
                let acct_id = acct.id.clone();
                let mut shutdown_rx = shutdown_tx.subscribe();
                tauri::async_runtime::spawn(async move {
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
                                    _ = tokio::time::sleep(std::time::Duration::from_secs(120)) => {}
                                    _ = shutdown_rx.recv() => { break; }
                                }
                                continue;
                            }
                        };
                        let session = match crate::mail::imap::connect(&acct, &password).await {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::error!("IDLE: connect failed for {}: {}", acct.email, e);
                                tokio::select! {
                                    _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
                                    _ = shutdown_rx.recv() => { break; }
                                }
                                continue;
                            }
                        };
                        match crate::mail::idle::idle_wait(session, "INBOX", timeout_secs).await {
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
                                    _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {}
                                    _ = shutdown_rx.recv() => { break; }
                                }
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Account commands
            commands::accounts::get_accounts,
            commands::accounts::detect_provider,
            commands::accounts::start_oauth,
            commands::accounts::add_oauth_account,
            commands::accounts::add_account,
            commands::accounts::remove_account,
            commands::accounts::test_connection,
            // Mail commands — folders
            commands::mail_folders::get_folders,
            // Mail commands — reading
            commands::mail_read::get_mails,
            commands::mail_read::get_mail_body,
            commands::mail_read::get_mail_headers,
            commands::mail_read::open_eml_file,
            commands::mail_read::open_mail_window,
            // Mail commands — sync
            commands::mail_sync::refresh_folder,
            commands::mail_sync::sync_all_mails,
            commands::mail_sync::start_idle,
            // Mail commands — flags
            commands::mail_flags::mark_read,
            commands::mail_flags::mark_unread,
            commands::mail_flags::toggle_star,
            commands::mail_flags::delete_mail,
            commands::mail_flags::archive_mail,
            // Mail commands — move
            commands::mail_move::move_mail,
            commands::mail_move::sweep_sender,
            // Mail commands — search
            commands::mail_search::search_local,
            // Mail commands — utils
            commands::mail_utils::download_attachment,
            commands::mail_utils::export_mail,
            commands::mail_utils::get_security_log,
            commands::mail_utils::add_category,
            commands::mail_utils::remove_category,
            commands::mail_utils::report_spam,
            commands::mail_utils::report_not_spam,
            commands::mail_utils::get_analytics,
            commands::mail_utils::cleanup_old_messages,
            commands::mail_utils::save_note,
            commands::mail_utils::get_note,
            commands::mail_utils::find_duplicates,
            // Compose commands
            commands::compose::send_mail,
            commands::compose::save_draft,
            commands::compose::save_draft_version,
            commands::compose::load_draft_versions,
            commands::compose::delete_draft_versions,
            // Settings commands
            commands::settings::get_settings,
            commands::settings::save_settings,
            // Signature commands
            commands::settings::get_signature,
            commands::settings::save_signature,
            // Autostart command
            commands::settings::set_autostart,
            // Contact commands
            commands::contacts::search_contacts,
            commands::contacts::get_all_contacts,
            commands::contacts::update_contact,
            commands::contacts::delete_contact,
            commands::contacts::get_unread_count,
            // Rules commands
            commands::rules::get_rules,
            commands::rules::save_rule,
            commands::rules::delete_rule,
            // Pin commands
            commands::pin::pin_mail,
            commands::pin::unpin_mail,
            // Snooze commands
            commands::snooze::snooze_mail,
            commands::snooze::unsnooze_mail,
            commands::snooze::get_due_snoozed,
            // Schedule commands
            commands::schedule::schedule_send,
            commands::schedule::get_scheduled_emails,
            commands::schedule::cancel_scheduled,
            commands::schedule::send_due_scheduled,
            // Flag follow-up commands
            commands::flags::flag_mail,
            commands::flags::unflag_mail,
            // Follow-up tracker commands
            commands::followup::add_followup,
            commands::followup::get_followups,
            commands::followup::resolve_followup,
            commands::followup::delete_followup,
            commands::followup::check_followups,
            // Task commands
            commands::tasks::create_task,
            commands::tasks::get_tasks,
            commands::tasks::complete_task,
            commands::tasks::delete_task,
            // Read receipt command
            commands::receipts::send_read_receipt,
            // Mailto handler
            get_pending_mailto,
            // Secure wipe
            commands::settings::secure_wipe,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            let msg = format!(
                "Exospine failed to start.\n\n\
                 Error: {e}\n\n\
                 This is often caused by a missing or outdated WebView2 runtime.\n\
                 Please install WebView2 from:\n\
                 https://developer.microsoft.com/en-us/microsoft-edge/webview2/\n\n\
                 If the problem persists, try running Exospine as administrator.",
            );
            tracing::error!("{}", msg);
            // Show a native message box so the user sees the error even without a console
            #[cfg(target_os = "windows")]
            {
                use std::ffi::OsStr;
                use std::os::windows::ffi::OsStrExt;
                let wide_msg: Vec<u16> = OsStr::new(&msg).encode_wide().chain(Some(0)).collect();
                let wide_title: Vec<u16> = OsStr::new("Exospine — Startup Error")
                    .encode_wide()
                    .chain(Some(0))
                    .collect();
                unsafe {
                    #[link(name = "user32")]
                    extern "system" {
                        fn MessageBoxW(
                            hwnd: *mut std::ffi::c_void,
                            text: *const u16,
                            caption: *const u16,
                            utype: u32,
                        ) -> i32;
                    }
                    // MB_OK | MB_ICONERROR = 0x10
                    MessageBoxW(
                        std::ptr::null_mut(),
                        wide_msg.as_ptr(),
                        wide_title.as_ptr(),
                        0x10,
                    );
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                eprintln!("{}", msg);
            }
            std::process::exit(1);
        });
}
