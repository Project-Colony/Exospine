//! Application state types for the Tauri backend.
//!
//! All iced-specific types have been removed. Structs that need to be sent
//! to the frontend derive `Serialize` (and `Deserialize` where needed).

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex as StdMutex, MutexGuard};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::mail::attachments::AttachmentMeta;
use crate::mail::security::{AuthStatus, PhishingWarning};
use crate::storage::db::Database;

// ── Lock helpers ────────────────────────────────────────────────────────

/// Acquire a mutex lock, recovering from a poisoned state.
pub fn lock_or_recover<T>(mutex: &std::sync::Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Convenience: lock the DB mutex and return a guard, or an error string.
#[allow(dead_code)]
pub fn get_db(state: &crate::AppState) -> Result<MutexGuard<'_, Option<Database>>, String> {
    Ok(lock_or_recover(&state.db))
}

// ── Global in-memory credential store ──────────────────────────────────

/// Global credential store shared between async tasks.
/// Maps account_id -> password/access_token.
pub static CREDENTIALS: LazyLock<StdMutex<HashMap<String, String>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

/// Store a credential in the global in-memory store.
pub fn store_credential(account_id: &str, password: &str) {
    let mut store = lock_or_recover(&CREDENTIALS);
    // Zeroize the old value if it exists before replacing.
    if let Some(old) = store.get_mut(account_id) {
        old.zeroize();
    }
    store.insert(account_id.to_owned(), password.to_owned());
}

/// Remove a credential from the in-memory store, zeroizing the secret before drop.
#[allow(dead_code)]
pub fn remove_credential(account_id: &str) {
    let mut store = lock_or_recover(&CREDENTIALS);
    if let Some(mut old) = store.remove(account_id) {
        old.zeroize();
    }
}

/// Retrieve the password for an account: checks in-memory store first, then keyring.
pub async fn fetch_password(account_id: String) -> Result<String, String> {
    // Check in-memory store first (reliable on all platforms).
    if let Some(pw) = lock_or_recover(&CREDENTIALS).get(&account_id) {
        return Ok(pw.clone());
    }
    // Fall back to OS keyring.
    tokio::task::spawn_blocking(move || {
        crate::accounts::keyring_store::get_password(&account_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

// ── Authentication method ──────────────────────────────────────────────

/// Authentication method for an account.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthMethod {
    /// Standard username + password (or app-specific password).
    #[default]
    Basic,
    /// OAuth2 with a stored refresh token.
    OAuth2 {
        /// The OAuth2 refresh token used to obtain new access tokens.
        refresh_token: String,
    },
}


// ── Account ────────────────────────────────────────────────────────────

/// An email account with IMAP/SMTP connection details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Email address (e.g. "user@example.com").
    pub email: String,
    /// IMAP server hostname.
    pub imap_host: String,
    /// IMAP server port (typically 993 for TLS).
    pub imap_port: u16,
    /// SMTP server hostname.
    pub smtp_host: String,
    /// SMTP server port (typically 465 or 587).
    pub smtp_port: u16,
    /// Login username (often the same as the email address).
    pub username: String,
    /// Whether to use TLS for both IMAP and SMTP connections.
    pub use_tls: bool,
    /// Mailbox folders discovered on the server.
    pub folders: Vec<Folder>,
    /// Authentication method (Basic password or OAuth2).
    #[serde(default)]
    pub auth_method: AuthMethod,
    /// Plain-text email signature appended to outgoing mail.
    #[serde(default)]
    pub signature: String,
    /// Optional HTML version of the email signature.
    #[serde(default)]
    pub signature_html: Option<String>,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            email: String::new(),
            imap_host: String::new(),
            imap_port: 993,
            smtp_host: String::new(),
            smtp_port: 465,
            username: String::new(),
            use_tls: true,
            folders: Vec::new(),
            auth_method: AuthMethod::Basic,
            signature: String::new(),
            signature_html: None,
        }
    }
}

