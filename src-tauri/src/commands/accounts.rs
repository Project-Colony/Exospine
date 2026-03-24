//! Tauri commands for account management.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_state::{lock_or_recover, store_credential, Account};
use crate::AppState;

/// Serializable provider detection result for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub provider_name: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub auth_method: String,
}

/// Return the list of configured accounts.
#[tauri::command]
pub async fn get_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, String> {
    tracing::debug!("get_accounts");
    let accounts = lock_or_recover(&state.accounts);
    Ok(accounts.clone())
}

/// Auto-detect provider settings from an email address.
#[tauri::command]
pub async fn detect_provider(email: String) -> Result<ProviderInfo, String> {
    tracing::debug!("detect_provider: email={}", email);
    let cfg = crate::accounts::provider::detect_provider(&email);
    Ok(ProviderInfo {
        provider_name: cfg.provider.label().to_string(),
        imap_host: cfg.imap_host.to_string(),
        imap_port: cfg.imap_port,
        smtp_host: cfg.smtp_host.to_string(),
        smtp_port: cfg.smtp_port,
        auth_method: format!("{:?}", cfg.auth_method),
    })
}

/// Begin an OAuth2 flow for a given provider.
/// Returns the access_token, refresh_token, and user email/name.
#[derive(Debug, Serialize)]
pub struct OAuthResult {
    pub access_token: String,
    pub refresh_token: String,
    pub email: String,
    pub name: String,
}

#[tauri::command]
pub async fn start_oauth(
    state: State<'_, AppState>,
    provider: String,
) -> Result<OAuthResult, String> {
    tracing::info!("start_oauth: provider={}", provider);

    let prov = match provider.to_lowercase().as_str() {
        "gmail" | "google" => crate::accounts::provider::Provider::Gmail,
        "outlook" | "microsoft" | "hotmail" | "exchange" | "office365" => {
            crate::accounts::provider::Provider::Outlook
        }
        _ => return Err(format!("Unsupported OAuth provider: {}", provider)),
    };

    let (client_id, client_secret) = {
        let config = lock_or_recover(&state.config);
        match prov {
            crate::accounts::provider::Provider::Gmail => {
                (config.google_client_id.clone(), config.google_client_secret.clone())
            }
            crate::accounts::provider::Provider::Outlook => {
                (config.microsoft_client_id.clone(), config.microsoft_client_secret.clone())
            }
            _ => return Err("Unsupported provider".to_string()),
        }
    };

    if client_id.is_empty() {
        return Err(format!(
            "No OAuth client ID configured for {}. Set it in config.toml.",
            provider
        ));
    }

    let token = crate::accounts::oauth2_flow::start_oauth_flow(prov, client_id, client_secret)
        .await
        .map_err(|e| {
            crate::storage::audit_log::log_event(
                "OAUTH_FAIL",
                &format!("provider={}, reason={}", provider, e),
            );
            format!("OAuth flow failed: {}", e)
        })?;

    let refresh_token = token.refresh_token.clone().unwrap_or_default();

    // Fetch user info from the provider's userinfo endpoint
    let (email, name) = match prov {
        crate::accounts::provider::Provider::Gmail => {
            match crate::accounts::oauth2::fetch_google_userinfo(&token.access_token).await {
                Ok(info) => (info.email, info.name),
                Err(_) => (String::new(), String::new()),
            }
        }
        crate::accounts::provider::Provider::Outlook => {
            // Microsoft Graph /me endpoint works for both consumer Outlook and Exchange Online / Office 365
            match crate::accounts::oauth2::fetch_microsoft_userinfo(&token.access_token).await {
                Ok(info) => (info.email, info.name),
                Err(_) => (String::new(), String::new()),
            }
        }
        _ => (String::new(), String::new()),
    };

    crate::storage::audit_log::log_event(
        "OAUTH_LOGIN",
        &format!("email={}, provider={}", email, provider),
    );

    tracing::info!("start_oauth: completed for email={}", email);
    Ok(OAuthResult {
        access_token: token.access_token,
        refresh_token,
        email,
        name,
    })
}

/// Finalize adding an OAuth account.
#[derive(Debug, Deserialize)]
pub struct AddOAuthAccountParams {
    pub name: String,
    pub email: String,
    pub access_token: String,
    pub refresh_token: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
}

