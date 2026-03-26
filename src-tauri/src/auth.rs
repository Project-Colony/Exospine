//! OAuth2 token refresh helper for IMAP auth failures.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::app_state::{self, Account};
use crate::config::Config;

/// Prevents concurrent token refreshes for the same account.
/// Multiple tasks waiting on the same account will serialize;
/// the first to finish stores the new token and the rest will
/// find it in the credential store, avoiding double-refresh.
pub static TOKEN_REFRESH_LOCKS: LazyLock<Mutex<HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn get_token_refresh_lock(account_id: &str) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    let mut map = TOKEN_REFRESH_LOCKS.lock().unwrap_or_else(|e| e.into_inner());
    map.entry(account_id.to_string())
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

/// Try to refresh an OAuth2 token for the given account.
/// Returns the new access token on success, or an error message.
/// Also stores the rotated refresh token if one is returned.
/// Uses a per-account lock to prevent concurrent double-refreshes.
pub async fn try_refresh_oauth_token(
    account: &Account,
    config: &Config,
) -> Result<String, String> {
    // Acquire per-account lock — if another task is already refreshing,
    // we wait for it and then return the already-refreshed credential.
    let lock = get_token_refresh_lock(&account.id);
    let _guard = lock.lock().await;
    let refresh_token = match &account.auth_method {
        app_state::AuthMethod::OAuth2 { refresh_token } => refresh_token.clone(),
        _ => return Err("Account is not OAuth2".to_string()),
    };

    let oauth_cfg = crate::accounts::provider::oauth2_config_for_account(account, config)
        .ok_or_else(|| "Account is not OAuth2 or provider not supported".to_string())?;

    let token_resp = crate::accounts::oauth2::refresh_token(&oauth_cfg, &refresh_token)
        .await
        .map_err(|e| format!("Token refresh failed: {}", e))?;

    // Store the new access token
    app_state::store_credential(&account.id, &token_resp.access_token);

    // If the provider rotated the refresh token, persist it
    if let Some(ref new_rt) = token_resp.refresh_token {
        if *new_rt != refresh_token {
            tracing::info!("OAuth2 refresh token rotated for {}", account.email);
            // Update the in-memory account list and persist to disk
            let mut accounts = crate::config::load_accounts();
            if let Some(acct) = accounts.iter_mut().find(|a| a.id == account.id) {
                acct.auth_method = app_state::AuthMethod::OAuth2 {
                    refresh_token: new_rt.clone(),
                };
                crate::config::save_accounts(&accounts);
                tracing::info!("Persisted rotated refresh token for {}", account.email);
            }
        }
    }

    Ok(token_resp.access_token)
}
