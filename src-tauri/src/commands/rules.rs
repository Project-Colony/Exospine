//! Tauri commands for email rules/filters.

use crate::mail::rules::{self, EmailRule};

/// Get all rules.
#[tauri::command]
pub async fn get_rules() -> Result<Vec<EmailRule>, String> {
    Ok(rules::load_rules())
}

/// Save (create or update) a rule.
#[tauri::command]
pub async fn save_rule(rule: EmailRule) -> Result<(), String> {
    let mut rules = rules::load_rules();
    if let Some(existing) = rules.iter_mut().find(|r| r.id == rule.id) {
        *existing = rule;
    } else {
        rules.push(rule);
    }
    rules::save_rules(&rules)
}

/// Delete a rule by ID.
#[tauri::command]
pub async fn delete_rule(rule_id: String) -> Result<(), String> {
    let mut rules = rules::load_rules();
    rules.retain(|r| r.id != rule_id);
    rules::save_rules(&rules)
}
