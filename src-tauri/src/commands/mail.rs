//! Tauri commands for mail operations.

use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::State;
use tokio::time::timeout;

use crate::app_state::{fetch_password, lock_or_recover, Account, Folder, MailEntry};
use crate::mail::attachments;
use crate::mail::security::{AuthStatus, PhishingWarning};
use crate::AppState;

// ── Folder cache TTL (5 minutes) ────────────────────────────────────
static FOLDER_CACHE_TS: std::sync::LazyLock<StdMutex<HashMap<String, Instant>>> =
    std::sync::LazyLock::new(|| StdMutex::new(HashMap::new()));

const FOLDER_CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes

/// Try to connect to IMAP. If the connection fails with what looks like an
/// authentication error on an OAuth2 account, attempt to refresh the token
/// and retry once.
#[allow(dead_code)]
async fn connect_with_oauth_retry(
    state: &State<'_, AppState>,
    account: &Account,
    password: &str,
) -> Result<crate::mail::imap::ImapSession, String> {
    match crate::mail::connection::connect_with_retry(account, password, 3).await {
        Ok(session) => Ok(session),
        Err(e) => {
            let err_str = e.to_string().to_lowercase();
            let is_auth_error = err_str.contains("auth")
                || err_str.contains("login")
                || err_str.contains("credential")
                || err_str.contains("invalid");

            if is_auth_error && matches!(account.auth_method, crate::app_state::AuthMethod::OAuth2 { .. }) {
                tracing::warn!(
                    "IMAP auth failed for {}, attempting OAuth2 token refresh",
                    account.email
                );
                let config = lock_or_recover(&state.config).clone();
                match crate::try_refresh_oauth_token(account, &config).await {
                    Ok(new_token) => {
                        // Retry connection with refreshed token
                        crate::mail::connection::connect_with_retry(account, &new_token, 3)
                            .await
                            .map_err(|e| format!("IMAP connection failed after token refresh: {}", e))
                    }
                    Err(refresh_err) => {
                        tracing::error!(
                            "OAuth2 token refresh failed for {}: {}. User may need to re-authenticate.",
                            account.email,
                            refresh_err
                        );
                        Err(format!(
                            "Authentication failed and token refresh failed: {}. Please re-authenticate the account.",
                            refresh_err
                        ))
                    }
                }
            } else {
                Err(format!("Failed to connect to IMAP: {}", e))
            }
        }
    }
}

/// Response structure for get_mail_body, serialized as { text, html, security } for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct MailBodyResponse {
    pub text: String,
    pub html: Option<String>,
    pub phishing_warnings: Vec<PhishingWarning>,
    pub auth_status: Option<AuthStatus>,
    pub sender_warnings: Vec<String>,
    pub unsubscribe_url: Option<String>,
}

/// Helper: get account by ID from state.
fn get_account(state: &State<'_, AppState>, account_id: &str) -> Result<Account, String> {
    let accounts = lock_or_recover(&state.accounts);
    accounts
        .iter()
        .find(|a| a.id == account_id)
        .cloned()
        .ok_or_else(|| format!("Account not found: {}", account_id))
}

/// Fetch folder list for an account.
/// Returns cached folders if available and less than 5 minutes old; otherwise refreshes from IMAP.
#[tauri::command]
pub async fn get_folders(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Vec<Folder>, String> {
    tracing::debug!("get_folders: account_id={}", account_id);

    // 1. Check if we already have cached folders for this account within TTL
    let cached_folders = {
        let accounts = lock_or_recover(&state.accounts);
        accounts
            .iter()
            .find(|a| a.id == account_id)
            .and_then(|a| {
                if a.folders.is_empty() {
                    None
                } else {
                    Some(a.folders.clone())
                }
            })
    };

    if let Some(ref folders) = cached_folders {
        // Check TTL: if folders were fetched less than 5 min ago, return cached
        let within_ttl = FOLDER_CACHE_TS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&account_id)
            .map(|ts| ts.elapsed() < FOLDER_CACHE_TTL)
            .unwrap_or(false);

        if within_ttl {
            tracing::debug!("get_folders: returning {} cached folders (within TTL)", folders.len());
            return Ok(folders.clone());
        }
    }

    // 2. Cache missing or TTL expired — fetch from IMAP
    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    let mut session = crate::mail::connection::get_session(&account_id, &account, &password)
        .await
        .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

    let folders = timeout(
        Duration::from_secs(30),
        crate::mail::folders::fetch_folders_with_counts(&mut session),
    )
    .await
    .map_err(|_| "Timeout: folder fetch took longer than 30 seconds".to_string())?
    .map_err(|e| format!("Failed to fetch folders: {}", e))?;

    crate::mail::connection::return_session(&account_id, session);

    // 3. Cache folders in account state, persist, and record TTL timestamp
    {
        let mut accounts = lock_or_recover(&state.accounts);
        if let Some(acct) = accounts.iter_mut().find(|a| a.id == account_id) {
            acct.folders = folders.clone();
        }
        crate::config::save_accounts(&accounts);
    }
    FOLDER_CACHE_TS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(account_id.clone(), Instant::now());

    tracing::info!("get_folders: fetched {} folders from IMAP", folders.len());
    Ok(folders)
}

