//! Task commands.

use tauri::State;

use crate::app_state::lock_or_recover;
use crate::AppState;

/// Create a task (optionally linked to a mail).
#[tauri::command]
pub async fn create_task(
    state: State<'_, AppState>,
    title: String,
    description: String,
    mail_id: String,
    due_date: String,
) -> Result<String, String> {
    tracing::info!("create_task: {}", title);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.create_task(&title, &description, &mail_id, &due_date)
        .map_err(|e| format!("Failed to create task: {}", e))
}

/// Get all tasks.
#[tauri::command]
pub async fn get_tasks(
    state: State<'_, AppState>,
) -> Result<Vec<crate::storage::db::Task>, String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.get_tasks()
        .map_err(|e| format!("Failed to get tasks: {}", e))
}

/// Complete a task.
#[tauri::command]
pub async fn complete_task(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    tracing::debug!("complete_task: {}", id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.complete_task(&id)
        .map_err(|e| format!("Failed to complete task: {}", e))
}

/// Delete a task.
#[tauri::command]
pub async fn delete_task(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    tracing::debug!("delete_task: {}", id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.delete_task(&id)
        .map_err(|e| format!("Failed to delete task: {}", e))
}
