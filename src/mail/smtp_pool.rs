//! SMTP connection pool — caches SmtpTransport per account to avoid
//! re-establishing TLS connections on every send.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

use anyhow::{Context, Result};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, Tokio1Executor};

use crate::app_state::{lock_or_recover, Account};

/// A pooled SMTP transport with a creation timestamp for staleness checks.
struct PoolEntry {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    created_at: Instant,
}

/// Global SMTP transport pool: account_id -> PoolEntry.
static SMTP_POOL: LazyLock<Mutex<HashMap<String, PoolEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Maximum age before a pooled transport is considered stale (5 minutes).
const MAX_AGE_SECS: u64 = 300;

/// Get or create an SMTP transport for the given account.
///
/// If a cached transport exists and is fresh enough, it is reused.
/// Otherwise a new transport is built and cached.
pub fn get_transport(
    account: &Account,
    password: &str,
) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    // Check pool for a fresh transport
    {
        let mut pool = lock_or_recover(&SMTP_POOL);
        if let Some(entry) = pool.get(&account.id) {
            if entry.created_at.elapsed().as_secs() < MAX_AGE_SECS {
                tracing::debug!(
                    account = %account.email,
                    "Reusing pooled SMTP transport (age={}s)",
                    entry.created_at.elapsed().as_secs()
                );
                // Clone the transport — lettre's AsyncSmtpTransport is cheap to clone
                // as it wraps an Arc internally
                return Ok(entry.transport.clone());
            } else {
                tracing::debug!(
                    account = %account.email,
                    "Pooled SMTP transport stale, creating new one"
                );
                pool.remove(&account.id);
            }
        }
    }

    // Build a new transport
    let creds = Credentials::new(account.username.clone(), password.to_string());

    let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&account.smtp_host)
        .context("Failed to create SMTP transport")?
        .credentials(creds)
        .port(account.smtp_port)
        .build();

    // Cache it
    {
        let mut pool = lock_or_recover(&SMTP_POOL);
        pool.insert(
            account.id.clone(),
            PoolEntry {
                transport: transport.clone(),
                created_at: Instant::now(),
            },
        );
    }

    Ok(transport)
}

/// Evict a cached SMTP transport for the given account (e.g. when account is removed
/// or credentials change).
#[allow(dead_code)]
pub fn evict_transport(account_id: &str) {
    let mut pool = lock_or_recover(&SMTP_POOL);
    pool.remove(account_id);
}

/// Evict all cached SMTP transports.
#[allow(dead_code)]
pub fn evict_all() {
    let mut pool = lock_or_recover(&SMTP_POOL);
    pool.clear();
}