/// Get mail headers from SQLite cache (paginated).
/// On the first call (page 0 with no cached data), triggers an IMAP sync automatically.
#[tauri::command]
pub async fn get_mails(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    page: u32,
    per_page: u32,
) -> Result<Vec<MailEntry>, String> {
    tracing::debug!(
        "get_mails: account_id={}, folder={}, page={}, per_page={}",
        account_id, folder, page, per_page
    );
    let offset = page * per_page;

    // Check if we have cached data
    let cached_count = {
        let db_guard = lock_or_recover(&state.db);
        match db_guard.as_ref() {
            Some(db) => db.message_count(&account_id, &folder).unwrap_or(0),
            None => 0,
        }
    };

    // Pagination boundary check: if requested offset exceeds total count, return empty
    if cached_count > 0 && offset >= cached_count as u32 {
        tracing::debug!(
            "get_mails: page {} beyond total count {} — returning empty",
            page, cached_count
        );
        return Ok(Vec::new());
    }

    // If no cached data and first page, fetch the latest N mails from IMAP (fast)
    if cached_count == 0 && page == 0 {
        tracing::info!("get_mails: no cache, fetching from IMAP for {}/{}", account_id, folder);
        let account = get_account(&state, &account_id)?;
        let password = fetch_password(&account_id).await?;

        let mut session = crate::mail::connection::get_session_for_folder(&account_id, &account, &password, &folder)
            .await
            .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

        // Only fetch the latest 50 mails (not all 2000+)
        let mut mails =
            crate::mail::imap::fetch_messages(&mut session, &folder, per_page)
                .await
                .map_err(|e| format!("Failed to fetch messages: {}", e))?;

        crate::mail::connection::return_session_with_folder(&account_id, session, folder.clone());

        // Ensure account_id is set on all entries (fetch_messages passes "" to parser)
        for m in &mut mails {
            if m.account_id.is_empty() {
                m.account_id = account_id.clone();
            }
        }

        // Cache to DB
        if !mails.is_empty() {
            let db_guard = lock_or_recover(&state.db);
            if let Some(ref db) = *db_guard {
                if let Err(e) = db.save_messages(&mails) {
                    tracing::error!("get_mails: failed to cache messages to DB: {}", e);
                }
            }
        }

        tracing::info!("get_mails: returning {} mails from IMAP", mails.len());
        return Ok(mails);
    }

    // Read from SQLite cache
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;

    let result = db
        .load_message_headers_page(&account_id, &folder, per_page, offset)
        .map_err(|e| format!("Failed to load messages from cache: {}", e))?;

    tracing::debug!("get_mails: returning {} cached mails", result.len());
    Ok(result)
}

/// Background sync: fetch ALL mails from IMAP that are not yet in SQLite cache.
/// Returns the number of newly synced mails.
#[tauri::command]
pub async fn sync_all_mails(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
) -> Result<u32, String> {
    tracing::info!("sync_all_mails: starting for {}/{}", account_id, folder);

    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    let mut session = crate::mail::connection::get_session_for_folder(&account_id, &account, &password, &folder)
        .await
        .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

    // 1. Get all UIDs from IMAP
    let all_uids = crate::mail::imap::fetch_uids_for_folder(&mut session, &folder)
        .await
        .map_err(|e| format!("Failed to fetch UIDs: {}", e))?;

    if all_uids.is_empty() {
        crate::mail::connection::return_session_with_folder(&account_id, session, folder);
        tracing::info!("sync_all_mails: no UIDs found, nothing to sync");
        return Ok(0);
    }

    // 2. Get cached UIDs from SQLite
    let cached_uids = {
        let db_guard = lock_or_recover(&state.db);
        match db_guard.as_ref() {
            Some(db) => db.cached_uids(&account_id, &folder).unwrap_or_default(),
            None => Vec::new(),
        }
    };

    // 3. Find missing UIDs
    let cached_set: std::collections::HashSet<u32> = cached_uids.into_iter().collect();
    let missing_uids: Vec<u32> = all_uids
        .into_iter()
        .filter(|uid| !cached_set.contains(uid))
        .collect();

    if missing_uids.is_empty() {
        crate::mail::connection::return_session_with_folder(&account_id, session, folder);
        tracing::info!("sync_all_mails: all UIDs already cached");
        return Ok(0);
    }

    let total_missing = missing_uids.len() as u32;
    tracing::info!(
        "sync_all_mails: {} missing UIDs to fetch for {}/{}",
        total_missing,
        account_id,
        folder
    );

    // 4. Acquire rate limiter permit
    let limiter = crate::mail::rate_limiter::get_limiter(&account_id);
    let _permit = limiter.acquire().await.map_err(|e| format!("Rate limiter error: {}", e))?;

    // 5. Fetch missing in batches of 100 (resilient: continue on batch failure)
    let mut synced = 0u32;
    let mut failed = 0u32;
    for chunk in missing_uids.chunks(100) {
        let batch_result = timeout(
            Duration::from_secs(30),
            crate::mail::imap::fetch_batch_by_uids(
                &mut session,
                chunk,
                &account_id,
                &folder,
            ),
        )
        .await;
        match batch_result {
            Err(_) => {
                tracing::error!(
                    "sync_all_mails: timeout fetching batch of {} UIDs for {}/{}",
                    chunk.len(), account_id, folder
                );
                failed += chunk.len() as u32;
                continue;
            }
            Ok(inner) => match inner {
                Ok(mails) => {
                    if !mails.is_empty() {
                        let count = mails.len() as u32;
                        let db_guard = lock_or_recover(&state.db);
                        if let Some(ref db) = *db_guard {
                            if let Err(e) = db.save_messages(&mails) {
                                tracing::error!("sync_all_mails: failed to save batch to DB: {}", e);
                                failed += count;
                                continue;
                            }
                        }
                        synced += count;
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "sync_all_mails: batch fetch failed for {}/{}: {} (skipping {} UIDs)",
                        account_id, folder, e, chunk.len()
                    );
                    failed += chunk.len() as u32;
                    continue;
                }
            },
        }
    }

    crate::mail::connection::return_session_with_folder(&account_id, session, folder.clone());

    if failed > 0 {
        tracing::warn!(
            "sync_all_mails: {} UIDs failed, {} synced for {}/{}",
            failed, synced, account_id, folder
        );
    }

    // Send desktop notification for newly synced mails
    if synced > 0 {
        let show_notif = state
            .settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .show_notifications;
        if show_notif {
            if let Err(e) = notify_rust::Notification::new()
                .summary("New email")
                .body(&format!("{} new emails synced in {}", synced, folder))
                .appname("Exospine")
                .show()
            {
                tracing::warn!("Failed to show desktop notification: {}", e);
            }
        }
    }

    tracing::info!(
        "sync_all_mails: synced {} new mails for {}/{}",
        synced,
        account_id,
        folder
    );

    Ok(synced)
}

