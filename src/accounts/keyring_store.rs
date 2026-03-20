//! Secure credential storage backed by the OS keyring.
//!
//! Wraps the [`keyring`] crate to store, retrieve, and delete
//! account passwords. Falls back gracefully with a warning when
//! no keyring daemon is available (e.g. headless Linux without
//! `secret-service` or `gnome-keyring`).

use anyhow::{Context, Result};

/// Service name registered in the OS credential store.
const SERVICE_NAME: &str = "exospine";

/// Store a password for the given account identifier.
///
/// The password is written to the OS keyring under `SERVICE_NAME`
/// with `account_id` as the user key.
pub fn store_password(account_id: &str, password: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, account_id)
        .context("Failed to create keyring entry")?;

    entry
        .set_password(password)
        .context("Failed to store password in keyring. \
                  Ensure a credential store (e.g. gnome-keyring, \
                  KWallet, or macOS Keychain) is available.")?;

    tracing::debug!("Password stored for account {}", account_id);
    Ok(())
}

/// Retrieve the stored password for the given account identifier.
///
/// Returns `Err` if no password is stored or the keyring is unavailable.
pub fn get_password(account_id: &str) -> Result<String> {
    let entry = keyring::Entry::new(SERVICE_NAME, account_id)
        .context("Failed to create keyring entry")?;

    let password = entry
        .get_password()
        .context("Failed to retrieve password from keyring. \
                  The credential may not exist or the keyring daemon \
                  may be unavailable.")?;

    Ok(password)
}

/// Delete the stored password for the given account identifier.
///
/// Silently succeeds if no password was stored (the keyring crate
/// returns an error for missing entries, which we map to a warning).
pub fn delete_password(account_id: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, account_id)
        .context("Failed to create keyring entry")?;

    match entry.delete_credential() {
        Ok(()) => {
            tracing::debug!("Password deleted for account {}", account_id);
            Ok(())
        }
        Err(keyring::Error::NoEntry) => {
            tracing::warn!(
                "No keyring entry found for account {} — nothing to delete",
                account_id
            );
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!(e))
            .context("Failed to delete password from keyring"),
    }
}
