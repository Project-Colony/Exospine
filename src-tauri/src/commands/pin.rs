//! Pin commands.

use tauri::State;

use crate::app_state::lock_or_recover;
use crate::AppState;

/// Pin a mail so it stays at the top of the list.
#[tauri::command]
pub async fn pin_mail(state: State<'_, AppState>, mail_id: String) -> Result<(), String> {
    tracing::debug!("pin_mail: {}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.update_pinned_status(&mail_id, true)
        .map_err(|e| format!("Failed to pin mail: {}", e))
}

/// Unpin a mail.
#[tauri::command]
pub async fn unpin_mail(state: State<'_, AppState>, mail_id: String) -> Result<(), String> {
    tracing::debug!("unpin_mail: {}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.update_pinned_status(&mail_id, false)
        .map_err(|e| format!("Failed to unpin mail: {}", e))
}