// ── Folder ─────────────────────────────────────────────────────────────

/// A mailbox folder within an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    /// Folder name as reported by the server (e.g. "INBOX", "Sent").
    pub name: String,
    /// Number of unread messages.
    pub unread_count: u32,
    /// Total number of messages.
    pub total_count: u32,
    /// Semantic folder type used for icon/sort decisions.
    pub folder_type: FolderType,
}

impl Default for Folder {
    fn default() -> Self {
        Self {
            name: String::new(),
            unread_count: 0,
            total_count: 0,
            folder_type: FolderType::Custom,
        }
    }
}

/// Semantic type of a mail folder, used for special handling and icons.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FolderType {
    Inbox,
    Sent,
    Drafts,
    Trash,
    Starred,
    #[default]
    Custom,
}

// ── Mail entry ─────────────────────────────────────────────────────────

/// A single mail message with headers, body, and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailEntry {
    /// Unique message identifier (server UID or generated).
    pub id: String,
    /// IMAP UID for delta sync.
    pub uid: u32,
    /// Sender address.
    pub from: String,
    /// Recipient addresses.
    pub to: Vec<String>,
    /// Subject line.
    pub subject: String,
    /// Date the message was sent or received.
    pub date: DateTime<Utc>,
    /// Plain-text preview (first ~100 characters of the body).
    pub preview: String,
    /// Full plain-text body (empty when loaded as header-only; loaded on demand from DB).
    pub body_text: String,
    /// Full HTML body, if available (None when loaded as header-only).
    pub body_html: Option<String>,
    /// Whether this message has been read.
    pub is_read: bool,
    /// Whether this message is starred/flagged.
    pub is_starred: bool,
    /// Whether this message has file attachments.
    pub has_attachments: bool,
    /// Name of the folder this message belongs to.
    pub folder: String,
    /// ID of the account this message belongs to.
    pub account_id: String,
    /// Categories/labels applied to this message.
    pub categories: Vec<String>,
    /// Attachment metadata extracted during parsing.
    pub attachment_meta: Vec<AttachmentMeta>,
    /// Phishing warnings detected in email body links.
    #[serde(default)]
    pub phishing_warnings: Vec<PhishingWarning>,
    /// SPF/DKIM/DMARC authentication status from headers.
    #[serde(default)]
    pub auth_status: Option<AuthStatus>,
    /// Sender-level warnings (homograph, reply-to mismatch, etc.).
    #[serde(default)]
    pub sender_warnings: Vec<String>,
    /// RFC 2822 Message-ID header.
    #[serde(default)]
    pub message_id: String,
    /// In-Reply-To header (parent message ID).
    #[serde(default)]
    pub in_reply_to: Option<String>,
    /// References header (chain of ancestor message IDs).
    #[serde(default)]
    pub references: Vec<String>,
    /// Thread identifier: first Message-ID in the References chain, or normalized subject.
    #[serde(default)]
    pub thread_id: String,
    /// Spam score computed by the rule-based filter (0.0 = clean, >3.0 = likely spam).
    #[serde(default)]
    pub spam_score: f32,
    /// Whether this message is pinned (stays at top of list).
    #[serde(default)]
    pub is_pinned: bool,
    /// If snoozed, the ISO 8601 datetime when it should reappear.
    #[serde(default)]
    pub snoozed_until: Option<String>,
    /// Whether the sender requested a read receipt (Disposition-Notification-To).
    #[serde(default)]
    pub read_receipt_requested: bool,
    /// Address to send the read receipt to.
    #[serde(default)]
    pub read_receipt_to: Option<String>,
    /// Message importance/priority: "high", "normal", or "low".
    #[serde(default = "default_importance")]
    pub importance: String,
    /// Flag follow-up due date (ISO 8601), if flagged.
    #[serde(default)]
    pub flag_due_date: Option<String>,
}

