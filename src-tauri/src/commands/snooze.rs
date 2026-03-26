//! Snooze commands.

use tauri::State;

use crate::app_state::lock_or_recover;
use crate::AppState;

/// Snooze a mail until a given ISO 8601 datetime.
#[tauri::command]
pub async fn snooze_mail(
    state: State<'_, AppState>,
    mail_id: String,
    until: String,
) -> Result<(), String> {
    tracing::info!("snooze_mail: {} until {}", mail_id, until);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.update_snoozed_until(&mail_id, Some(&until))
        .map_err(|e| format!("Failed to snooze mail: {}", e))
}

/// Unsnooze a mail (remove snoozed_until).
#[tauri::command]
pub async fn unsnooze_mail(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<(), String> {
    tracing::debug!("unsnooze_mail: {}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.update_snoozed_until(&mail_id, None)
        .map_err(|e| format!("Failed to unsnooze mail: {}", e))
}

/// Get all snoozed mails that are due (snoozed_until <= now).
#[tauri::command]
pub async fn get_due_snoozed(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.get_due_snoozed_ids()
        .map_err(|e| format!("Failed to get snoozed mails: {}", e))
}
