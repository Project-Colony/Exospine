//! Mail flag commands: read/unread, star, delete, archive.

use tauri::State;

use crate::app_state::{fetch_password, lock_or_recover};
use crate::AppState;

use super::mail::get_account;

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

    // "Archive" is the standard IMAP archive folder name (RFC 6154 \Archive).
    // Gmail uses "[Gmail]/All Mail" but most other providers use "Archive".
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
