//! Follow-up tracker commands.

use tauri::State;

use crate::app_state::lock_or_recover;
use crate::AppState;

/// Add a follow-up: track a mail waiting for a reply from expected_from.
#[tauri::command]
pub async fn add_followup(
    state: State<'_, AppState>,
    mail_id: String,
    expected_from: String,
    due_date: String,
) -> Result<(), String> {
    tracing::info!("add_followup: {} from={}", mail_id, expected_from);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.add_followup(&mail_id, &expected_from, &due_date)
        .map_err(|e| format!("Failed to add followup: {}", e))
}

/// Get all follow-ups.
#[tauri::command]
pub async fn get_followups(
    state: State<'_, AppState>,
) -> Result<Vec<crate::storage::db::Followup>, String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.get_followups()
        .map_err(|e| format!("Failed to get followups: {}", e))
}

/// Resolve a follow-up (mark as replied).
#[tauri::command]
pub async fn resolve_followup(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<(), String> {
    tracing::debug!("resolve_followup: {}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.resolve_followup(&mail_id)
        .map_err(|e| format!("Failed to resolve followup: {}", e))
}

/// Delete a follow-up.
#[tauri::command]
pub async fn delete_followup(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<(), String> {
    tracing::debug!("delete_followup: {}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.delete_followup(&mail_id)
        .map_err(|e| format!("Failed to delete followup: {}", e))
}

/// Check if any unresolved followups have been replied to. Returns resolved mail IDs.
#[tauri::command]
pub async fn check_followups(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Vec<String>, String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.check_resolved_followups(&account_id)
        .map_err(|e| format!("Failed to check followups: {}", e))
}
