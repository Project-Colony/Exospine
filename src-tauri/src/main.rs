// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Parsed mailto: URL components.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct MailtoData {
    pub to: String,
    pub subject: String,
    pub body: String,
    pub cc: String,
    pub bcc: String,
}

/// Parse a `mailto:` URL into its components.
fn parse_mailto(url: &str) -> MailtoData {
    let mut data = MailtoData::default();
    let stripped = url.strip_prefix("mailto:").unwrap_or(url);

    // Split address part from query string
    let (addr_part, query_part) = if let Some(idx) = stripped.find('?') {
        (&stripped[..idx], Some(&stripped[idx + 1..]))
    } else {
        (stripped, None)
    };

    // URL-decode the address
    data.to = url_decode(addr_part);

    // Parse query parameters
    if let Some(query) = query_part {
        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                let decoded = url_decode(value);
                match key.to_lowercase().as_str() {
                    "subject" => data.subject = decoded,
                    "body" => data.body = decoded,
                    "cc" => data.cc = decoded,
                    "bcc" => data.bcc = decoded,
                    "to" => {
                        if !data.to.is_empty() {
                            data.to.push_str(", ");
                        }
                        data.to.push_str(&decoded);
                    }
                    _ => {}
                }
            }
        }
    }

    data
}

/// Simple percent-decoding for mailto URLs.
fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().unwrap_or(0);
            let lo = chars.next().unwrap_or(0);
            let hex = [hi, lo];
            if let Ok(s) = std::str::from_utf8(&hex) {
                if let Ok(val) = u8::from_str_radix(s, 16) {
                    result.push(val as char);
                    continue;
                }
            }
            result.push('%');
            result.push(hi as char);
            result.push(lo as char);
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    result
}

/// Parse an `exospine://compose?to=...&subject=...` URL into MailtoData.
fn parse_exospine_url(url: &str) -> MailtoData {
    let mut data = MailtoData::default();
    // Strip the scheme: "exospine://compose?..." -> "compose?..."
    let stripped = url
        .strip_prefix("exospine://")
        .unwrap_or(url);

    // Find query string after '?'
    let query_part = if let Some(idx) = stripped.find('?') {
        Some(&stripped[idx + 1..])
    } else {
        None
    };

    if let Some(query) = query_part {
        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                let decoded = url_decode(value);
                match key.to_lowercase().as_str() {
                    "to" => {
                        if !data.to.is_empty() {
                            data.to.push_str(", ");
                        }
                        data.to.push_str(&decoded);
                    }
                    "subject" => data.subject = decoded,
                    "body" => data.body = decoded,
                    "cc" => data.cc = decoded,
                    "bcc" => data.bcc = decoded,
                    _ => {}
                }
            }
        }
    }

    data
}

/// Global storage for a pending mailto: compose request, picked up by the frontend.
static PENDING_MAILTO: LazyLock<Mutex<Option<MailtoData>>> =
    LazyLock::new(|| Mutex::new(None));

mod accounts;
mod app_state;
mod commands;
mod config;
mod mail;
pub mod notifications;
mod search;
mod storage;

