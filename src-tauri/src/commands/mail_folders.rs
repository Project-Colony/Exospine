//! Folder-related mail commands.

use std::collections::HashMap;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

use tauri::State;
use tokio::time::timeout;

use crate::app_state::{fetch_password, lock_or_recover, map_err_str, Folder};
use crate::AppState;

use super::mail::get_account;

/// Timeout for fetching folder list from IMAP.
const FOLDER_FETCH_TIMEOUT_SECS: u64 = 30;

// ── Folder cache TTL (5 minutes) ────────────────────────────────────
static FOLDER_CACHE_TS: std::sync::LazyLock<StdMutex<HashMap<String, Instant>>> =
    std::sync::LazyLock::new(|| StdMutex::new(HashMap::new()));

const FOLDER_CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes

/// Fetch folder list for an account.
/// Returns cached folders if available and less than 5 minutes old; otherwise refreshes from IMAP.
#[tauri::command]
pub async fn get_folders(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Vec<Folder>, String> {
    tracing::debug!("get_folders: account_id={}", account_id);

    // 1. Check if we already have cached folders for this account within TTL
    let cached_folders = {
        let accounts = lock_or_recover(&state.accounts);
        accounts
            .iter()
            .find(|a| a.id == account_id)
            .and_then(|a| {
                if a.folders.is_empty() {
                    None
                } else {
                    Some(a.folders.clone())
                }
            })
    };

    if let Some(ref folders) = cached_folders {
        // Check TTL: if folders were fetched less than 5 min ago, return cached
        let within_ttl = FOLDER_CACHE_TS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&account_id)
            .map(|ts| ts.elapsed() < FOLDER_CACHE_TTL)
            .unwrap_or(false);

        if within_ttl {
            tracing::debug!("get_folders: returning {} cached folders (within TTL)", folders.len());
            return Ok(folders.clone());
        }
    }

    // 2. Cache missing or TTL expired — fetch from IMAP
    let account = get_account(&state, &account_id)?;
    let password = fetch_password(&account_id).await?;

    let mut session = crate::mail::connection::get_session(&account_id, &account, &password)
        .await
        .map_err(map_err_str("Failed to connect to IMAP"))?;

    let folders = timeout(
        Duration::from_secs(FOLDER_FETCH_TIMEOUT_SECS),
        crate::mail::folders::fetch_folders_with_counts(&mut session),
    )
    .await
    .map_err(|_| "Timeout: folder fetch took longer than 30 seconds".to_string())?
    .map_err(map_err_str("Failed to fetch folders"))?;

    crate::mail::connection::return_session(&account_id, session);

    // 3. Cache folders in account state, persist, and record TTL timestamp
    {
        let mut accounts = lock_or_recover(&state.accounts);
        if let Some(acct) = accounts.iter_mut().find(|a| a.id == account_id) {
            acct.folders = folders.clone();
        }
        crate::config::save_accounts(&accounts);
    }
    FOLDER_CACHE_TS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(account_id.clone(), Instant::now());

    tracing::info!("get_folders: fetched {} folders from IMAP", folders.len());
    Ok(folders)
}
