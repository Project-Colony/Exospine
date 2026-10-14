//! Scheduled send commands.

use std::collections::HashMap;

use tauri::State;

use crate::app_state::{fetch_password, lock_or_recover, with_db, ComposeDraft};
use crate::AppState;

/// Schedule an email to be sent at a future time.
#[tauri::command]
pub async fn schedule_send(
    state: State<'_, AppState>,
    draft: ComposeDraft,
    scheduled_time: String,
) -> Result<(), String> {
    tracing::info!("schedule_send: to={}, at={}", draft.to, scheduled_time);
    with_db(&state, |db| {
        db.save_scheduled_email(&draft, &scheduled_time)
            .map_err(|e| format!("Failed to schedule email: {}", e))
    })
}

/// Get all scheduled (not yet sent) emails.
#[tauri::command]
pub async fn get_scheduled_emails(
    state: State<'_, AppState>,
) -> Result<Vec<crate::storage::db::ScheduledEmail>, String> {
    with_db(&state, |db| {
        db.get_scheduled_emails()
            .map_err(|e| format!("Failed to get scheduled emails: {}", e))
    })
}

/// Cancel a scheduled email.
#[tauri::command]
pub async fn cancel_scheduled(
    state: State<'_, AppState>,
    schedule_id: String,
) -> Result<(), String> {
    tracing::info!("cancel_scheduled: {}", schedule_id);
    with_db(&state, |db| {
        db.delete_scheduled_email(&schedule_id)
            .map_err(|e| format!("Failed to cancel scheduled email: {}", e))
    })
}

/// Send all due scheduled emails. Called by the background task.
#[tauri::command]
pub async fn send_due_scheduled(state: State<'_, AppState>) -> Result<u32, String> {
    let due = with_db(&state, |db| {
        db.get_due_scheduled_emails()
            .map_err(|e| format!("Failed to get due scheduled: {}", e))
    })?;

    // Lock accounts ONCE, clone into a lookup map, then drop the guard before any .await
    let account_map: HashMap<String, crate::app_state::Account> = {
        let accounts = lock_or_recover(&state.accounts);
        accounts
            .iter()
            .map(|a| (a.id.clone(), a.clone()))
            .collect()
    };

    let mut sent_count = 0u32;
    for sched in &due {
        let draft = ComposeDraft {
            to: sched.to_addr.clone(),
            cc: sched.cc.clone(),
            bcc: sched.bcc.clone(),
            subject: sched.subject.clone(),
            body: sched.body.clone(),
            body_html: None,
            reply_to: None,
            account_id: sched.account_id.clone(),
            attachments: Vec::new(),
            importance: "normal".to_string(),
        };

        let Some(account) = account_map.get(&draft.account_id) else {
            tracing::error!(
                "send_due_scheduled: account not found for {}",
                draft.account_id
            );
            continue;
        };

        let password = match fetch_password(&account.id).await {
            Ok(pw) => pw,
            Err(e) => {
                tracing::error!("send_due_scheduled: password error: {}", e);
                continue;
            }
        };

        match crate::mail::smtp::send_mail(account, &password, &draft, None).await {
            Ok(_) => {
                tracing::info!("send_due_scheduled: sent to {}", draft.to);
                let db_guard = lock_or_recover(&state.db);
                if let Some(ref db) = *db_guard {
                    let _ = db.delete_scheduled_email(&sched.id);
                }
                sent_count += 1;
            }
            Err(e) => {
                tracing::error!("send_due_scheduled: smtp error: {}", e);
            }
        }
    }

    Ok(sent_count)
}