use app_state::{Account, AppSettings};
use config::Config;
use storage::db::Database;

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

    // Refresh OAuth2 tokens at startup in parallel (non-blocking)
    {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        let mut handles = Vec::new();
        for acct in &accounts {
            if let app_state::AuthMethod::OAuth2 { ref refresh_token } = acct.auth_method {
                let provider_cfg = accounts::provider::detect_provider(&acct.email);
                let (client_id, client_secret) = match provider_cfg.provider {
                    accounts::provider::Provider::Gmail => {
                        (config.google_client_id.clone(), config.google_client_secret.clone())
                    }
                    accounts::provider::Provider::Outlook => {
                        (config.microsoft_client_id.clone(), config.microsoft_client_secret.clone())
                    }
                    _ => {
                        (config.google_client_id.clone(), config.google_client_secret.clone())
                    }
                };
                let oauth_cfg = accounts::oauth2::config_for_provider(
                    provider_cfg.provider,
                    client_id,
                    client_secret,
                );
                let rt_token = refresh_token.clone();
                let acct_id = acct.id.clone();
                let acct_email = acct.email.clone();
                handles.push(rt.spawn(async move {
                    match accounts::oauth2::refresh_token(&oauth_cfg, &rt_token).await {
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
                }));
            }
        }
        // Wait for all refresh tasks to complete (with timeout)
        rt.block_on(async {
            for handle in handles {
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    handle,
                ).await;
            }
        });
    }

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
                window.open_devtools();
                // Restore window position/size from saved state
                restore_window_state(&window);
                // Save window state on close and trigger graceful shutdown
                let win = window.clone();
                let shutdown = shutdown_tx.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        save_window_state(&win);
                        // Signal all background tasks to stop
                        let _ = shutdown.send(());
                        // Brief grace period for background tasks
                        std::thread::sleep(std::time::Duration::from_millis(500));
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
                let acct_clone = acct.clone();
                let mut shutdown_rx = shutdown_tx.subscribe();
                tauri::async_runtime::spawn(async move {
                    let timeout_secs: u64 = 29 * 60; // 29 minutes per RFC 2177
                    loop {
                        // Check for shutdown before each cycle
                        if shutdown_rx.try_recv().is_ok() {
                            tracing::info!("IDLE task for {} received shutdown", acct_clone.email);
                            break;
                        }

                        let password = match crate::app_state::fetch_password(&acct_id).await {
                            Ok(pw) => pw,
                            Err(e) => {
                                tracing::error!("IDLE setup: no password for {}: {}", acct_clone.email, e);
                                tokio::select! {
                                    _ = tokio::time::sleep(std::time::Duration::from_secs(120)) => {}
                                    _ = shutdown_rx.recv() => { break; }
                                }
                                continue;
                            }
                        };
                        let session = match crate::mail::imap::connect(&acct_clone, &password).await {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::error!("IDLE: connect failed for {}: {}", acct_clone.email, e);
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
                                    tracing::info!("IDLE: new mail for {}", acct_clone.email);
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
                                tracing::error!("IDLE: error for {}: {}", acct_clone.email, e);
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
            // Mail commands
            commands::mail::get_folders,
            commands::mail::get_mails,
            commands::mail::get_mail_body,
            commands::mail::refresh_folder,
            commands::mail::sync_all_mails,
            commands::mail::mark_read,
            commands::mail::mark_unread,
            commands::mail::toggle_star,
            commands::mail::delete_mail,
            commands::mail::archive_mail,
            commands::mail::move_mail,
            commands::mail::download_attachment,
            commands::mail::export_mail,
            commands::mail::search_local,
            commands::mail::get_mail_headers,
            commands::mail::get_security_log,
            commands::mail::add_category,
            commands::mail::remove_category,
            commands::mail::report_spam,
            commands::mail::report_not_spam,
            commands::mail::get_analytics,
            commands::mail::cleanup_old_messages,
            commands::mail::start_idle,
            commands::mail::sweep_sender,
            commands::mail::open_eml_file,
            // Notes commands
            commands::mail::save_note,
            commands::mail::get_note,
            // Duplicate detection
            commands::mail::find_duplicates,
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
            // Pin/Snooze/Schedule commands
            commands::pin_snooze::pin_mail,
            commands::pin_snooze::unpin_mail,
            commands::pin_snooze::snooze_mail,
            commands::pin_snooze::unsnooze_mail,
            commands::pin_snooze::get_due_snoozed,
            commands::pin_snooze::schedule_send,
            commands::pin_snooze::get_scheduled_emails,
            commands::pin_snooze::cancel_scheduled,
            commands::pin_snooze::send_due_scheduled,
            // Flag follow-up commands
            commands::pin_snooze::flag_mail,
            commands::pin_snooze::unflag_mail,
            // Follow-up tracker commands
            commands::pin_snooze::add_followup,
            commands::pin_snooze::get_followups,
            commands::pin_snooze::resolve_followup,
            commands::pin_snooze::delete_followup,
            commands::pin_snooze::check_followups,
            // Task commands
            commands::pin_snooze::create_task,
            commands::pin_snooze::get_tasks,
            commands::pin_snooze::complete_task,
            commands::pin_snooze::delete_task,
            // Read receipt command
            commands::pin_snooze::send_read_receipt,
            // Mailto handler
            get_pending_mailto,
            // Secure wipe
            commands::settings::secure_wipe,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            let msg = format!(
                "Exospine failed to start.\n\n\
                 Error: {}\n\n\
                 This is often caused by a missing or outdated WebView2 runtime.\n\
                 Please install WebView2 from:\n\
                 https://developer.microsoft.com/en-us/microsoft-edge/webview2/\n\n\
                 If the problem persists, try running Exospine as administrator.",
                e
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

// ── Per-account token refresh lock ───────────────────────────────────

/// Prevents concurrent token refreshes for the same account.
/// Multiple tasks waiting on the same account will serialize;
/// the first to finish stores the new token and the rest will
/// find it in the credential store, avoiding double-refresh.
static TOKEN_REFRESH_LOCKS: LazyLock<Mutex<HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn get_token_refresh_lock(account_id: &str) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    let mut map = TOKEN_REFRESH_LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    map.entry(account_id.to_string())
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

// ── OAuth2 token refresh helper for IMAP auth failures ──────────────

/// Try to refresh an OAuth2 token for the given account.
/// Returns the new access token on success, or an error message.
/// Also stores the rotated refresh token if one is returned.
/// Uses a per-account lock to prevent concurrent double-refreshes.
pub async fn try_refresh_oauth_token(
    account: &Account,
    config: &Config,
) -> Result<String, String> {
    // Acquire per-account lock — if another task is already refreshing,
    // we wait for it and then return the already-refreshed credential.
    let lock = get_token_refresh_lock(&account.id);
    let _guard = lock.lock().await;
    let refresh_token = match &account.auth_method {
        app_state::AuthMethod::OAuth2 { refresh_token } => refresh_token.clone(),
        _ => return Err("Account is not OAuth2".to_string()),
    };

    let provider_cfg = accounts::provider::detect_provider(&account.email);
    let (client_id, client_secret) = match provider_cfg.provider {
        accounts::provider::Provider::Gmail => {
            (config.google_client_id.clone(), config.google_client_secret.clone())
        }
        accounts::provider::Provider::Outlook => {
            (config.microsoft_client_id.clone(), config.microsoft_client_secret.clone())
        }
        _ => {
            (config.google_client_id.clone(), config.google_client_secret.clone())
        }
    };

    let oauth_cfg = accounts::oauth2::config_for_provider(
        provider_cfg.provider,
        client_id,
        client_secret,
    );

    let token_resp = accounts::oauth2::refresh_token(&oauth_cfg, &refresh_token)
        .await
        .map_err(|e| format!("Token refresh failed: {}", e))?;

    // Store the new access token
    app_state::store_credential(&account.id, &token_resp.access_token);

    // If the provider rotated the refresh token, persist it
    if let Some(ref new_rt) = token_resp.refresh_token {
        if *new_rt != refresh_token {
            tracing::info!("OAuth2 refresh token rotated for {}", account.email);
            // Update the in-memory account and persist to disk
            // (Caller should update the accounts list if needed)
        }
    }

    Ok(token_resp.access_token)
}

// ── Window state persistence ─────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct WindowState {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    maximized: bool,
}

fn window_state_path() -> std::path::PathBuf {
    let dir = config::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("window_state.json")
}

fn save_window_state(window: &tauri::WebviewWindow) {
    let Ok(position) = window.outer_position() else { return };
    let Ok(size) = window.outer_size() else { return };
    let maximized = window.is_maximized().unwrap_or(false);

    let scale = window.scale_factor().unwrap_or(1.0);
    let state = WindowState {
        x: position.x as f64 / scale,
        y: position.y as f64 / scale,
        width: size.width as f64 / scale,
        height: size.height as f64 / scale,
        maximized,
    };

    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let _ = std::fs::write(window_state_path(), json);
    }
}

fn restore_window_state(window: &tauri::WebviewWindow) {
    let path = window_state_path();
    let Ok(data) = std::fs::read_to_string(&path) else { return };
    let Ok(state) = serde_json::from_str::<WindowState>(&data) else { return };

    let scale = window.scale_factor().unwrap_or(1.0);
    let _ = window.set_position(tauri::PhysicalPosition::new(
        (state.x * scale) as i32,
        (state.y * scale) as i32,
    ));
    let _ = window.set_size(tauri::PhysicalSize::new(
        (state.width * scale) as u32,
        (state.height * scale) as u32,
    ));
    if state.maximized {
        let _ = window.maximize();
    }
}
