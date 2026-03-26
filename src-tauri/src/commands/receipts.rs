//! Read receipt commands.

use tauri::State;

use crate::app_state::{fetch_password, lock_or_recover, ComposeDraft};
use crate::AppState;

/// Send a read receipt (MDN) for a mail.
#[tauri::command]
pub async fn send_read_receipt(
    state: State<'_, AppState>,
    mail_id: String,
    account_id: String,
    receipt_to: String,
    original_subject: String,
    original_from: String,
) -> Result<(), String> {
    tracing::info!("send_read_receipt: mail={} to={}", mail_id, receipt_to);

    let account = {
        let accounts = lock_or_recover(&state.accounts);
        accounts
            .iter()
            .find(|a| a.id == account_id)
            .cloned()
            .ok_or_else(|| format!("Account not found: {}", account_id))?
    };

    let password = fetch_password(&account.id).await?;

    // Build a simple MDN message
    let draft = ComposeDraft {
        to: receipt_to,
        cc: String::new(),
        bcc: String::new(),
        subject: format!("Read: {}", original_subject),
        body: format!(
            "Your message to {} with subject \"{}\" was displayed on {}.",
            original_from,
            original_subject,
            chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"),
        ),
        body_html: None,
        reply_to: None,
        account_id: account_id.clone(),
        attachments: Vec::new(),
        importance: "normal".to_string(),
    };

    crate::mail::smtp::send_mail(&account, &password, &draft, None)
        .await
        .map_err(|e| format!("Failed to send read receipt: {}", e))?;

    Ok(())
}
