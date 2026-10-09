//! Mail reading commands: fetching mail lists, bodies, headers, and EML files.

use std::time::Duration;

use serde::Serialize;
use tauri::State;
use tokio::time::timeout;

use crate::app_state::{get_imap_session, lock_or_recover, map_err_str, MailEntry};
use crate::mail::security::{AuthStatus, PhishingWarning};
use crate::AppState;

/// Response structure for get_mail_body, serialized as { text, html, security } for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct MailBodyResponse {
    pub text: String,
    #[serde(serialize_with = "crate::mail::html_render::serialize_sanitized")]
    pub html: Option<String>,
    pub phishing_warnings: Vec<PhishingWarning>,
    pub auth_status: Option<AuthStatus>,
    pub sender_warnings: Vec<String>,
    pub unsubscribe_url: Option<String>,
}

/// Get mail headers from SQLite cache (paginated).
/// On the first call (page 0 with no cached data), triggers an IMAP sync automatically.
#[tauri::command]
pub async fn get_mails(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    page: u32,
    per_page: u32,
) -> Result<Vec<MailEntry>, String> {
    tracing::debug!(
        "get_mails: account_id={}, folder={}, page={}, per_page={}",
        account_id, folder, page, per_page
    );
    let offset = page * per_page;

    // Check if we have cached data
    let cached_count = {
        let db_guard = lock_or_recover(&state.db);
        match db_guard.as_ref() {
            Some(db) => db.message_count(&account_id, &folder).unwrap_or(0),
            None => 0,
        }
    };

    // Pagination boundary check: if requested offset exceeds total count, return empty
    if cached_count > 0 && offset >= cached_count {
        tracing::debug!(
            "get_mails: page {} beyond total count {} — returning empty",
            page, cached_count
        );
        return Ok(Vec::new());
    }

    // If no cached data and first page, fetch the latest N mails from IMAP (fast)
    if cached_count == 0 && page == 0 {
        tracing::info!("get_mails: no cache, fetching from IMAP for {}/{}", account_id, folder);

        let (mut session, _account) = get_imap_session(&state, &account_id, &folder).await?;

        // Only fetch the latest 50 mails (not all 2000+)
        let mut mails =
            crate::mail::imap::fetch_messages(&mut session, &folder, per_page)
                .await
                .map_err(map_err_str("Failed to fetch messages"))?;

        crate::mail::connection::return_session_with_folder(&account_id, session, folder.clone());

        // Ensure account_id is set on all entries (fetch_messages passes "" to parser)
        for m in &mut mails {
            if m.account_id.is_empty() {
                m.account_id = account_id.clone();
            }
        }

        // Cache to DB
        if !mails.is_empty() {
            let db_guard = lock_or_recover(&state.db);
            if let Some(ref db) = *db_guard {
                if let Err(e) = db.save_messages(&mails) {
                    tracing::error!("get_mails: failed to cache messages to DB: {}", e);
                }
            }
        }

        tracing::info!("get_mails: returning {} mails from IMAP", mails.len());
        return Ok(mails);
    }

    // Read from SQLite cache
    let db_guard = lock_or_recover(&state.db);
    let db = db_guard
        .as_ref()
        .ok_or_else(|| "Database not available".to_string())?;

    let result = db
        .load_message_headers_page(&account_id, &folder, per_page, offset)
        .map_err(map_err_str("Failed to load messages from cache"))?;

    tracing::debug!("get_mails: returning {} cached mails", result.len());
    Ok(result)
}

