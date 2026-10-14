//! Mail utility commands: attachments, export, duplicates, notes, cleanup,
//! security log, categories, spam reporting, analytics.

use serde::Serialize;
use tauri::State;

use crate::app_state::{fetch_password, get_imap_session, lock_or_recover, map_err_str, with_db};
use crate::mail::attachments;
use crate::AppState;

use super::mail::get_account;

/// Download a specific attachment from an email.
/// Fetches the raw message from IMAP, extracts the attachment by part_index,
/// saves it to the Downloads/Exospine folder, and returns the file path.
#[tauri::command]
pub async fn download_attachment(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    mail_uid: u32,
    part_index: usize,
) -> Result<String, String> {
    tracing::info!(
        "download_attachment: uid={} part={} in {}/{}",
        mail_uid, part_index, account_id, folder
    );

    let (mut session, _account) = get_imap_session(&state, &account_id, &folder).await?;

    // Fetch the raw message by UID
    let raw = fetch_raw_message(&mut session, mail_uid).await?;

    crate::mail::connection::return_session_with_folder(&account_id, session, folder);

    // Save the attachment to disk
    let dest = attachments::download_dir();
    let file_path = attachments::save_attachment(&raw, part_index, &dest)
        .await
        .map_err(map_err_str("Failed to save attachment"))?;

    Ok(file_path.to_string_lossy().to_string())
}

/// Export (download) the raw RFC 822 email as a .eml file.
#[tauri::command]
pub async fn export_mail(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    mail_uid: u32,
) -> Result<String, String> {
    tracing::info!(
        "export_mail: uid={} in {}/{}",
        mail_uid, account_id, folder
    );

    let (mut session, _account) = get_imap_session(&state, &account_id, &folder).await?;

    // Fetch the raw message
    let raw = fetch_raw_message(&mut session, mail_uid).await?;

    crate::mail::connection::return_session_with_folder(&account_id, session, folder);

    // Save as .eml file in Downloads/Exospine
    let dest = attachments::download_dir();
    tokio::fs::create_dir_all(&dest)
        .await
        .map_err(map_err_str("Failed to create directory"))?;

    let filename = format!("email_{}.eml", mail_uid);
    let file_path = dest.join(&filename);

    tokio::fs::write(&file_path, &raw)
        .await
        .map_err(map_err_str("Failed to write .eml file"))?;

    Ok(file_path.to_string_lossy().to_string())
}

/// Read the last N entries from the security audit log.
#[tauri::command]
pub async fn get_security_log(count: Option<u32>) -> Result<Vec<String>, String> {
    let n = count.unwrap_or(20) as usize;
    Ok(crate::storage::audit_log::read_last_entries(n))
}

/// Add a category/label to a mail message.
/// Also attempts to sync the keyword to IMAP via STORE +FLAGS.
#[tauri::command]
pub async fn add_category(
    state: State<'_, AppState>,
    mail_id: String,
    category: String,
) -> Result<(), String> {
    tracing::debug!("add_category: mail_id={}, category={}", mail_id, category);
    with_db(&state, |db| {
        db.add_category(&mail_id, &category)
            .map_err(map_err_str("Failed to add category"))
    })?;

    // Best-effort sync to IMAP keywords
    sync_imap_keyword(&state, &mail_id, &category, true).await;

    Ok(())
}

/// Remove a category/label from a mail message.
/// Also attempts to remove the IMAP keyword via STORE -FLAGS.
#[tauri::command]
pub async fn remove_category(
    state: State<'_, AppState>,
    mail_id: String,
    category: String,
) -> Result<(), String> {
    tracing::debug!("remove_category: mail_id={}, category={}", mail_id, category);
    with_db(&state, |db| {
        db.remove_category(&mail_id, &category)
            .map_err(map_err_str("Failed to remove category"))
    })?;

    // Best-effort sync to IMAP keywords
    sync_imap_keyword(&state, &mail_id, &category, false).await;

    Ok(())
}

/// Helper: sync a category as an IMAP keyword flag (+FLAGS or -FLAGS).
/// Best-effort: failures are logged but do not cause the command to fail.
async fn sync_imap_keyword(
    state: &State<'_, AppState>,
    mail_id: &str,
    category: &str,
    add: bool,
) {
    // Look up account_id, folder, uid for this mail
    let meta = {
        let db_guard = lock_or_recover(&state.db);
        match db_guard.as_ref() {
            Some(db) => db.load_message_meta(mail_id).ok(),
            None => None,
        }
    };

    let Some((account_id, folder, uid)) = meta else {
        tracing::debug!("sync_imap_keyword: no meta for mail_id={}, skipping IMAP sync", mail_id);
        return;
    };

    let account = match get_account(state, &account_id) {
        Ok(a) => a,
        Err(_) => return,
    };
    let password = match fetch_password(&account_id).await {
        Ok(p) => p,
        Err(_) => return,
    };

    let session_result = crate::mail::connection::get_session_for_folder(
        &account_id, &account, &password, &folder,
    )
    .await;

    let mut session = match session_result {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("sync_imap_keyword: failed to connect for {}: {}", mail_id, e);
            return;
        }
    };

    let flag_op = if add { "+FLAGS" } else { "-FLAGS" };
    let uid_str = uid.to_string();
    let keyword = category.replace(' ', "_");

    match session
        .uid_store(&uid_str, &format!("{} ({})", flag_op, keyword))
        .await
    {
        Ok(stream) => {
            // Consume the stream to complete the operation
            use futures::TryStreamExt;
            let _ = stream.try_collect::<Vec<_>>().await;
            tracing::debug!(
                "sync_imap_keyword: {} keyword '{}' on uid={} in {}/{}",
                if add { "added" } else { "removed" },
                keyword,
                uid,
                account_id,
                folder
            );
        }
        Err(e) => {
            tracing::warn!(
                "sync_imap_keyword: STORE {} failed for uid={}: {}",
                flag_op,
                uid,
                e
            );
        }
    }

    crate::mail::connection::return_session_with_folder(&account_id, session, folder);
}

