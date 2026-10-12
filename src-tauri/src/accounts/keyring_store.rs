//! Secure credential storage backed by the OS keyring.
//!
//! Each account has up to two entries under the service "exospine": its
//! password (user = account id) and, for OAuth2 accounts, its refresh token
//! (user = "<account id>:refresh-token"). OAuth2 access tokens expire within
//! the hour and are only kept in memory.

use anyhow::{Context, Result};

const SERVICE_NAME: &str = "exospine";

fn entry(user: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE_NAME, user).context("Failed to create keyring entry")
}

fn refresh_token_user(account_id: &str) -> String {
    format!("{account_id}:refresh-token")
}

pub fn store_password(account_id: &str, password: &str) -> Result<()> {
    entry(account_id)?
        .set_password(password)
        .context("Failed to store password in keyring.")?;

    tracing::info!("Password stored in keyring for account {}", account_id);
    Ok(())
}

pub fn get_password(account_id: &str) -> Result<String> {
    entry(account_id)?
        .get_password()
        .context("Failed to retrieve password from keyring.")
}

/// Store the OAuth2 refresh token of an account, replacing any previous one.
///
/// The token is kept as raw UTF-8 bytes: Windows caps a credential at 2560
/// bytes, and `set_password` would spend two of them on every character of a
/// long Microsoft token.
pub fn store_refresh_token(account_id: &str, token: &str) -> Result<()> {
    entry(&refresh_token_user(account_id))?
        .set_secret(token.as_bytes())
        .context("Failed to store refresh token in keyring.")?;

    tracing::info!("Refresh token stored in keyring for account {}", account_id);
    Ok(())
}

pub fn get_refresh_token(account_id: &str) -> Result<String> {
    let secret = entry(&refresh_token_user(account_id))?
        .get_secret()
        .context("Failed to retrieve refresh token from keyring.")?;
    String::from_utf8(secret).context("Stored refresh token is not valid UTF-8")
}

/// Delete every keyring entry of an account. A missing entry is not an error.
pub fn delete_account_secrets(account_id: &str) {
    for user in [account_id.to_owned(), refresh_token_user(account_id)] {
        let deleted = entry(&user).and_then(|entry| match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        });
        if let Err(e) = deleted {
            tracing::warn!("Failed to delete keyring entry {}: {:#}", user, e);
        }
    }
}