impl Default for MailEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            uid: 0,
            from: String::new(),
            to: Vec::new(),
            subject: String::new(),
            date: Utc::now(),
            preview: String::new(),
            body_text: String::new(),
            body_html: None,
            is_read: false,
            is_starred: false,
            has_attachments: false,
            folder: String::new(),
            account_id: String::new(),
            categories: Vec::new(),
            attachment_meta: Vec::new(),
            phishing_warnings: Vec::new(),
            auth_status: None,
            sender_warnings: Vec::new(),
            message_id: String::new(),
            in_reply_to: None,
            references: Vec::new(),
            thread_id: String::new(),
            spam_score: 0.0,
            is_pinned: false,
            snoozed_until: None,
            read_receipt_requested: false,
            read_receipt_to: None,
            importance: "normal".to_string(),
            flag_due_date: None,
        }
    }
}

// ── Compose draft ──────────────────────────────────────────────────────

/// A file attached to a compose draft.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeAttachment {
    /// Full path to the file on disk.
    pub path: String,
    /// Display filename (basename).
    pub filename: String,
    /// MIME content type (e.g. "application/pdf").
    pub content_type: String,
    /// File size in bytes.
    pub size: usize,
}

/// A message being composed (new, reply, or forward).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeDraft {
    /// Recipient address(es), comma-separated.
    pub to: String,
    /// CC address(es), comma-separated.
    pub cc: String,
    /// BCC address(es), comma-separated.
    pub bcc: String,
    /// Subject line.
    pub subject: String,
    /// Message body text (plain text fallback).
    pub body: String,
    /// Optional HTML body for rich text emails.
    #[serde(default)]
    pub body_html: Option<String>,
    /// If this is a reply, the original message ID.
    pub reply_to: Option<String>,
    /// Account to send from.
    pub account_id: String,
    /// File attachments to include with the message.
    pub attachments: Vec<ComposeAttachment>,
    /// Message importance/priority for outgoing mail: "high", "normal", or "low".
    #[serde(default = "default_importance")]
    pub importance: String,
}

impl Default for ComposeDraft {
    fn default() -> Self {
        Self {
            to: String::new(),
            cc: String::new(),
            bcc: String::new(),
            subject: String::new(),
            body: String::new(),
            body_html: None,
            reply_to: None,
            account_id: String::new(),
            attachments: Vec::new(),
            importance: "normal".to_string(),
        }
    }
}

// ── Mail template ──────────────────────────────────────────────────────

/// A reusable mail template with pre-filled subject and body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailTemplate {
    /// Human-readable template name (e.g. "Meeting request").
    pub name: String,
    /// Pre-filled subject line.
    pub subject: String,
    /// Pre-filled body text.
    pub body: String,
}

// ── Application settings ───────────────────────────────────────────────

/// Persisted application settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Theme name (e.g. "dark", "light").
    pub theme: String,
    /// Base font size in pixels.
    pub font_size: u16,
    /// How often to check for new mail, in seconds.
    pub check_interval_secs: u64,
    /// Whether to show desktop notifications for new mail.
    pub show_notifications: bool,
    /// Reading pane position: "right", "bottom", or "off".
    #[serde(default = "default_reading_pane")]
    pub reading_pane: String,
    /// Display density: "compact" or "normal".
    #[serde(default = "default_density")]
    pub density: String,
    /// UI language: "en" or "fr".
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_importance() -> String {
    "normal".to_string()
}

fn default_reading_pane() -> String {
    "right".to_string()
}
fn default_density() -> String {
    "normal".to_string()
}
fn default_language() -> String {
    "en".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            font_size: 14,
            check_interval_secs: 300,
            show_notifications: true,
            reading_pane: "right".to_string(),
            density: "normal".to_string(),
            language: "en".to_string(),
        }
    }
}

// ── Quick filter ───────────────────────────────────────────────────────

/// Predefined quick-filter categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuickFilter {
    All,
    Unread,
    Starred,
    HasAttachments,
}