/// Load body from SQLite for a specific message.
/// Falls back to fetching from IMAP if the cached body is empty.
#[tauri::command]
pub async fn get_mail_body(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<MailBodyResponse, String> {
    tracing::debug!("get_mail_body: mail_id={}", mail_id);

    // Try loading from DB first
    let db_result = {
        let db_guard = lock_or_recover(&state.db);
        match db_guard.as_ref() {
            Some(db) => db.load_message_body(&mail_id).ok(),
            None => None,
        }
    };

    // Fetch unsubscribe_url from DB (separate lightweight query)
    let unsub_url = {
        let db_guard = lock_or_recover(&state.db);
        match db_guard.as_ref() {
            Some(db) => db.get_unsubscribe_url(&mail_id).unwrap_or(None),
            None => None,
        }
    };

    if let Some((ref text, ref html)) = db_result {
        if !text.is_empty() || html.is_some() {
            tracing::debug!("get_mail_body: returning cached body");
            // Run security analysis on cached body
            let analysis = crate::mail::security::analyze_email(
                "", // from not available from body-only cache
                None,
                html.as_deref(),
                "",
            );
            return Ok(MailBodyResponse {
                text: text.clone(),
                html: html.clone(),
                phishing_warnings: analysis.phishing_warnings,
                auth_status: analysis.auth_status,
                sender_warnings: analysis.sender_warnings,
                unsubscribe_url: unsub_url,
            });
        }
    }

    tracing::debug!("get_mail_body: cache miss, fetching from IMAP");

    // Fallback: fetch from IMAP by UID
    let (account_id, folder, uid) = {
        let db_guard = lock_or_recover(&state.db);
        let db = db_guard
            .as_ref()
            .ok_or_else(|| "Database not available".to_string())?;
        db.load_message_meta(&mail_id)
            .map_err(|e| format!("Failed to load message metadata: {}", e))?
    };

    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    let mut session = crate::mail::connection::get_session_for_folder(&account_id, &account, &password, &folder)
        .await
        .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

    let mails = timeout(
        Duration::from_secs(15),
        crate::mail::imap::fetch_batch_by_uids(
            &mut session,
            &[uid],
            &account_id,
            &folder,
        ),
    )
    .await
    .map_err(|_| "Timeout: fetching message body took longer than 15 seconds".to_string())?
    .map_err(|e| format!("Failed to fetch message body: {}", e))?;

    crate::mail::connection::return_session_with_folder(&account_id, session, folder.clone());

    if let Some(mail) = mails.first() {
        // Cache the body to DB for future requests
        {
            let db_guard = lock_or_recover(&state.db);
            if let Some(ref db) = *db_guard {
                if let Err(e) = db.save_messages(std::slice::from_ref(mail)) {
                    tracing::error!("get_mail_body: failed to cache body to DB: {}", e);
                }
            }
        }

        Ok(MailBodyResponse {
            text: mail.body_text.clone(),
            html: mail.body_html.clone(),
            phishing_warnings: mail.phishing_warnings.clone(),
            auth_status: mail.auth_status.clone(),
            sender_warnings: mail.sender_warnings.clone(),
            unsubscribe_url: mail.unsubscribe_url.clone(),
        })
    } else {
        tracing::warn!("get_mail_body: no message found for uid={}", uid);
        Ok(MailBodyResponse {
            text: String::new(),
            html: None,
            phishing_warnings: Vec::new(),
            auth_status: None,
            sender_warnings: Vec::new(),
            unsubscribe_url: None,
        })
    }
}

/// Delta sync from IMAP: fetch new messages since last known UID.
#[tauri::command]
pub async fn refresh_folder(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
) -> Result<Vec<MailEntry>, String> {
    tracing::info!("refresh_folder: {}/{}", account_id, folder);

    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    // Get max UID from DB
    let since_uid = {
        let db_guard = lock_or_recover(&state.db);
        match db_guard.as_ref() {
            Some(db) => db.max_uid(&account_id, &folder).unwrap_or(0),
            None => 0,
        }
    };

    let mut session = crate::mail::connection::get_session_for_folder(&account_id, &account, &password, &folder)
        .await
        .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

    let imap_future = async {
        if since_uid == 0 {
            crate::mail::imap::fetch_all_mails_batched(&mut session, &folder, &account_id)
                .await
                .map_err(|e| format!("Failed to fetch all mails: {}", e))
        } else {
            crate::mail::imap::fetch_new_mails(&mut session, &folder, &account_id, since_uid)
                .await
                .map_err(|e| format!("Failed to fetch new mails: {}", e))
        }
    };
    let new_mails = timeout(Duration::from_secs(60), imap_future)
        .await
        .map_err(|_| "Timeout: refresh_folder took longer than 60 seconds".to_string())??;

    // Also refresh folder list/counts while we have an open session
    if let Ok(folders) = crate::mail::folders::fetch_folders_with_counts(&mut session).await {
        let mut accounts = lock_or_recover(&state.accounts);
        if let Some(acct) = accounts.iter_mut().find(|a| a.id == account_id) {
            acct.folders = folders;
        }
        crate::config::save_accounts(&accounts);
    }

    crate::mail::connection::return_session_with_folder(&account_id, session, folder.clone());

    // Save to DB
    if !new_mails.is_empty() {
        let db_guard = lock_or_recover(&state.db);
        if let Some(ref db) = *db_guard {
            db.save_messages(&new_mails)
                .map_err(|e| format!("Failed to save new mails to DB: {}", e))?;
        }
    }

    // Send desktop notification for new mails
    if !new_mails.is_empty() {
        // Check if notifications are enabled
        let show_notif = state
            .settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .show_notifications;
        if show_notif {
            crate::notifications::notify_new_mails_batch(&new_mails);
        }
    }

    // Extract contacts from new mails
    if !new_mails.is_empty() {
        let db_guard = lock_or_recover(&state.db);
        if let Some(ref db) = *db_guard {
            if let Err(e) = db.extract_contacts_from_mails(&new_mails) {
                tracing::warn!("refresh_folder: failed to extract contacts: {}", e);
            }
        }
    }

    // Apply email rules to new mails
    let rules = crate::mail::rules::load_rules();
    if !rules.is_empty() && !new_mails.is_empty() {
        for mail in &new_mails {
            let actions = crate::mail::rules::apply_rules(mail, &rules);
            for action in &actions {
                match action {
                    crate::mail::rules::RuleAction::MarkAsRead => {
                        let db_guard = lock_or_recover(&state.db);
                        if let Some(ref db) = *db_guard {
                            let _ = db.update_read_status(&mail.id, true);
                        }
                    }
                    crate::mail::rules::RuleAction::Star => {
                        let db_guard = lock_or_recover(&state.db);
                        if let Some(ref db) = *db_guard {
                            let _ = db.update_star_status(&mail.id, true);
                        }
                    }
                    _ => {
                        tracing::info!("refresh_folder: rule action for mail {}", mail.id);
                    }
                }
            }
        }
    }

    tracing::info!("refresh_folder: {} new mails synced", new_mails.len());
    Ok(new_mails)
}

/// Mark a message as read on IMAP and in the local DB.
#[tauri::command]
pub async fn mark_read(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    mail_uid: String,
) -> Result<(), String> {
    tracing::debug!("mark_read: uid={} in {}/{}", mail_uid, account_id, folder);

    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    let mut session = crate::mail::connection::get_session_for_folder(&account_id, &account, &password, &folder)
        .await
        .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

    crate::mail::imap::mark_as_read(&mut session, &mail_uid)
        .await
        .map_err(|e| format!("Failed to mark as read: {}", e))?;

    crate::mail::connection::return_session_with_folder(&account_id, session, folder);

    // Update DB
    {
        let db_guard = lock_or_recover(&state.db);
        if let Some(ref db) = *db_guard {
            if let Err(e) = db.update_read_status(&mail_uid, true) {
                tracing::error!("mark_read: DB update failed: {}", e);
            }
        }
    }

    Ok(())
}

/// Mark a message as unread on IMAP and in the local DB.
#[tauri::command]
pub async fn mark_unread(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    mail_uid: String,
) -> Result<(), String> {
    tracing::debug!("mark_unread: uid={} in {}/{}", mail_uid, account_id, folder);

    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    let mut session = crate::mail::connection::get_session_for_folder(&account_id, &account, &password, &folder)
        .await
        .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

    crate::mail::imap::mark_as_unread(&mut session, &mail_uid)
        .await
        .map_err(|e| format!("Failed to mark as unread: {}", e))?;

    crate::mail::connection::return_session_with_folder(&account_id, session, folder);

    {
        let db_guard = lock_or_recover(&state.db);
        if let Some(ref db) = *db_guard {
            if let Err(e) = db.update_read_status(&mail_uid, false) {
                tracing::error!("mark_unread: DB update failed: {}", e);
            }
        }
    }

    Ok(())
}

/// Toggle starred/flagged status on IMAP and in the local DB.
#[tauri::command]
pub async fn toggle_star(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    mail_uid: String,
    is_starred: bool,
) -> Result<(), String> {
    tracing::debug!("toggle_star: uid={}, starred={} in {}/{}", mail_uid, is_starred, account_id, folder);

    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    let mut session = crate::mail::connection::get_session_for_folder(&account_id, &account, &password, &folder)
        .await
        .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

    if is_starred {
        crate::mail::imap::set_flagged(&mut session, &mail_uid)
            .await
            .map_err(|e| format!("Failed to set flag: {}", e))?;
    } else {
        crate::mail::imap::remove_flagged(&mut session, &mail_uid)
            .await
            .map_err(|e| format!("Failed to remove flag: {}", e))?;
    }

    crate::mail::connection::return_session_with_folder(&account_id, session, folder);

    {
        let db_guard = lock_or_recover(&state.db);
        if let Some(ref db) = *db_guard {
            if let Err(e) = db.update_star_status(&mail_uid, is_starred) {
                tracing::error!("toggle_star: DB update failed: {}", e);
            }
        }
    }

    Ok(())
}

/// Delete a message on IMAP and in the local DB.
#[tauri::command]
pub async fn delete_mail(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    mail_uid: String,
) -> Result<(), String> {
    tracing::info!("delete_mail: uid={} in {}/{}", mail_uid, account_id, folder);

    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    let mut session = crate::mail::connection::get_session_for_folder(&account_id, &account, &password, &folder)
        .await
        .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

    crate::mail::imap::delete_message(&mut session, &mail_uid)
        .await
        .map_err(|e| format!("Failed to delete message: {}", e))?;

    crate::mail::connection::return_session_with_folder(&account_id, session, folder);

    {
        let db_guard = lock_or_recover(&state.db);
        if let Some(ref db) = *db_guard {
            if let Err(e) = db.delete_message(&mail_uid) {
                tracing::error!("delete_mail: DB delete failed: {}", e);
            }
        }
    }

    Ok(())
}

/// Archive a message (move to "Archive" folder on IMAP).
#[tauri::command]
pub async fn archive_mail(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    mail_uid: String,
) -> Result<(), String> {
    tracing::info!("archive_mail: uid={} in {}/{}", mail_uid, account_id, folder);

    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    let mut session = crate::mail::connection::get_session_for_folder(&account_id, &account, &password, &folder)
        .await
        .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

    crate::mail::imap::move_message(&mut session, &mail_uid, "Archive")
        .await
        .map_err(|e| format!("Failed to archive message: {}", e))?;

    crate::mail::connection::return_session_with_folder(&account_id, session, folder);

    {
        let db_guard = lock_or_recover(&state.db);
        if let Some(ref db) = *db_guard {
            if let Err(e) = db.delete_message(&mail_uid) {
                tracing::error!("archive_mail: DB delete failed: {}", e);
            }
        }
    }

    Ok(())
}

/// Search messages in the local SQLite cache for a given account and folder.
/// Supports optional structured filters: filter_from, filter_subject, filter_to, filter_has_attachment.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn search_local(
    state: State<'_, AppState>,
    query: String,
    account_id: String,
    folder: String,
    filter_from: Option<String>,
    filter_subject: Option<String>,
    filter_to: Option<String>,
    filter_has_attachment: Option<bool>,
) -> Result<Vec<MailEntry>, String> {
    tracing::debug!(
        "search_local: query={}, account_id={}, folder={}, from={:?}, subject={:?}, to={:?}, has_attachment={:?}",
        query, account_id, folder, filter_from, filter_subject, filter_to, filter_has_attachment
    );
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard.as_ref().ok_or("Database not available")?;

    // If no structured filters, use the standard search
    let has_filters = filter_from.is_some()
        || filter_subject.is_some()
        || filter_to.is_some()
        || filter_has_attachment.unwrap_or(false);

    if !has_filters {
        return db
            .search_messages_in_folder(&query, &account_id, &folder)
            .map_err(|e| e.to_string());
    }

    // Build dynamic SQL WHERE clause for structured search
    let mut conditions = vec![
        "account_id = ?1".to_string(),
        "folder = ?2".to_string(),
    ];
    let mut param_values: Vec<String> = vec![account_id.clone(), folder.clone()];
    let mut param_idx = 3u32;

    if !query.is_empty() {
        let pattern = format!("%{}%", query);
        conditions.push(format!(
            "(subject LIKE ?{idx} OR from_addr LIKE ?{idx} OR preview LIKE ?{idx})",
            idx = param_idx
        ));
        param_values.push(pattern);
        param_idx += 1;
    }

    if let Some(ref from_filter) = filter_from {
        let pattern = format!("%{}%", from_filter);
        conditions.push(format!("from_addr LIKE ?{}", param_idx));
        param_values.push(pattern);
        param_idx += 1;
    }

    if let Some(ref subject_filter) = filter_subject {
        let pattern = format!("%{}%", subject_filter);
        conditions.push(format!("subject LIKE ?{}", param_idx));
        param_values.push(pattern);
        param_idx += 1;
    }

    if let Some(ref to_filter) = filter_to {
        let pattern = format!("%{}%", to_filter);
        conditions.push(format!("to_addrs LIKE ?{}", param_idx));
        param_values.push(pattern);
        param_idx += 1;
    }

    if filter_has_attachment.unwrap_or(false) {
        conditions.push("has_attachments = 1".to_string());
    }

    let _ = param_idx; // suppress unused warning

    let where_clause = conditions.join(" AND ");
    let sql = format!(
        "SELECT id, account_id, folder, from_addr, to_addrs, subject, date, preview, body_text, body_html, is_read, is_starred, has_attachments, uid, message_id, in_reply_to, refs, thread_id, spam_score, categories, is_pinned, snoozed_until \
         FROM messages WHERE {} ORDER BY date DESC LIMIT 100",
        where_clause
    );

    db.search_messages_dynamic(&sql, &param_values)
        .map_err(|e| e.to_string())
}

