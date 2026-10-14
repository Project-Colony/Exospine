//! OAuth2 token refresh helper for IMAP auth failures.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::accounts::keyring_store;
use crate::app_state::{self, Account, AppState, AuthMethod};
use crate::config::Config;

/// Get a fresh access token for every OAuth2 account in the background, so
/// that startup does not wait for the network.
pub fn refresh_all_in_background(app: &tauri::AppHandle) {
    use tauri::Manager;
    let state = app.state::<AppState>();
    let config = app_state::lock_or_recover(&state.config).clone();
    let accounts: Vec<Account> = app_state::lock_or_recover(&state.accounts)
        .iter()
        .filter(|acct| acct.auth_method == AuthMethod::OAuth2)
        .cloned()
        .collect();
    for acct in accounts {
        let config = config.clone();
        tauri::async_runtime::spawn(async move {
            match try_refresh_oauth_token(&acct, &config).await {
                Ok(_) => tracing::info!("Refreshed OAuth2 token for account {}", acct.email),
                Err(e) => tracing::error!(
                    "Failed to refresh OAuth2 token for account {}: {}",
                    acct.email,
                    e
                ),
            }
        });
    }
}

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
/// The refresh token is read from the OS keyring, and a rotated one is
/// written back there, so accounts.json never holds it.
/// Uses a per-account lock to prevent concurrent double-refreshes.
pub async fn try_refresh_oauth_token(
    account: &Account,
    config: &Config,
) -> Result<String, String> {
    // Acquire per-account lock — if another task is already refreshing,
    // we wait for it and then return the already-refreshed credential.
    let lock = get_token_refresh_lock(&account.id);
    let _guard = lock.lock().await;

    let oauth_cfg = crate::accounts::provider::oauth2_config_for_account(account, config)
        .ok_or_else(|| "Account is not OAuth2 or provider not supported".to_string())?;

    let id = account.id.clone();
    let refresh_token =
        tokio::task::spawn_blocking(move || keyring_store::get_refresh_token(&id))
            .await
            .map_err(|e| format!("Task join error: {}", e))?
            .map_err(|e| format!("No refresh token for {}: {:#}", account.email, e))?;

    let token_resp = crate::accounts::oauth2::refresh_token(&oauth_cfg, &refresh_token)
        .await
        .map_err(|e| format!("Token refresh failed: {}", e))?;

    // Store the new access token
    app_state::store_credential(&account.id, &token_resp.access_token);

    // A provider that rotates refresh tokens (Microsoft does) may revoke the
    // old one, so the new one must reach the keyring.
    if let Some(new_rt) = token_resp.refresh_token.filter(|rt| *rt != refresh_token) {
        match keyring_store::store_refresh_token(&account.id, &new_rt) {
            Ok(()) => tracing::info!("OAuth2 refresh token rotated for {}", account.email),
            Err(e) => tracing::error!(
                "Failed to store the rotated refresh token for {}: {:#}",
                account.email,
                e
            ),
        }
    }

    Ok(token_resp.access_token)
}
