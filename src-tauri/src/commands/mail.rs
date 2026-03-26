//! Tauri commands for mail operations.
//!
//! This module is a facade that re-exports all mail-related commands
//! from their respective sub-modules.

use tauri::State;

use crate::app_state::{lock_or_recover, Account};
use crate::AppState;

/// Helper: get account by ID from state.
pub(crate) fn get_account(state: &State<'_, AppState>, account_id: &str) -> Result<Account, String> {
    let accounts = lock_or_recover(&state.accounts);
    accounts
        .iter()
        .find(|a| a.id == account_id)
        .cloned()
        .ok_or_else(|| format!("Account not found: {}", account_id))
}

// ── Re-exports from sub-modules ─────────────────────────────────────
// These re-exports allow other crate code to use `commands::mail::X` paths.
// The `generate_handler!` macro needs direct module paths instead.

#[allow(unused_imports)]
pub use super::mail_folders::get_folders;
#[allow(unused_imports)]
pub use super::mail_read::{get_mail_body, get_mail_headers, get_mails, open_eml_file, MailBodyResponse};
#[allow(unused_imports)]
pub use super::mail_sync::{refresh_folder, start_idle, sync_all_mails};
#[allow(unused_imports)]
pub use super::mail_flags::{archive_mail, delete_mail, mark_read, mark_unread, toggle_star};
#[allow(unused_imports)]
pub use super::mail_move::{move_mail, sweep_sender};
#[allow(unused_imports)]
pub use super::mail_search::search_local;
#[allow(unused_imports)]
pub use super::mail_utils::{
    add_category, cleanup_old_messages, download_attachment, export_mail, find_duplicates,
    get_analytics, get_note, get_security_log, remove_category, report_not_spam, report_spam,
    save_note, DuplicatePair,
};