/// Move a message to a different folder on IMAP.
#[tauri::command]
pub async fn move_mail(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    mail_uid: String,
    target_folder: String,
) -> Result<(), String> {
    tracing::info!("move_mail: uid={} from {}/{} to {}", mail_uid, account_id, folder, target_folder);

    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    let mut session = crate::mail::connection::get_session_for_folder(&account_id, &account, &password, &folder)
        .await
        .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

    crate::mail::imap::move_message(&mut session, &mail_uid, &target_folder)
        .await
        .map_err(|e| format!("Failed to move message: {}", e))?;

    crate::mail::connection::return_session_with_folder(&account_id, session, folder);

    {
        let db_guard = lock_or_recover(&state.db);
        if let Some(ref db) = *db_guard {
            if let Err(e) = db.delete_message(&mail_uid) {
                tracing::error!("move_mail: DB delete failed: {}", e);
            }
        }
    }

    Ok(())
}

/// Download a specific attachment from an email.
/// Fetches the raw message from IMAP, extracts the attachment by part_index,
/// saves it to the Downloads/Exospine folder, and returns the file path.
#[tauri::command]
pub async fn download_attachment(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    mail_uid: u32,
    part_index: usize,
) -> Result<String, String> {
    tracing::info!(
        "download_attachment: uid={} part={} in {}/{}",
        mail_uid, part_index, account_id, folder
    );

    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    let mut session = crate::mail::connection::get_session_for_folder(&account_id, &account, &password, &folder)
        .await
        .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

    // Fetch the raw message by UID
    let raw = fetch_raw_message(&mut session, mail_uid).await?;

    crate::mail::connection::return_session_with_folder(&account_id, session, folder);

    // Save the attachment to disk
    let dest = attachments::download_dir();
    let file_path = attachments::save_attachment(&raw, part_index, &dest)
        .await
        .map_err(|e| format!("Failed to save attachment: {}", e))?;

    Ok(file_path.to_string_lossy().to_string())
}

