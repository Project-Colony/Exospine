//! Secure credential storage backed by the OS keyring.

use anyhow::{Context, Result};

const SERVICE_NAME: &str = "exospine";

pub fn store_password(account_id: &str, password: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, account_id)
        .context("Failed to create keyring entry")?;

    entry
        .set_password(password)
        .context("Failed to store password in keyring.")?;

    tracing::info!("Password stored in keyring for account {}", account_id);
    Ok(())
}

pub fn get_password(account_id: &str) -> Result<String> {
    let entry = keyring::Entry::new(SERVICE_NAME, account_id)
        .context("Failed to create keyring entry")?;

    let password = entry
        .get_password()
        .context("Failed to retrieve password from keyring.")?;

    Ok(password)
}

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
                "No keyring entry found for account {} -- nothing to delete",
                account_id
            );
            Ok(())
        }
        Err(e) => {
            Err(anyhow::anyhow!(e)).context("Failed to delete password from keyring")
        }
    }
}
