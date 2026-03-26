//! Tauri commands for pin, snooze, and scheduled send features.

use tauri::State;

use std::collections::HashMap;

use crate::app_state::{fetch_password, lock_or_recover, ComposeDraft};
use crate::AppState;

// ── Pin ──────────────────────────────────────────────────────────────

/// Pin a mail so it stays at the top of the list.
#[tauri::command]
pub async fn pin_mail(state: State<'_, AppState>, mail_id: String) -> Result<(), String> {
    tracing::debug!("pin_mail: {}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.update_pinned_status(&mail_id, true)
        .map_err(|e| format!("Failed to pin mail: {}", e))
}

/// Unpin a mail.
#[tauri::command]
pub async fn unpin_mail(state: State<'_, AppState>, mail_id: String) -> Result<(), String> {
    tracing::debug!("unpin_mail: {}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.update_pinned_status(&mail_id, false)
        .map_err(|e| format!("Failed to unpin mail: {}", e))
}

// ── Snooze ───────────────────────────────────────────────────────────

/// Snooze a mail until a given ISO 8601 datetime.
#[tauri::command]
pub async fn snooze_mail(
    state: State<'_, AppState>,
    mail_id: String,
    until: String,
) -> Result<(), String> {
    tracing::info!("snooze_mail: {} until {}", mail_id, until);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.update_snoozed_until(&mail_id, Some(&until))
        .map_err(|e| format!("Failed to snooze mail: {}", e))
}

/// Unsnooze a mail (remove snoozed_until).
#[tauri::command]
pub async fn unsnooze_mail(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<(), String> {
    tracing::debug!("unsnooze_mail: {}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.update_snoozed_until(&mail_id, None)
        .map_err(|e| format!("Failed to unsnooze mail: {}", e))
}

/// Get all snoozed mails that are due (snoozed_until <= now).
#[tauri::command]
pub async fn get_due_snoozed(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.get_due_snoozed_ids()
        .map_err(|e| format!("Failed to get snoozed mails: {}", e))
}

// ── Flag follow-up ──────────────────────────────────────────────────

/// Flag a mail with an optional due date for follow-up.
#[tauri::command]
pub async fn flag_mail(
    state: State<'_, AppState>,
    mail_id: String,
    due_date: Option<String>,
) -> Result<(), String> {
    tracing::debug!("flag_mail: {} due={:?}", mail_id, due_date);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.update_flag_due_date(&mail_id, due_date.as_deref())
        .map_err(|e| format!("Failed to flag mail: {}", e))
}

/// Unflag a mail (remove flag due date).
#[tauri::command]
pub async fn unflag_mail(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<(), String> {
    tracing::debug!("unflag_mail: {}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.update_flag_due_date(&mail_id, None)
        .map_err(|e| format!("Failed to unflag mail: {}", e))
}

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

// ── Follow-up Tracker ────────────────────────────────────────────────

/// Add a follow-up: track a mail waiting for a reply from expected_from.
#[tauri::command]
pub async fn add_followup(
    state: State<'_, AppState>,
    mail_id: String,
    expected_from: String,
    due_date: String,
) -> Result<(), String> {
    tracing::info!("add_followup: {} from={}", mail_id, expected_from);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.add_followup(&mail_id, &expected_from, &due_date)
        .map_err(|e| format!("Failed to add followup: {}", e))
}

/// Get all follow-ups.
#[tauri::command]
pub async fn get_followups(
    state: State<'_, AppState>,
) -> Result<Vec<crate::storage::db::Followup>, String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.get_followups()
        .map_err(|e| format!("Failed to get followups: {}", e))
}

/// Resolve a follow-up (mark as replied).
#[tauri::command]
pub async fn resolve_followup(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<(), String> {
    tracing::debug!("resolve_followup: {}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.resolve_followup(&mail_id)
        .map_err(|e| format!("Failed to resolve followup: {}", e))
}

/// Delete a follow-up.
#[tauri::command]
pub async fn delete_followup(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<(), String> {
    tracing::debug!("delete_followup: {}", mail_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.delete_followup(&mail_id)
        .map_err(|e| format!("Failed to delete followup: {}", e))
}

/// Check if any unresolved followups have been replied to. Returns resolved mail IDs.
#[tauri::command]
pub async fn check_followups(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Vec<String>, String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.check_resolved_followups(&account_id)
        .map_err(|e| format!("Failed to check followups: {}", e))
}

// ── Tasks ────────────────────────────────────────────────────────────

/// Create a task (optionally linked to a mail).
#[tauri::command]
pub async fn create_task(
    state: State<'_, AppState>,
    title: String,
    description: String,
    mail_id: String,
    due_date: String,
) -> Result<String, String> {
    tracing::info!("create_task: {}", title);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.create_task(&title, &description, &mail_id, &due_date)
        .map_err(|e| format!("Failed to create task: {}", e))
}

/// Get all tasks.
#[tauri::command]
pub async fn get_tasks(
    state: State<'_, AppState>,
) -> Result<Vec<crate::storage::db::Task>, String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.get_tasks()
        .map_err(|e| format!("Failed to get tasks: {}", e))
}

/// Complete a task.
#[tauri::command]
pub async fn complete_task(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    tracing::debug!("complete_task: {}", id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.complete_task(&id)
        .map_err(|e| format!("Failed to complete task: {}", e))
}

/// Delete a task.
#[tauri::command]
pub async fn delete_task(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    tracing::debug!("delete_task: {}", id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.delete_task(&id)
        .map_err(|e| format!("Failed to delete task: {}", e))
}

// ── Scheduled Send ───────────────────────────────────────────────────

/// Schedule an email to be sent at a future time.
#[tauri::command]
pub async fn schedule_send(
    state: State<'_, AppState>,
    draft: ComposeDraft,
    scheduled_time: String,
) -> Result<(), String> {
    tracing::info!("schedule_send: to={}, at={}", draft.to, scheduled_time);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.save_scheduled_email(&draft, &scheduled_time)
        .map_err(|e| format!("Failed to schedule email: {}", e))
}

/// Get all scheduled (not yet sent) emails.
#[tauri::command]
pub async fn get_scheduled_emails(
    state: State<'_, AppState>,
) -> Result<Vec<crate::storage::db::ScheduledEmail>, String> {
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.get_scheduled_emails()
        .map_err(|e| format!("Failed to get scheduled emails: {}", e))
}

/// Cancel a scheduled email.
#[tauri::command]
pub async fn cancel_scheduled(
    state: State<'_, AppState>,
    schedule_id: String,
) -> Result<(), String> {
    tracing::info!("cancel_scheduled: {}", schedule_id);
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;
    db.delete_scheduled_email(&schedule_id)
        .map_err(|e| format!("Failed to cancel scheduled email: {}", e))
}

/// Send all due scheduled emails. Called by the background task.
#[tauri::command]
pub async fn send_due_scheduled(state: State<'_, AppState>) -> Result<u32, String> {
    let due = {
        let db_guard = lock_or_recover(&state.db);
        let db = db_guard
            .as_ref()
            .ok_or_else(|| "Database not available".to_string())?;
        db.get_due_scheduled_emails()
            .map_err(|e| format!("Failed to get due scheduled: {}", e))?
    };

    // Lock accounts ONCE and build a HashMap for O(1) lookups
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