/// Report a mail as spam (stores user decision for future filtering).
#[tauri::command]
pub async fn report_spam(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<(), String> {
    tracing::debug!("report_spam: mail_id={}", mail_id);
    with_db(&state, |db| {
        db.add_category(&mail_id, "Spam")
            .map_err(map_err_str("Failed to report spam"))
    })
}

/// Report a mail as not spam (removes Spam category).
#[tauri::command]
pub async fn report_not_spam(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<(), String> {
    tracing::debug!("report_not_spam: mail_id={}", mail_id);
    with_db(&state, |db| {
        db.remove_category(&mail_id, "Spam")
            .map_err(map_err_str("Failed to unmark spam"))
    })
}

/// Helper: fetch the raw RFC 822 message bytes for a given UID.
pub(crate) async fn fetch_raw_message(
    session: &mut crate::mail::imap::ImapSession,
    uid: u32,
) -> Result<Vec<u8>, String> {
    use futures::TryStreamExt;

    let uid_str = uid.to_string();
    let messages: Vec<_> = session
        .uid_fetch(&uid_str, "BODY.PEEK[]")
        .await
        .map_err(map_err_str("Failed to fetch raw message"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(map_err_str("Failed to collect raw message"))?;

    let msg = messages
        .first()
        .ok_or_else(|| format!("No message found for UID {}", uid))?;

    let body = msg
        .body()
        .ok_or_else(|| format!("Message UID {} has no body", uid))?;

    Ok(body.to_vec())
}

/// Get email analytics for an account.
#[tauri::command]
pub async fn get_analytics(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<crate::storage::db::EmailAnalytics, String> {
    with_db(&state, |db| {
        db.get_analytics(&account_id)
            .map_err(map_err_str("Failed to get analytics"))
    })
}

/// Delete messages older than the specified number of days. 0 = disabled (no-op).
#[tauri::command]
pub async fn cleanup_old_messages(
    state: State<'_, AppState>,
    days: u32,
) -> Result<usize, String> {
    if days == 0 {
        return Ok(0);
    }
    tracing::info!("cleanup_old_messages: deleting messages older than {} days", days);
    let count = with_db(&state, |db| {
        db.cleanup_old_messages(days)
            .map_err(map_err_str("Failed to cleanup old messages"))
    })?;
    if count > 0 {
        tracing::info!("cleanup_old_messages: deleted {} messages", count);
    }
    Ok(count)
}

// ── Notes commands ──────────────────────────────────────────────────

/// Save a note for a mail message.
#[tauri::command]
pub async fn save_note(
    state: State<'_, AppState>,
    mail_id: String,
    note: String,
) -> Result<(), String> {
    tracing::debug!("save_note: mail_id={}", mail_id);
    with_db(&state, |db| {
        if note.trim().is_empty() {
            db.delete_note(&mail_id)
                .map_err(map_err_str("Failed to delete note"))
        } else {
            db.save_note(&mail_id, &note)
                .map_err(map_err_str("Failed to save note"))
        }
    })
}

/// Get a note for a mail message.
#[tauri::command]
pub async fn get_note(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<Option<String>, String> {
    with_db(&state, |db| {
        db.get_note(&mail_id)
            .map_err(map_err_str("Failed to get note"))
    })
}

// ── Duplicate detection commands ────────────────────────────────────

/// A pair of duplicate message IDs.
#[derive(Debug, Clone, Serialize)]
pub struct DuplicatePair {
    pub original_id: String,
    pub duplicate_id: String,
}

/// Find duplicate messages in a folder.
#[tauri::command]
pub async fn find_duplicates(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
) -> Result<Vec<DuplicatePair>, String> {
    tracing::debug!("find_duplicates: {}/{}", account_id, folder);
    with_db(&state, |db| {
        let pairs = db
            .find_duplicates(&account_id, &folder)
            .map_err(map_err_str("Failed to find duplicates"))?;
        Ok(pairs
            .into_iter()
            .map(|(orig, dup)| DuplicatePair {
                original_id: orig,
                duplicate_id: dup,
            })
            .collect())
    })
}
