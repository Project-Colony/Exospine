//! Pin commands.

use tauri::State;

use crate::app_state::with_db;
use crate::AppState;

/// Pin a mail so it stays at the top of the list.
#[tauri::command]
pub async fn pin_mail(state: State<'_, AppState>, mail_id: String) -> Result<(), String> {
    tracing::debug!("pin_mail: {}", mail_id);
    with_db(&state, |db| {
        db.update_pinned_status(&mail_id, true)
            .map_err(|e| format!("Failed to pin mail: {}", e))
    })
}

/// Unpin a mail.
#[tauri::command]
pub async fn unpin_mail(state: State<'_, AppState>, mail_id: String) -> Result<(), String> {
    tracing::debug!("unpin_mail: {}", mail_id);
    with_db(&state, |db| {
        db.update_pinned_status(&mail_id, false)
            .map_err(|e| format!("Failed to unpin mail: {}", e))
    })
}
