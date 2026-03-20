//! Application state types for Exospine.
//!
//! This module defines all the core data structures that make up
//! the application state, including accounts, mail entries, folders,
//! compose drafts, and settings.

use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::mail::attachments::AttachmentMeta;
use crate::search::QuickFilter;
use crate::ui::confirm_dialog::ConfirmDialog;
use crate::ui::toast::Toast;

// ── Context menu types ──────────────────────────────────────────────────

/// Target of a right-click context menu action.
#[derive(Debug, Clone)]
pub enum ContextTarget {
    /// A mail entry by its index in `mail_entries`.
    Mail(usize),
    /// A folder by its name.
    Folder(String),
}

/// State for a visible context menu overlay.
#[derive(Debug, Clone)]
pub struct ContextMenu {
    /// Screen position (x, y) where the menu should appear.
    pub position: (f32, f32),
    /// What was right-clicked.
    pub target: ContextTarget,
}

/// The main Iced application state.
#[derive(Debug, Clone)]
pub struct App {
    /// Configured email accounts.
    pub accounts: Vec<Account>,
    /// Index of the currently selected account in `accounts`.
    pub selected_account: Option<usize>,
    /// Name of the currently selected folder (e.g. "INBOX").
    pub selected_folder: Option<String>,
    /// Index of the currently selected mail in `mail_entries`.
    pub selected_mail: Option<usize>,
    /// Mail entries loaded for the active folder.
    pub mail_entries: Vec<MailEntry>,
    /// Current search/filter query.
    pub search_query: String,
    /// Active quick filter.
    pub quick_filter: QuickFilter,
    /// Filtered indices into mail_entries (updated on search/filter change).
    pub filtered_indices: Vec<usize>,
    /// Active compose draft, if the user is writing a message.
    pub composing: Option<ComposeDraft>,
    /// Which top-level view is active.
    pub view: View,
    /// Transient status bar message (e.g. "Mail sent", "Error: …").
    pub status_message: Option<String>,
    /// Whether a background operation is in progress.
    pub loading: bool,
    /// Persisted application settings.
    pub settings: AppSettings,
    /// Account setup dialog state (None = closed).
    pub account_dialog: Option<AccountDialogState>,
    /// Confirmation dialog (None = closed).
    pub confirm_dialog: Option<ConfirmDialog>,
    /// Toast notification stack.
    pub toasts: Vec<Toast>,
    /// Counter for unique toast IDs.
    pub next_toast_id: u64,
    /// Attachment metadata for the currently selected mail.
    pub current_attachments: Vec<AttachmentMeta>,
    /// Timestamp of the last successful mail sync (for periodic refresh).
    pub last_sync: Option<Instant>,
    /// Current pagination page (starts at 0).
    pub mail_page: u32,
    /// Number of messages to fetch per page.
    pub mails_per_page: u32,
    /// Whether more messages exist on the server beyond what has been loaded.
    pub has_more_mails: bool,
    /// Text input for composing attachment file path.
    pub compose_attachment_path: String,
    /// Whether this is the first launch (no accounts configured).
    pub first_launch: bool,
    /// Current onboarding step: 0 = welcome, 1 = add account, 2 = done.
    pub onboarding_step: u8,
    /// Active context menu overlay (None = hidden).
    pub context_menu: Option<ContextMenu>,
    /// Whether IMAP IDLE (push notifications) is enabled.
    pub idle_enabled: bool,
    /// Last known message count in the selected folder (for polling fallback).
    pub last_known_count: u32,
    /// Reusable mail templates (initialized with built-in defaults).
    pub templates: Vec<MailTemplate>,
    /// Whether to show the HTML version of the email body (true) or plain text (false).
    pub show_html: bool,
    /// Whether to allow loading external images in HTML emails.
    pub allow_external_images: bool,
    /// Calendar events (P4 scaffolding).
    pub calendar_events: Vec<crate::calendar::CalendarEvent>,
    /// Contacts extracted from mail or imported (P4 scaffolding).
    pub contacts: Vec<crate::contacts::Contact>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            accounts: Vec::new(),
            selected_account: None,
            selected_folder: None,
            selected_mail: None,
            mail_entries: Vec::new(),
            search_query: String::new(),
            quick_filter: QuickFilter::All,
            filtered_indices: Vec::new(),
            composing: None,
            view: View::Mail,
            status_message: None,
            loading: false,
            settings: AppSettings::default(),
            account_dialog: None,
            confirm_dialog: None,
            toasts: Vec::new(),
            next_toast_id: 0,
            current_attachments: Vec::new(),
            last_sync: None,
            mail_page: 0,
            mails_per_page: 50,
            has_more_mails: true,
            compose_attachment_path: String::new(),
            first_launch: true, // Will be set based on whether accounts exist
            onboarding_step: 0,
            context_menu: None,
            idle_enabled: true,
            last_known_count: 0,
            templates: crate::mail::signatures::default_templates(),
            show_html: false,
            allow_external_images: false,
            calendar_events: Vec::new(),
            contacts: Vec::new(),
        }
    }
}

impl App {
    /// Add a toast notification.
    pub fn push_toast(&mut self, message: impl Into<String>, level: crate::ui::toast::ToastLevel) {
        let toast = Toast {
            id: self.next_toast_id,
            message: message.into(),
            level,
            created_at: Instant::now(),
            duration_secs: 4,
        };
        self.next_toast_id += 1;
        self.toasts.push(toast);
    }