/// Export (download) the raw RFC 822 email as a .eml file.
#[tauri::command]
pub async fn export_mail(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    mail_uid: u32,
) -> Result<String, String> {
    tracing::info!(
        "export_mail: uid={} in {}/{}",
        mail_uid, account_id, folder
    );

    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    let mut session = crate::mail::connection::get_session_for_folder(&account_id, &account, &password, &folder)
        .await
        .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

    // Fetch the raw message
    let raw = fetch_raw_message(&mut session, mail_uid).await?;

    crate::mail::connection::return_session_with_folder(&account_id, session, folder);

    // Save as .eml file in Downloads/Exospine
    let dest = attachments::download_dir();
    tokio::fs::create_dir_all(&dest)
        .await
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let filename = format!("email_{}.eml", mail_uid);
    let file_path = dest.join(&filename);

    tokio::fs::write(&file_path, &raw)
        .await
        .map_err(|e| format!("Failed to write .eml file: {}", e))?;

    Ok(file_path.to_string_lossy().to_string())
}

/// Fetch raw email headers for a specific message by UID.
/// Returns the raw header string for display in the UI.
#[tauri::command]
pub async fn get_mail_headers(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    mail_uid: u32,
) -> Result<String, String> {
    tracing::debug!(
        "get_mail_headers: uid={} in {}/{}",
        mail_uid, account_id, folder
    );

    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    let mut session = crate::mail::connection::get_session_for_folder(
        &account_id, &account, &password, &folder,
    )
    .await
    .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

    // Fetch only the HEADER section for this UID
    let uid_str = mail_uid.to_string();
    let messages: Vec<_> = {
        use futures::TryStreamExt;
        session
            .uid_fetch(&uid_str, "BODY.PEEK[HEADER]")
            .await
            .map_err(|e| format!("Failed to fetch headers: {}", e))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| format!("Failed to collect headers: {}", e))?
    };

    crate::mail::connection::return_session_with_folder(&account_id, session, folder);

    let msg = messages
        .first()
        .ok_or_else(|| format!("No message found for UID {}", mail_uid))?;

    // BODY[HEADER] is returned in the body() field
    let header_bytes = msg
        .body()
        .ok_or_else(|| format!("Message UID {} has no headers", mail_uid))?;

    let headers = String::from_utf8_lossy(header_bytes).to_string();
    Ok(headers)
}

