//! Mail search commands.

use tauri::State;

use crate::app_state::{lock_or_recover, MailEntry};
use crate::AppState;

/// Search messages in the local SQLite cache for a given account and folder.
/// Supports optional structured filters: filter_from, filter_subject, filter_to, filter_has_attachment.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn search_local(
    state: State<'_, AppState>,
    query: String,
    account_id: String,
    folder: String,
    filter_from: Option<String>,
    filter_subject: Option<String>,
    filter_to: Option<String>,
    filter_has_attachment: Option<bool>,
) -> Result<Vec<MailEntry>, String> {
    tracing::debug!(
        "search_local: query={}, account_id={}, folder={}, from={:?}, subject={:?}, to={:?}, has_attachment={:?}",
        query, account_id, folder, filter_from, filter_subject, filter_to, filter_has_attachment
    );
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard.as_ref().ok_or("Database not available")?;

    // If no structured filters, use the standard search
    let has_filters = filter_from.is_some()
        || filter_subject.is_some()
        || filter_to.is_some()
        || filter_has_attachment.unwrap_or(false);

    if !has_filters {
        return db
            .search_messages_in_folder(&query, &account_id, &folder)
            .map_err(|e| e.to_string());
    }

    // Build dynamic SQL WHERE clause for structured search
    let mut conditions = vec![
        "account_id = ?1".to_string(),
        "folder = ?2".to_string(),
    ];
    let mut param_values: Vec<String> = vec![account_id.clone(), folder.clone()];
    let mut param_idx = 3u32;

    if !query.is_empty() {
        let pattern = format!("%{}%", query);
        conditions.push(format!(
            "(subject LIKE ?{idx} OR from_addr LIKE ?{idx} OR preview LIKE ?{idx})",
            idx = param_idx
        ));
        param_values.push(pattern);
        param_idx += 1;
    }

    if let Some(ref from_filter) = filter_from {
        let pattern = format!("%{}%", from_filter);
        conditions.push(format!("from_addr LIKE ?{}", param_idx));
        param_values.push(pattern);
        param_idx += 1;
    }

    if let Some(ref subject_filter) = filter_subject {
        let pattern = format!("%{}%", subject_filter);
        conditions.push(format!("subject LIKE ?{}", param_idx));
        param_values.push(pattern);
        param_idx += 1;
    }

    if let Some(ref to_filter) = filter_to {
        let pattern = format!("%{}%", to_filter);
        conditions.push(format!("to_addrs LIKE ?{}", param_idx));
        param_values.push(pattern);
        param_idx += 1;
    }

    if filter_has_attachment.unwrap_or(false) {
        conditions.push("has_attachments = 1".to_string());
    }

    let _ = param_idx; // suppress unused warning

    let where_clause = conditions.join(" AND ");
    let sql = format!(
        "SELECT id, account_id, folder, from_addr, to_addrs, subject, date, preview, body_text, body_html, is_read, is_starred, has_attachments, uid, message_id, in_reply_to, refs, thread_id, spam_score, categories, is_pinned, snoozed_until \
         FROM messages WHERE {} ORDER BY date DESC LIMIT 100",
        where_clause
    );

    db.search_messages_dynamic(&sql, &param_values)
        .map_err(|e| e.to_string())
}