#[tauri::command]
pub async fn add_oauth_account(
    state: State<'_, AppState>,
    params: AddOAuthAccountParams,
) -> Result<Account, String> {
    tracing::info!("add_oauth_account: email={}", params.email);

    let account = crate::accounts::setup::create_oauth_account(
        &params.name,
        &params.email,
        &params.access_token,
        &params.refresh_token,
        &params.imap_host,
        params.imap_port,
        &params.smtp_host,
        params.smtp_port,
    )
    .await
    .map_err(|e| format!("Failed to create OAuth account: {}", e))?;

    // Store access token in memory too
    store_credential(&account.id, &params.access_token);

    // Add to state and persist
    {
        let mut accounts = lock_or_recover(&state.accounts);
        accounts.push(account.clone());
        crate::config::save_accounts(&accounts);
    }

    // Save to DB
    {
        let db_guard = lock_or_recover(&state.db);
        if let Some(ref db) = *db_guard {
            if let Err(e) = db.save_account(&account) {
                tracing::error!("add_oauth_account: DB save failed: {}", e);
            }
        }
    }

    tracing::info!("add_oauth_account: account {} created", account.id);
    Ok(account)
}

/// Add a basic auth account.
#[derive(Debug, Deserialize)]
pub struct AddAccountParams {
    pub name: String,
    pub email: String,
    pub password: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
}

#[tauri::command]
pub async fn add_account(
    state: State<'_, AppState>,
    params: AddAccountParams,
) -> Result<Account, String> {
    tracing::info!("add_account: email={}", params.email);

    let account = crate::accounts::setup::create_account(
        &params.name,
        &params.email,
        &params.password,
        &params.imap_host,
        params.imap_port,
        &params.smtp_host,
        params.smtp_port,
    )
    .await
    .map_err(|e| format!("Failed to create account: {}", e))?;

    // Store password in memory
    store_credential(&account.id, &params.password);

    // Add to state and persist
    {
        let mut accounts = lock_or_recover(&state.accounts);
        accounts.push(account.clone());
        crate::config::save_accounts(&accounts);
    }

    // Save to DB
    {
        let db_guard = lock_or_recover(&state.db);
        if let Some(ref db) = *db_guard {
            if let Err(e) = db.save_account(&account) {
                tracing::error!("add_account: DB save failed: {}", e);
            }
        }
    }

    tracing::info!("add_account: account {} created", account.id);
    Ok(account)
}

/// Remove an account by ID.
#[tauri::command]
pub async fn remove_account(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<(), String> {
    tracing::info!("remove_account: account_id={}", account_id);

    // Delete from keyring
    let _ = crate::accounts::keyring_store::delete_password(&account_id);

    // Remove from state
    {
        let mut accounts = lock_or_recover(&state.accounts);
        accounts.retain(|a| a.id != account_id);
        crate::config::save_accounts(&accounts);
    }

    // Remove from DB
    {
        let db_guard = lock_or_recover(&state.db);
        if let Some(ref db) = *db_guard {
            if let Err(e) = db.delete_account(&account_id) {
                tracing::error!("remove_account: DB delete failed: {}", e);
            }
        }
    }

    tracing::info!("remove_account: account {} removed", account_id);
    Ok(())
}

/// Test IMAP/SMTP connection for given credentials.
#[derive(Debug, Deserialize)]
pub struct TestConnectionParams {
    pub email: String,
    pub password: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
}

#[tauri::command]
pub async fn test_connection(params: TestConnectionParams) -> Result<(), String> {
    tracing::info!("test_connection: imap={}:{}, smtp={}:{}", params.imap_host, params.imap_port, params.smtp_host, params.smtp_port);

    let (imap_result, smtp_result) = tokio::join!(
        crate::accounts::setup::test_imap_connection(
            &params.imap_host,
            params.imap_port,
            &params.email,
            &params.password,
        ),
        crate::accounts::setup::test_smtp_connection(
            &params.smtp_host,
            params.smtp_port,
            &params.email,
            &params.password,
        ),
    );

    imap_result.map_err(|e| format!("IMAP connection failed: {}", e))?;
    smtp_result.map_err(|e| format!("SMTP connection failed: {}", e))?;

    tracing::info!("test_connection: both IMAP and SMTP passed");
    Ok(())
}
