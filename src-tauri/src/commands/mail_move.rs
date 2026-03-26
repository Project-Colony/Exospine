//! Mail move commands: move between folders and sweep sender.

use tauri::State;

use crate::app_state::{get_imap_session, lock_or_recover, map_err_str};
use crate::AppState;

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

    let (mut session, _account) = get_imap_session(&state, &account_id, &folder).await?;

    crate::mail::imap::move_message(&mut session, &mail_uid, &target_folder)
        .await
        .map_err(map_err_str("Failed to move message"))?;

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
            .map_err(map_err_str("DB sweep failed"))?
    };

    let count = uids.len();

    // 2. Delete on IMAP
    if !uids.is_empty() {
        let (mut session, _account) = get_imap_session(&state, &account_id, &folder).await?;

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
