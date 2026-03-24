//! Tauri commands for application settings.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_state::{lock_or_recover, AppSettings};
use crate::AppState;

/// Read the current settings.
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    tracing::debug!("get_settings");
    let settings = lock_or_recover(&state.settings);
    Ok(settings.clone())
}

/// Update and persist settings.
#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), String> {
    let new_settings = settings;
    tracing::info!("save_settings: theme={}, font_size={}", new_settings.theme, new_settings.font_size);

    // Update runtime state
    {
        let mut settings = lock_or_recover(&state.settings);
        *settings = new_settings.clone();
    }

    // Sync relevant fields into the config file
    {
        let mut config = lock_or_recover(&state.config);
        config.theme = new_settings.theme;
        config.font_size = new_settings.font_size;
        config.check_interval_secs = new_settings.check_interval_secs;
        config.show_notifications = new_settings.show_notifications;
        config.reading_pane = new_settings.reading_pane;
        config.density = new_settings.density;
        config.language = new_settings.language;
        config.save().map_err(|e| format!("Failed to save config: {}", e))?;
    }

    tracing::info!("save_settings: persisted to config file");
    Ok(())
}

/// Response for get_signature.
#[derive(Debug, Clone, Serialize)]
pub struct SignatureResponse {
    pub signature: String,
    pub signature_html: Option<String>,
}

/// Get the email signature for an account.
#[tauri::command]
pub async fn get_signature(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<SignatureResponse, String> {
    tracing::debug!("get_signature: account_id={}", account_id);
    let accounts = lock_or_recover(&state.accounts);
    let account = accounts
        .iter()
        .find(|a| a.id == account_id)
        .ok_or_else(|| format!("Account not found: {}", account_id))?;
    Ok(SignatureResponse {
        signature: account.signature.clone(),
        signature_html: account.signature_html.clone(),
    })
}

/// Parameters for save_signature.
#[derive(Debug, Deserialize)]
pub struct SaveSignatureParams {
    pub account_id: String,
    pub signature: String,
    pub signature_html: Option<String>,
}

/// Save the email signature for an account and persist to accounts.json.
#[tauri::command]
pub async fn save_signature(
    state: State<'_, AppState>,
    params: SaveSignatureParams,
) -> Result<(), String> {
    tracing::info!("save_signature: account_id={}", params.account_id);
    {
        let mut accounts = lock_or_recover(&state.accounts);
        let account = accounts
            .iter_mut()
            .find(|a| a.id == params.account_id)
            .ok_or_else(|| format!("Account not found: {}", params.account_id))?;
        account.signature = params.signature;
        account.signature_html = params.signature_html;
        crate::config::save_accounts(&accounts);
    }
    tracing::info!("save_signature: persisted");
    Ok(())
}
