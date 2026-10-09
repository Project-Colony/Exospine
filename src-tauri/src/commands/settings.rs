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

/// Enable or disable starting Exospine with Windows via the registry.
#[tauri::command]
pub async fn set_autostart(enabled: bool) -> Result<(), String> {
    tracing::info!("set_autostart: enabled={}", enabled);
    #[cfg(target_os = "windows")]
    {
        let exe = std::env::current_exe()
            .map_err(|e| format!("Failed to get exe path: {}", e))?;
        // Quoted, so a path with spaces is not split into a program and arguments.
        let exe_str = format!("\"{}\"", exe.display());

        if enabled {
            let output = std::process::Command::new("reg")
                .args([
                    "add",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v", "Exospine",
                    "/t", "REG_SZ",
                    "/d", &exe_str,
                    "/f",
                ])
                .output()
                .map_err(|e| format!("Failed to run reg add: {}", e))?;
            if !output.status.success() {
                return Err(format!(
                    "reg add failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        } else {
            let output = std::process::Command::new("reg")
                .args([
                    "delete",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v", "Exospine",
                    "/f",
                ])
                .output()
                .map_err(|e| format!("Failed to run reg delete: {}", e))?;
            // Ignore "not found" errors when disabling
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stderr.contains("unable to find") && !stderr.contains("not found") {
                    return Err(format!("reg delete failed: {}", stderr));
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        tracing::warn!("set_autostart: not implemented on this platform");
    }
    Ok(())
}

/// Securely wipe all data: messages, accounts, config, credentials.
#[tauri::command]
pub async fn secure_wipe(state: State<'_, AppState>) -> Result<(), String> {
    tracing::warn!("secure_wipe: wiping all data!");

    // 1. Delete all keyring credentials
    {
        let accounts = lock_or_recover(&state.accounts);
        for acct in accounts.iter() {
            let _ = crate::accounts::keyring_store::delete_password(&acct.id);
            crate::app_state::remove_credential(&acct.id);
        }
    }

    // 2. Wipe SQLite database
    {
        let db_guard = lock_or_recover(&state.db);
        if let Some(ref db) = *db_guard {
            db.wipe_all().map_err(|e| format!("Failed to wipe database: {}", e))?;
        }
    }

    // 3. Clear in-memory accounts
    {
        let mut accounts = lock_or_recover(&state.accounts);
        accounts.clear();
    }

    // 4. Delete accounts.json on disk
    {
        let config = lock_or_recover(&state.config);
        let _ = config.delete_accounts_file();
    }

    // 5. Evict SMTP pool
    crate::mail::smtp_pool::evict_all();

    tracing::warn!("secure_wipe: completed");
    Ok(())
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
