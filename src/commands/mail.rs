//! Helpers shared by the mail commands.

use tauri::State;

use crate::app_state::Account;
use crate::app_state::AppState;

/// Helper: get account by ID from state.
/// Delegates to `app_state::get_account` so all modules share one implementation.
pub(crate) fn get_account(state: &State<'_, AppState>, account_id: &str) -> Result<Account, String> {
    crate::app_state::get_account(state, account_id)
}
