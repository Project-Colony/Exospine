//! Iced message types for Exospine.
//!
//! Every user interaction, async result, and system event is represented
//! as a variant of [`Message`]. The `update` function in the application
//! matches on these to advance state.

use crate::state::{Account, Folder, MailEntry, View};

/// The unified message type driven through Iced's runtime.
#[derive(Debug, Clone)]
pub enum Message {
    // ── Navigation ──────────────────────────────────────────────────

    /// Select an account by its index in `App::accounts`.
    SelectAccount(usize),
    /// Select a folder by name within the current account.
    SelectFolder(String),
    /// Select a mail entry by its index in `App::mail_entries`.
    SelectMail(usize),
    /// Switch to a different top-level view.
    SwitchView(View),

    // ── Search ──────────────────────────────────────────────────────

    /// The search query text changed.
    SearchChanged(String),

    // ── Mail actions ────────────────────────────────────────────────

    /// Mark a mail entry as read by index.
    MarkAsRead(usize),
    /// Mark a mail entry as unread by index.
    MarkAsUnread(usize),
    /// Toggle the starred/flagged state of a mail entry by index.
    ToggleStar(usize),
    /// Delete a mail entry by index (move to trash or permanent delete).
    DeleteMail(usize),
    /// Archive a mail entry by index.
    ArchiveMail(usize),
    /// Move a mail entry to a different folder: `(mail_index, target_folder)`.
    MoveMail(usize, String),
    /// Re-fetch the contents of the currently selected folder.
    RefreshFolder,

    // ── Compose ─────────────────────────────────────────────────────

    /// Begin composing a new mail from scratch.
    NewMail,
    /// Reply to the currently selected message.
    Reply,
    /// Reply-all to the currently selected message.
    ReplyAll,
    /// Forward the currently selected message.
    Forward,
    /// The "To" field changed in the compose view.
    ComposeToChanged(String),
    /// The "Cc" field changed in the compose view.
    ComposeCcChanged(String),
    /// The "Bcc" field changed in the compose view.
    ComposeBccChanged(String),
    /// The subject field changed in the compose view.
    ComposeSubjectChanged(String),
    /// The body text changed in the compose view.
    ComposeBodyChanged(String),
    /// Send the current draft.
    SendMail,
    /// Save the current draft without sending.
    SaveDraft,
    /// Discard the current draft entirely.
    DiscardDraft,

    // ── IMAP / SMTP async results ───────────────────────────────────

    /// A batch of mail entries was fetched from the server.
    MailsFetched(Result<Vec<MailEntry>, String>),
    /// Folder list was fetched from the server.
    FoldersFetched(Result<Vec<Folder>, String>),
    /// Result of an outgoing mail send attempt.
    MailSent(Result<(), String>),

    // ── Account management ──────────────────────────────────────────

    /// Open the "add account" flow.
    AddAccount,
    /// Remove an account by its index.
    RemoveAccount(usize),
    /// An account was successfully added (or failed).
    AccountAdded(Result<Account, String>),

    // ── Settings ────────────────────────────────────────────────────

    /// Open the settings panel.
    OpenSettings,
    /// Close the settings panel and return to the previous view.
    CloseSettings,

    // ── System ──────────────────────────────────────────────────────

    /// Periodic tick for background polling (new-mail checks, etc.).
    Tick,
    /// Clear the transient status bar message.
    ClearStatus,
    /// Result of loading a custom font.
    FontLoaded(Result<(), iced::font::Error>),
}
