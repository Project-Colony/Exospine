//! Tauri commands for composing and sending mail.

use tauri::State;

use crate::app_state::{fetch_password, lock_or_recover, ComposeDraft};
use crate::AppState;

/// Send a composed email via SMTP.
#[tauri::command]
pub async fn send_mail(
    state: State<'_, AppState>,
    draft: ComposeDraft,
) -> Result<(), String> {
    tracing::info!("send_mail: to={}, subject={}", draft.to, draft.subject);

    let account = {
        let accounts = lock_or_recover(&state.accounts);
        accounts
            .iter()
            .find(|a| a.id == draft.account_id)
            .cloned()
            .ok_or_else(|| format!("Account not found: {}", draft.account_id))?
    };

    let password = fetch_password(account.id.clone()).await?;

    let reply_to = draft.reply_to.as_deref();

    crate::mail::smtp::send_mail(&account, &password, &draft, reply_to)
        .await
        .map_err(|e| format!("Failed to send email: {}", e))?;

    tracing::info!("send_mail: completed successfully");
    Ok(())
}

/// Save a draft to the IMAP Drafts folder.
#[tauri::command]
pub async fn save_draft(
    state: State<'_, AppState>,
    draft: ComposeDraft,
) -> Result<(), String> {
    tracing::info!("save_draft: subject={}", draft.subject);

    let account = {
        let accounts = lock_or_recover(&state.accounts);
        accounts
            .iter()
            .find(|a| a.id == draft.account_id)
            .cloned()
            .ok_or_else(|| format!("Account not found: {}", draft.account_id))?
    };

    let password = fetch_password(account.id.clone()).await?;

    // Build a minimal RFC 822 message for saving as a draft.
    // If HTML body is provided, include it as the main content.
    let body_content = draft
        .body_html
        .as_ref()
        .filter(|h| !h.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| draft.body.clone());

    let content_type = if draft
        .body_html
        .as_ref()
        .is_some_and(|h| !h.trim().is_empty())
    {
        "text/html; charset=utf-8"
    } else {
        "text/plain; charset=utf-8"
    };

    let rfc822 = format!(
        "From: {}\r\nTo: {}\r\nSubject: {}\r\nDate: {}\r\nContent-Type: {}\r\n\r\n{}",
        account.email,
        draft.to,
        draft.subject,
        chrono::Utc::now().to_rfc2822(),
        content_type,
        body_content,
    );

    let mut session = crate::mail::connection::connect_with_retry(&account, &password, 3)
        .await
        .map_err(|e| format!("Failed to connect to IMAP: {}", e))?;

    // Find the Drafts folder
    let drafts_folder = account
        .folders
        .iter()
        .find(|f| f.folder_type == crate::app_state::FolderType::Drafts)
        .map(|f| f.name.clone())
        .unwrap_or_else(|| "Drafts".to_string());

    crate::mail::imap::append_message(&mut session, &drafts_folder, rfc822.as_bytes())
        .await
        .map_err(|e| format!("Failed to save draft: {}", e))?;

    let _ = session.logout().await;

    tracing::info!("save_draft: saved to {}", drafts_folder);
    Ok(())
}

/// Save a draft version locally (for version history).
#[tauri::command]
pub async fn save_draft_version(
    state: State<'_, AppState>,
    draft_id: String,
    subject: String,
    body: String,
    body_html: Option<String>,
) -> Result<(), String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard.as_ref().ok_or("Database not initialized")?;
    db.save_draft_version(&draft_id, &subject, &body, body_html.as_deref())
        .map_err(|e| format!("Failed to save draft version: {}", e))
}

/// Load draft version history.
#[tauri::command]
pub async fn load_draft_versions(
    state: State<'_, AppState>,
    draft_id: String,
) -> Result<Vec<crate::storage::db::DraftVersion>, String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard.as_ref().ok_or("Database not initialized")?;
    db.load_draft_versions(&draft_id)
        .map_err(|e| format!("Failed to load draft versions: {}", e))
}

/// Delete all versions of a draft.
#[tauri::command]
pub async fn delete_draft_versions(
    state: State<'_, AppState>,
    draft_id: String,
) -> Result<(), String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard.as_ref().ok_or("Database not initialized")?;
    db.delete_draft_versions(&draft_id)
        .map_err(|e| format!("Failed to delete draft versions: {}", e))
}
