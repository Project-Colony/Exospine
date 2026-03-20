//! Application state types for Exospine.
//!
//! This module defines all the core data structures that make up
//! the application state, including accounts, mail entries, folders,
//! compose drafts, and settings.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The main Iced application state.
///
/// Holds everything needed to render the UI and manage user interactions,
/// including loaded accounts, the current selection state, mail entries
/// in the active folder, and application-wide settings.
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
            composing: None,
            view: View::Mail,
            status_message: None,
            loading: false,
            settings: AppSettings::default(),
        }
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
        }
    }
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
}

impl Default for View {
    fn default() -> Self {
        Self::Mail
    }
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
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            font_size: 14,
            check_interval_secs: 300,
            show_notifications: true,
        }
    }
}
