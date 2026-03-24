//! Per-account rate limiter for IMAP operations.
//!
//! Limits concurrent IMAP operations to 3 per account using a tokio Semaphore.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use tokio::sync::Semaphore;
use std::sync::Arc;

/// Maximum concurrent IMAP operations per account.
const MAX_CONCURRENT_OPS: usize = 3;

/// Global map of account_id -> Semaphore.
static RATE_LIMITERS: LazyLock<Mutex<HashMap<String, Arc<Semaphore>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Get (or create) the rate limiter semaphore for a given account.
pub fn get_limiter(account_id: &str) -> Arc<Semaphore> {
    let mut map = crate::app_state::lock_or_recover(&RATE_LIMITERS);
    map.entry(account_id.to_string())
        .or_insert_with(|| Arc::new(Semaphore::new(MAX_CONCURRENT_OPS)))
        .clone()
}
