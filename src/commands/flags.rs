//! Flag follow-up commands.

use tauri::State;

use crate::app_state::{map_err_str, with_db};
use crate::AppState;

/// Flag a mail with an optional due date for follow-up.
#[tauri::command]
pub async fn flag_mail(
    state: State<'_, AppState>,
    mail_id: String,
    due_date: Option<String>,
) -> Result<(), String> {
    tracing::debug!("flag_mail: {} due={:?}", mail_id, due_date);
    with_db(&state, |db| {
        db.update_flag_due_date(&mail_id, due_date.as_deref())
            .map_err(map_err_str("Failed to flag mail"))
    })
}

/// Unflag a mail (remove flag due date).
#[tauri::command]
pub async fn unflag_mail(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<(), String> {
    tracing::debug!("unflag_mail: {}", mail_id);
    with_db(&state, |db| {
        db.update_flag_due_date(&mail_id, None)
            .map_err(map_err_str("Failed to unflag mail"))
    })
}
