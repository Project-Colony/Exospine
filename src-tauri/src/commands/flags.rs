//! Flag follow-up commands.

use tauri::State;

use crate::app_state::lock_or_recover;
use crate::AppState;

/// Flag a mail with an optional due date for follow-up.
#[tauri::command]
pub async fn flag_mail(
    state: State<'_, AppState>,
    mail_id: String,
    due_date: Option<String>,
) -> Result<(), String> {
    tracing::debug!("flag_mail: {} due={:?}", mail_id, due_date);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.update_flag_due_date(&mail_id, due_date.as_deref())
        .map_err(|e| format!("Failed to flag mail: {}", e))
}

/// Unflag a mail (remove flag due date).
#[tauri::command]
pub async fn unflag_mail(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<(), String> {
    tracing::debug!("unflag_mail: {}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.update_flag_due_date(&mail_id, None)
        .map_err(|e| format!("Failed to unflag mail: {}", e))
}