/// Read the last N entries from the security audit log.
#[tauri::command]
pub async fn get_security_log(count: Option<u32>) -> Result<Vec<String>, String> {
    let n = count.unwrap_or(20) as usize;
    Ok(crate::storage::audit_log::read_last_entries(n))
}

/// Add a category/label to a mail message.
/// Also attempts to sync the keyword to IMAP via STORE +FLAGS.
#[tauri::command]
pub async fn add_category(
    state: State<'_, AppState>,
    mail_id: String,
    category: String,
) -> Result<(), String> {
    tracing::debug!("add_category: mail_id={}, category={}", mail_id, category);
    {
        let db_guard = lock_or_recover(&state.db);
        let db = db_guard.as_ref().ok_or("Database not available")?;
        db.add_category(&mail_id, &category)
            .map_err(|e| format!("Failed to add category: {}", e))?;
    }

    // Best-effort sync to IMAP keywords
    sync_imap_keyword(&state, &mail_id, &category, true).await;

    Ok(())
}

/// Remove a category/label from a mail message.
/// Also attempts to remove the IMAP keyword via STORE -FLAGS.
#[tauri::command]
pub async fn remove_category(
    state: State<'_, AppState>,
    mail_id: String,
    category: String,
) -> Result<(), String> {
    tracing::debug!("remove_category: mail_id={}, category={}", mail_id, category);
    {
        let db_guard = lock_or_recover(&state.db);
        let db = db_guard.as_ref().ok_or("Database not available")?;
        db.remove_category(&mail_id, &category)
            .map_err(|e| format!("Failed to remove category: {}", e))?;
    }

    // Best-effort sync to IMAP keywords
    sync_imap_keyword(&state, &mail_id, &category, false).await;

    Ok(())
}