    /// Remove expired toasts.
    pub fn prune_toasts(&mut self) {
        self.toasts.retain(|t| !t.is_expired());
    }
}

/// State for the "Add Account" dialog.
#[derive(Debug, Clone)]
pub struct AccountDialogState {
    pub email: String,
    pub display_name: String,
    pub password: String,
    pub imap_host: String,
    pub imap_port: String,
    pub smtp_host: String,
    pub smtp_port: String,
    pub provider_name: String,
    pub provider: crate::accounts::provider::ProviderConfig,
    pub testing: bool,
    pub test_result: Option<Result<(), String>>,
    pub show_advanced: bool,
}

impl Default for AccountDialogState {
    fn default() -> Self {
        let cfg = crate::accounts::provider::detect_provider("");
        Self {
            email: String::new(),
            display_name: String::new(),
            password: String::new(),
            imap_host: cfg.imap_host.to_owned(),
            imap_port: cfg.imap_port.to_string(),
            smtp_host: cfg.smtp_host.to_owned(),
            smtp_port: cfg.smtp_port.to_string(),
            provider_name: cfg.provider.label().to_owned(),
            provider: cfg,
            testing: false,
            test_result: None,
            show_advanced: false,
        }
    }
}

impl AccountDialogState {
    /// Re-detect the provider from the current email and update
    /// server fields. Called when the email input changes.
    pub fn update_provider(&mut self) {
        let cfg = crate::accounts::provider::detect_provider(&self.email);
        // Only overwrite host/port when the provider is not Custom,
        // so manually-entered values are preserved for unknown domains.
        if cfg.provider != crate::accounts::provider::Provider::Custom {
            self.imap_host = cfg.imap_host.to_owned();
            self.imap_port = cfg.imap_port.to_string();
            self.smtp_host = cfg.smtp_host.to_owned();
            self.smtp_port = cfg.smtp_port.to_string();
        }
        self.provider_name = cfg.provider.label().to_owned();
        self.provider = cfg;
        // Invalidate any previous test when settings change.
        self.test_result = None;
    }

    /// Parse the IMAP port string to `u16`, defaulting to 993.
    pub fn imap_port_u16(&self) -> u16 {
        self.imap_port.parse().unwrap_or(993)
    }

    /// Parse the SMTP port string to `u16`, defaulting to 587.
    pub fn smtp_port_u16(&self) -> u16 {
        self.smtp_port.parse().unwrap_or(587)
    }

    /// Whether the form has enough data to attempt a connection test.
    pub fn can_test(&self) -> bool {
        !self.email.is_empty()
            && !self.password.is_empty()
            && !self.imap_host.is_empty()
            && !self.smtp_host.is_empty()
            && !self.testing
    }

    /// Whether the form is ready to submit (test passed and name filled).
    pub fn can_submit(&self) -> bool {
        self.test_result.as_ref().is_some_and(|r| r.is_ok())
            && !self.display_name.is_empty()
            && !self.email.is_empty()
    }
}

/// Authentication method for an account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthMethod {
    /// Standard username + password (or app-specific password).
    Basic,
    /// OAuth2 with a stored refresh token.
    OAuth2 {
        /// The OAuth2 refresh token used to obtain new access tokens.
        refresh_token: String,
    },
}

impl Default for AuthMethod {
    fn default() -> Self {
        Self::Basic
    }
}

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FolderType {
    Inbox,
    Sent,
    Drafts,
    Trash,
    Starred,
    Custom,
}

impl Default for FolderType {
    fn default() -> Self {
        Self::Custom
    }
}

/// A single mail message with headers, body, and metadata.
#[derive(Debug, Clone)]
pub struct MailEntry {
    /// Unique message identifier (server UID or generated).
    pub id: String,
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
    /// Full plain-text body.
    pub body_text: String,
    /// Full HTML body, if available.
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
}

impl Default for MailEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
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
        }
    }
}

/// A file attached to a compose draft.
#[derive(Debug, Clone)]
pub struct ComposeAttachment {
    /// Full path to the file on disk.
    pub path: std::path::PathBuf,
    /// Display filename (basename).
    pub filename: String,
    /// MIME content type (e.g. "application/pdf").
    pub content_type: String,
    /// File size in bytes.
    pub size: usize,
}

/// A message being composed (new, reply, or forward).
#[derive(Debug, Clone)]
pub struct ComposeDraft {
    /// Recipient address(es), comma-separated.
    pub to: String,
    /// CC address(es), comma-separated.
    pub cc: String,
    /// BCC address(es), comma-separated.
    pub bcc: String,
    /// Subject line.
    pub subject: String,
    /// Message body text.
    pub body: String,
    /// If this is a reply, the original message ID.
    pub reply_to: Option<String>,
    /// Account to send from.
    pub account_id: String,
    /// File attachments to include with the message.
    pub attachments: Vec<ComposeAttachment>,
}

impl Default for ComposeDraft {
    fn default() -> Self {
        Self {
            to: String::new(),
            cc: String::new(),
            bcc: String::new(),
            subject: String::new(),
            body: String::new(),
            reply_to: None,
            account_id: String::new(),
            attachments: Vec::new(),
        }
    }
}

/// Top-level view the application can display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    /// The main mail list and reader pane.
    Mail,
    /// The compose/reply window.
    Compose,
    /// The settings panel.
    Settings,
    /// Calendar view (event list and scheduling).
    Calendar,
    /// Contacts view (address book).
    Contacts,
}

impl Default for View {
    fn default() -> Self {
        Self::Mail
    }
}

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
