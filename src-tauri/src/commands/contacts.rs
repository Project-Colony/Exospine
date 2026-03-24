//! Tauri commands for contact management.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_state::lock_or_recover;
use crate::AppState;

/// A contact entry returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactEntry {
    pub email: String,
    pub name: String,
    pub frequency: u32,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub company: String,
    #[serde(default)]
    pub notes: String,
}

/// Search contacts by name or email prefix.
#[tauri::command]
pub async fn search_contacts(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<ContactEntry>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;

    let contacts = db
        .search_contacts(&query)
        .map_err(|e| format!("Failed to search contacts: {}", e))?;

    Ok(contacts
        .into_iter()
        .map(|(email, name, freq)| ContactEntry {
            email,
            name,
            frequency: freq,
            phone: String::new(),
            company: String::new(),
            notes: String::new(),
        })
        .collect())
}

/// Get all contacts.
#[tauri::command]
pub async fn get_all_contacts(
    state: State<'_, AppState>,
) -> Result<Vec<ContactEntry>, String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;

    let contacts = db
        .get_all_contacts()
        .map_err(|e| format!("Failed to get contacts: {}", e))?;

    Ok(contacts
        .into_iter()
        .map(|(email, name, freq, phone, company, notes)| ContactEntry {
            email,
            name,
            frequency: freq,
            phone,
            company,
            notes,
        })
        .collect())
}

/// Update a contact's details.
#[tauri::command]
pub async fn update_contact(
    state: State<'_, AppState>,
    email: String,
    name: String,
    phone: String,
    company: String,
    notes: String,
) -> Result<(), String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;

    db.update_contact(&email, &name, &phone, &company, &notes)
        .map_err(|e| format!("Failed to update contact: {}", e))?;

    Ok(())
}

/// Delete a contact.
#[tauri::command]
pub async fn delete_contact(
    state: State<'_, AppState>,
    email: String,
) -> Result<(), String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;

    db.delete_contact(&email)
        .map_err(|e| format!("Failed to delete contact: {}", e))?;

    Ok(())
}

/// Get total unread count across all INBOX folders.
#[tauri::command]
pub async fn get_unread_count(
    state: State<'_, AppState>,
) -> Result<u32, String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;

    db.count_unread()
        .map_err(|e| format!("Failed to count unread: {}", e))
}