/// Helper: sync a category as an IMAP keyword flag (+FLAGS or -FLAGS).
/// Best-effort: failures are logged but do not cause the command to fail.
async fn sync_imap_keyword(
    state: &State<'_, AppState>,
    mail_id: &str,
    category: &str,
    add: bool,
) {
    // Look up account_id, folder, uid for this mail
    let meta = {
        let db_guard = lock_or_recover(&state.db);
        match db_guard.as_ref() {
            Some(db) => db.load_message_meta(mail_id).ok(),
            None => None,
        }
    };

    let Some((account_id, folder, uid)) = meta else {
        tracing::debug!("sync_imap_keyword: no meta for mail_id={}, skipping IMAP sync", mail_id);
        return;
    };

    let account = match get_account(state, &account_id) {
        Ok(a) => a,
        Err(_) => return,
    };
    let password = match fetch_password(&account_id).await {
        Ok(p) => p,
        Err(_) => return,
    };

    let session_result = crate::mail::connection::get_session_for_folder(
        &account_id, &account, &password, &folder,
    )
    .await;

    let mut session = match session_result {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("sync_imap_keyword: failed to connect for {}: {}", mail_id, e);
            return;
        }
    };

    let flag_op = if add { "+FLAGS" } else { "-FLAGS" };
    let uid_str = uid.to_string();
    let keyword = category.replace(' ', "_");

    match session
        .uid_store(&uid_str, &format!("{} ({})", flag_op, keyword))
        .await
    {
        Ok(stream) => {
            // Consume the stream to complete the operation
            use futures::TryStreamExt;
            let _ = stream.try_collect::<Vec<_>>().await;
            tracing::debug!(
                "sync_imap_keyword: {} keyword '{}' on uid={} in {}/{}",
                if add { "added" } else { "removed" },
                keyword,
                uid,
                account_id,
                folder
            );
        }
        Err(e) => {
            tracing::warn!(
                "sync_imap_keyword: STORE {} failed for uid={}: {}",
                flag_op,
                uid,
                e
            );
        }
    }

    crate::mail::connection::return_session_with_folder(&account_id, session, folder);
}

