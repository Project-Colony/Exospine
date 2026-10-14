//! Tauri commands for account management.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::accounts::provider::Provider;
use crate::app_state::{lock_or_recover, map_err_str, remove_credential, store_credential, Account};
use crate::app_state::AppState;

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

/// Return the list of configured accounts. `Account` holds no secret:
/// passwords and refresh tokens stay in the OS keyring.
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

/// Take the problem met while reading accounts.json at startup, if any.
/// The UI asks once it is ready: an event sent before its listeners exist
/// would be lost.
#[tauri::command]
pub fn take_accounts_problem(
    state: State<'_, AppState>,
) -> Option<crate::config::AccountsProblem> {
    lock_or_recover(&state.accounts_problem).take()
}

/// Run the OAuth2 sign-in for a provider and add the account.
/// The tokens never reach the webview: the refresh token goes to the OS
/// keyring, the access token to memory, and only the account is returned.
#[tauri::command]
pub async fn start_oauth(
    state: State<'_, AppState>,
    provider: String,
) -> Result<Account, String> {
    tracing::info!("start_oauth: provider={}", provider);
    crate::config::ensure_accounts_writable()?;

    let prov = match provider.to_lowercase().as_str() {
        "gmail" | "google" => Provider::Gmail,
        "outlook" | "microsoft" | "hotmail" | "exchange" | "office365" => Provider::Outlook,
        _ => return Err(format!("Unsupported OAuth provider: {}", provider)),
    };

    let (client_id, client_secret) = {
        let config = lock_or_recover(&state.config);
        match prov {
            Provider::Gmail => {
                (config.google_client_id.clone(), config.google_client_secret.clone())
            }
            Provider::Outlook => {
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

    let userinfo = match prov {
        Provider::Gmail => {
            crate::accounts::oauth2::fetch_google_userinfo(&token.access_token).await
        }
        _ => crate::accounts::oauth2::microsoft_userinfo_from_id_token(
            token.id_token.as_deref().unwrap_or_default(),
        ),
    };
    let (email, name) = match userinfo {
        Ok(info) if !info.email.is_empty() => (info.email, info.name),
        other => {
            if let Err(e) = other {
                tracing::error!("start_oauth: cannot read the account identity: {:#}", e);
            }
            return Err("The provider did not return the account email address.".to_string());
        }
    };

    crate::storage::audit_log::log_event(
        "OAUTH_LOGIN",
        &format!("email={}, provider={}", email, provider),
    );

    let servers = crate::accounts::provider::config_for(prov);
    let account = crate::accounts::setup::create_oauth_account(
        crate::accounts::setup::OAuthAccountParams {
            name: &name,
            email: &email,
            refresh_token: token.refresh_token.as_deref().unwrap_or_default(),
            imap_host: servers.imap_host,
            imap_port: servers.imap_port,
            smtp_host: servers.smtp_host,
            smtp_port: servers.smtp_port,
        },
    )
    .map_err(map_err_str("Failed to create OAuth account"))?;

    store_credential(&account.id, &token.access_token);
    register_account(&state, &account);

    tracing::info!("start_oauth: account {} created for {}", account.id, email);
    Ok(account)
}

/// Add a new account to the running state, accounts.json and the database.
fn register_account(state: &AppState, account: &Account) {
    {
        let mut accounts = lock_or_recover(&state.accounts);
        accounts.push(account.clone());
        crate::config::save_accounts(&accounts);
    }

    let db_guard = lock_or_recover(&state.db);
    if let Some(ref db) = *db_guard {
        if let Err(e) = db.save_account(account) {
            tracing::error!("Failed to save account {} to the database: {}", account.id, e);
        }
    }
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
    crate::config::ensure_accounts_writable()?;

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
    .map_err(map_err_str("Failed to create account"))?;

    // Store password in memory
    store_credential(&account.id, &params.password);
    register_account(&state, &account);

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
    crate::config::ensure_accounts_writable()?;

    // Delete from keyring
    crate::accounts::keyring_store::delete_account_secrets(&account_id);

    // Clean up in-memory credentials
    remove_credential(&account_id);

    // Evict any pooled IMAP session and SMTP transport, which hold its credentials
    crate::mail::connection::evict_session(&account_id).await;
    crate::mail::smtp_pool::evict_transport(&account_id);

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

    imap_result.map_err(map_err_str("IMAP connection failed"))?;
    smtp_result.map_err(map_err_str("SMTP connection failed"))?;

    tracing::info!("test_connection: both IMAP and SMTP passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AuthMethod;

    /// `get_accounts` hands `Account` to the webview as is.
    #[test]
    fn account_sent_to_webview_has_no_secret() {
        let account = Account {
            id: "acct-1".into(),
            email: "ada@example.com".into(),
            auth_method: AuthMethod::OAuth2,
            ..Default::default()
        };
        let json = serde_json::to_string(&account).unwrap().to_lowercase();
        assert!(!json.contains("token") && !json.contains("password") && !json.contains("secret"));
    }

    #[test]
    fn microsoft_identity_comes_from_the_id_token() {
        use base64::Engine as _;
        let encode = |json: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json);
        let jwt = |claims: &str| format!("{}.{}.sig", encode(r#"{"alg":"RS256"}"#), encode(claims));
        let from = |claims: &str| {
            crate::accounts::oauth2::microsoft_userinfo_from_id_token(&jwt(claims)).unwrap()
        };

        let info = from(r#"{"email":"ada@contoso.com","preferred_username":"ada@x.onmicrosoft.com","name":"Ada"}"#);
        assert_eq!((info.email.as_str(), info.name.as_str()), ("ada@contoso.com", "Ada"));
        // Work accounts often send no email claim.
        assert_eq!(from(r#"{"preferred_username":"ada@contoso.com"}"#).email, "ada@contoso.com");
        // A phone number sign-in is not a mailbox address.
        assert!(from(r#"{"preferred_username":"+15551234567"}"#).email.is_empty());
        assert!(crate::accounts::oauth2::microsoft_userinfo_from_id_token("").is_err());
    }
}