/// Load body from SQLite for a specific message.
/// Falls back to fetching from IMAP if the cached body is empty.
#[tauri::command]
pub async fn get_mail_body(
    state: State<'_, AppState>,
    mail_id: String,
) -> Result<MailBodyResponse, String> {
    tracing::debug!("get_mail_body: mail_id={}", mail_id);

    // Try loading from DB first
    let db_result = {
        let db_guard = lock_or_recover(&state.db);
        match db_guard.as_ref() {
            Some(db) => db.load_message_body(&mail_id).ok(),
            None => None,
        }
    };

    // Fetch unsubscribe_url from DB (separate lightweight query)
    let unsub_url = {
        let db_guard = lock_or_recover(&state.db);
        match db_guard.as_ref() {
            Some(db) => db.get_unsubscribe_url(&mail_id).unwrap_or(None),
            None => None,
        }
    };

    if let Some((ref text, ref html)) = db_result {
        if !text.is_empty() || html.is_some() {
            tracing::debug!("get_mail_body: returning cached body");
            // Run security analysis on cached body
            let analysis = crate::mail::security::analyze_email(
                "", // from not available from body-only cache
                None,
                html.as_deref(),
                "",
            );
            return Ok(MailBodyResponse {
                text: text.clone(),
                html: html.clone(),
                phishing_warnings: analysis.phishing_warnings,
                auth_status: analysis.auth_status,
                sender_warnings: analysis.sender_warnings,
                unsubscribe_url: unsub_url,
            });
        }
    }

    tracing::debug!("get_mail_body: cache miss, fetching from IMAP");

    // Fallback: fetch from IMAP by UID
    let (account_id, folder, uid) = {
        let db_guard = lock_or_recover(&state.db);
        let db = db_guard
            .as_ref()
            .ok_or_else(|| "Database not available".to_string())?;
        db.load_message_meta(&mail_id)
            .map_err(map_err_str("Failed to load message metadata"))?
    };

    let (mut session, _account) = get_imap_session(&state, &account_id, &folder).await?;

    let mails = timeout(
        Duration::from_secs(15),
        crate::mail::imap::fetch_batch_by_uids(
            &mut session,
            &[uid],
            &account_id,
            &folder,
        ),
    )
    .await
    .map_err(|_| "Timeout: fetching message body took longer than 15 seconds".to_string())?
    .map_err(map_err_str("Failed to fetch message body"))?;

    crate::mail::connection::return_session_with_folder(&account_id, session, folder.clone());

    if let Some(mail) = mails.first() {
        // Cache the body to DB for future requests
        {
            let db_guard = lock_or_recover(&state.db);
            if let Some(ref db) = *db_guard {
                if let Err(e) = db.save_messages(std::slice::from_ref(mail)) {
                    tracing::error!("get_mail_body: failed to cache body to DB: {}", e);
                }
            }
        }

        Ok(MailBodyResponse {
            text: mail.body_text.clone(),
            html: mail.body_html.clone(),
            phishing_warnings: mail.phishing_warnings.clone(),
            auth_status: mail.auth_status.clone(),
            sender_warnings: mail.sender_warnings.clone(),
            unsubscribe_url: mail.unsubscribe_url.clone(),
        })
    } else {
        tracing::warn!("get_mail_body: no message found for uid={}", uid);
        Ok(MailBodyResponse {
            text: String::new(),
            html: None,
            phishing_warnings: Vec::new(),
            auth_status: None,
            sender_warnings: Vec::new(),
            unsubscribe_url: None,
        })
    }
}

/// Fetch raw email headers for a specific message by UID.
/// Returns the raw header string for display in the UI.
#[tauri::command]
pub async fn get_mail_headers(
    state: State<'_, AppState>,
    account_id: String,
    folder: String,
    mail_uid: u32,
) -> Result<String, String> {
    tracing::debug!(
        "get_mail_headers: uid={} in {}/{}",
        mail_uid, account_id, folder
    );

    let (mut session, _account) = get_imap_session(&state, &account_id, &folder).await?;

    // Fetch only the HEADER section for this UID
    let uid_str = mail_uid.to_string();
    let messages: Vec<_> = {
        use futures::TryStreamExt;
        session
            .uid_fetch(&uid_str, "BODY.PEEK[HEADER]")
            .await
            .map_err(map_err_str("Failed to fetch headers"))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(map_err_str("Failed to collect headers"))?
    };

    crate::mail::connection::return_session_with_folder(&account_id, session, folder);

    let msg = messages
        .first()
        .ok_or_else(|| format!("No message found for UID {}", mail_uid))?;

    // BODY[HEADER] is returned in the body() field
    let header_bytes = msg
        .body()
        .ok_or_else(|| format!("Message UID {} has no headers", mail_uid))?;

    let headers = String::from_utf8_lossy(header_bytes).to_string();
    Ok(headers)
}

/// Open and parse a `.eml` file from the file system.
/// Returns the parsed email as a `MailEntry` for display in the reading pane.
#[tauri::command]
pub async fn open_eml_file(path: String) -> Result<MailEntry, String> {
    tracing::info!("open_eml_file: path={}", path);

    let raw = tokio::fs::read(&path)
        .await
        .map_err(map_err_str("Failed to read .eml file"))?;

    let entry = crate::mail::parser::parse_email(&raw, "__local__", "__eml__")
        .map_err(map_err_str("Failed to parse .eml file"))?;

    Ok(entry)
}

/// Open one message in its own window. The page (`mail.html`) loads the body
/// through `get_mail_body`, so it shows the same sanitized HTML as the
/// reading pane. `query` is the page's URL query (message id and header
/// fields), built by the caller.
#[tauri::command]
pub async fn open_mail_window(
    app: tauri::AppHandle,
    title: String,
    query: String,
) -> Result<(), String> {
    let url = tauri::WebviewUrl::App(format!("mail.html?{query}").into());
    tauri::WebviewWindowBuilder::new(&app, format!("mail-{}", uuid::Uuid::new_v4()), url)
        .title(title)
        .inner_size(800.0, 600.0)
        .build()
        .map_err(map_err_str("Failed to open the message window"))?;
    Ok(())
}