/// Report a mail as spam (stores user decision for future filtering).
#[tauri::command]
pub async fn report_spam(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<(), String> {
    tracing::debug!("report_spam: mail_id={}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard.as_ref().ok_or("Database not available")?;
    db.add_category(&mail_id, "Spam")
        .map_err(|e| format!("Failed to report spam: {}", e))
}

/// Report a mail as not spam (removes Spam category).
#[tauri::command]
pub async fn report_not_spam(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<(), String> {
    tracing::debug!("report_not_spam: mail_id={}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard.as_ref().ok_or("Database not available")?;
    db.remove_category(&mail_id, "Spam")
        .map_err(|e| format!("Failed to unmark spam: {}", e))
}

/// Helper: fetch the raw RFC 822 message bytes for a given UID.
async fn fetch_raw_message(
    session: &mut crate::mail::imap::ImapSession,
    uid: u32,
) -> Result<Vec<u8>, String> {
    use futures::TryStreamExt;

    let uid_str = uid.to_string();
    let messages: Vec<_> = session
        .uid_fetch(&uid_str, "BODY.PEEK[]")
        .await
        .map_err(|e| format!("Failed to fetch raw message: {}", e))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|e| format!("Failed to collect raw message: {}", e))?;

    let msg = messages
        .first()
        .ok_or_else(|| format!("No message found for UID {}", uid))?;

    let body = msg
        .body()
        .ok_or_else(|| format!("Message UID {} has no body", uid))?;

    Ok(body.to_vec())
}

/// Get email analytics for an account.
#[tauri::command]
pub async fn get_analytics(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<crate::storage::db::EmailAnalytics, String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard.as_ref().ok_or("Database not initialized")?;
    db.get_analytics(&account_id)
        .map_err(|e| format!("Failed to get analytics: {}", e))
}

/// Delete messages older than the specified number of days. 0 = disabled (no-op).
#[tauri::command]
pub async fn cleanup_old_messages(
    state: State<'_, AppState>,
    days: u32,
) -> Result<usize, String> {
    if days == 0 {
        return Ok(0);
    }
    tracing::info!("cleanup_old_messages: deleting messages older than {} days", days);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    let count = db
        .cleanup_old_messages(days)
        .map_err(|e| format!("Failed to cleanup old messages: {}", e))?;
    if count > 0 {
        tracing::info!("cleanup_old_messages: deleted {} messages", count);
    }
    Ok(count)
}

/// Sweep sender: delete all messages from a specific sender in a folder.
/// Deletes from both the local DB and IMAP.
#[tauri::command]
pub async fn sweep_sender(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    sender_email: String,
) -> Result<usize, String> {
    tracing::info!("sweep_sender: sender={} in {}/{}", sender_email, account_id, folder);

    // 1. Get UIDs from DB and delete locally
    let uids = {
        let db_guard = lock_or_recover(&state.db);
        let db = db_guard.as_ref().ok_or("Database not available")?;
        db.sweep_sender(&account_id, &folder, &sender_email)
            .map_err(|e| format!("DB sweep failed: {}", e))?
    };

    let count = uids.len();

    // 2. Delete on IMAP
    if !uids.is_empty() {
        let account = get_account(&state, &account_id)?;
        let password = fetch_password(&account_id).await?;

        let mut session = crate::mail::connection::get_session_for_folder(
            &account_id, &account, &password, &folder,
        )
        .await
        .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

        for uid in &uids {
            if let Err(e) = crate::mail::imap::delete_message(&mut session, uid).await {
                tracing::error!("sweep_sender: failed to delete uid {} on IMAP: {}", uid, e);
            }
        }

        crate::mail::connection::return_session_with_folder(&account_id, session, folder);
    }

    tracing::info!("sweep_sender: deleted {} messages from {}", count, sender_email);
    Ok(count)
}

/// Start IMAP IDLE on a given account's INBOX for push notifications.
/// Runs in a loop, reconnecting every 29 minutes per RFC 2177.
/// Emits `new-mail` Tauri event when new messages arrive.
#[tauri::command]
pub async fn start_idle(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    account_id: String,
) -> Result<(), String> {
    use tauri::Emitter;

    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    // Spawn a background task for IDLE
    tauri::async_runtime::spawn(async move {
        let timeout_secs: u64 = 29 * 60; // 29 minutes per RFC 2177

        loop {
            // Connect fresh for each IDLE cycle
            let session = match crate::mail::imap::connect(&account, &password).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("IDLE: failed to connect for {}: {}", account.email, e);
                    // Wait before retrying
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    continue;
                }
            };

            match crate::mail::idle::idle_wait(session, "INBOX", timeout_secs).await {
                Ok((new_mail, mut session)) => {
                    if new_mail {
                        tracing::info!("IDLE: new mail detected for {}", account.email);
                        let _ = app.emit("new-mail", serde_json::json!({
                            "count": 1,
                            "folder": "INBOX",
                            "accountId": account.id,
                        }));
                    }
                    // Logout and reconnect for next cycle
                    let _ = session.logout().await;
                }
                Err(e) => {
                    tracing::error!("IDLE: error for {}: {}", account.email, e);
                    // Wait before retrying
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                }
            }
        }
    });

    Ok(())
}

/// Open and parse a `.eml` file from the file system.
/// Returns the parsed email as a `MailEntry` for display in the reading pane.
#[tauri::command]
pub async fn open_eml_file(path: String) -> Result<MailEntry, String> {
    tracing::info!("open_eml_file: path={}", path);

    let raw = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("Failed to read .eml file: {}", e))?;

    let entry = crate::mail::parser::parse_email(&raw, "__local__", "__eml__")
        .map_err(|e| format!("Failed to parse .eml file: {}", e))?;

    Ok(entry)
}

// ── Notes commands ──────────────────────────────────────────────────

/// Save a note for a mail message.
#[tauri::command]
pub async fn save_note(
    state: State<'_, AppState>,
    mail_id: String,
    note: String,
) -> Result<(), String> {
    tracing::debug!("save_note: mail_id={}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard.as_ref().ok_or("Database not available")?;
    if note.trim().is_empty() {
        db.delete_note(&mail_id)
            .map_err(|e| format!("Failed to delete note: {}", e))
    } else {
        db.save_note(&mail_id, &note)
            .map_err(|e| format!("Failed to save note: {}", e))
    }
}

/// Get a note for a mail message.
#[tauri::command]
pub async fn get_note(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<Option<String>, String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard.as_ref().ok_or("Database not available")?;
    db.get_note(&mail_id)
        .map_err(|e| format!("Failed to get note: {}", e))
}

// ── Duplicate detection commands ────────────────────────────────────

/// A pair of duplicate message IDs.
#[derive(Debug, Clone, Serialize)]
pub struct DuplicatePair {
    pub original_id: String,
    pub duplicate_id: String,
}

/// Find duplicate messages in a folder.
#[tauri::command]
pub async fn find_duplicates(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
) -> Result<Vec<DuplicatePair>, String> {
    tracing::debug!("find_duplicates: {}/{}", account_id, folder);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard.as_ref().ok_or("Database not available")?;
    let pairs = db
        .find_duplicates(&account_id, &folder)
        .map_err(|e| format!("Failed to find duplicates: {}", e))?;
    Ok(pairs
        .into_iter()
        .map(|(orig, dup)| DuplicatePair {
            original_id: orig,
            duplicate_id: dup,
        })
        .collect())
}
