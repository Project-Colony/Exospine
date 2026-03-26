//! Mail sync commands: full sync, delta refresh, IMAP IDLE, and OAuth retry.

use std::time::Duration;

use tauri::State;
use tokio::time::timeout;

use crate::app_state::{fetch_password, lock_or_recover, Account, MailEntry};
use crate::AppState;

use super::mail::get_account;

/// Try to connect to IMAP. If the connection fails with what looks like an
/// authentication error on an OAuth2 account, attempt to refresh the token
/// and retry once.
#[allow(dead_code)]
pub(crate) async fn connect_with_oauth_retry(
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
                match crate::auth::try_refresh_oauth_token(account, &config).await {
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
            let cycle_result: Result<(), String> = async {
                // Connect fresh for each IDLE cycle
                let session = crate::mail::imap::connect(&account, &password)
                    .await
                    .map_err(|e| format!("IDLE: failed to connect for {}: {}", account.email, e))?;

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
                        return Err(format!("IDLE: error for {}: {}", account.email, e));
                    }
                }
                Ok(())
            }
            .await;

            if let Err(e) = cycle_result {
                tracing::error!("{}", e);
                // Wait before retrying — don't let a tight error loop burn CPU
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        }
    });

    Ok(())
}
