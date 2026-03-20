//! Iced message types for Exospine.
//!
//! Every user interaction, async result, and system event is represented
//! as a variant of [`Message`]. The `update` function in the application
//! matches on these to advance state.

use crate::state::{Account, Folder, MailEntry, View};
use crate::ui::confirm_dialog::ConfirmAction;

/// The unified message type driven through Iced's runtime.
#[derive(Debug, Clone)]
pub enum Message {
    // ── Navigation ──────────────────────────────────────────────────

    SelectAccount(usize),
    SelectFolder(String),
    SelectMail(usize),
    SwitchView(View),
    SelectNextMail,
    SelectPrevMail,

    // ── Search & Filter ─────────────────────────────────────────────

    SearchChanged(String),
    FocusSearch,
    SetQuickFilter(crate::search::QuickFilter),

    // ── Mail actions ────────────────────────────────────────────────

    MarkAsRead(usize),
    MarkAsUnread(usize),
    ToggleStar(usize),
    DeleteMail(usize),
    DeleteSelected,
    ArchiveMail(usize),
    MoveMail(usize, String),
    RefreshFolder,
    FlagSynced(Result<(), String>),

    // ── Compose ─────────────────────────────────────────────────────

    NewMail,
    Reply,
    ReplyAll,
    Forward,
    ComposeToChanged(String),
    ComposeCcChanged(String),
    ComposeBccChanged(String),
    ComposeSubjectChanged(String),
    ComposeBodyChanged(String),
    SendMail,
    SaveDraft,
    DiscardDraft,

    // ── IMAP / SMTP async results ───────────────────────────────────

    MailsFetched(Result<Vec<MailEntry>, String>),
    FoldersFetched(Result<Vec<Folder>, String>),
    MailSent(Result<(), String>),

    // ── Account management ──────────────────────────────────────────

    AddAccount,
    CancelAddAccount,
    RemoveAccount(usize),
    AccountAdded(Result<Account, String>),

    // ── Account dialog ──────────────────────────────────────────────

    DialogEmailChanged(String),
    DialogNameChanged(String),
    DialogPasswordChanged(String),
    DialogImapHostChanged(String),
    DialogImapPortChanged(String),
    DialogSmtpHostChanged(String),
    DialogSmtpPortChanged(String),
    DialogToggleAdvanced,
    DialogTestConnection,
    DialogTestResult(Result<(), String>),
    DialogSubmitAccount,

    // ── Attachments ─────────────────────────────────────────────────

    /// Download a single attachment: (mail_index, part_index).
    DownloadAttachment(usize, usize),
    AttachmentDownloaded(Result<std::path::PathBuf, String>),
    OpenAttachment(std::path::PathBuf),

    // ── Confirmation dialog ─────────────────────────────────────────

    ShowConfirm(crate::ui::confirm_dialog::ConfirmDialog),
    ConfirmYes(ConfirmAction),
    ConfirmCancel,

    // ── Toast notifications ─────────────────────────────────────────

    DismissToast(u64),
    TickToasts,

    // ── Keyboard ────────────────────────────────────────────────────

    EscapePressed,
    KeyboardEvent(iced::keyboard::Event),

    // ── Settings ────────────────────────────────────────────────────

    OpenSettings,
    CloseSettings,

    // ── Folder management ───────────────────────────────────────────

    CreateFolder(String),
    RenameFolder(String, String),
    DeleteFolder(String),
    EmptyTrash,
    FolderOpCompleted(Result<(), String>),

    // ── System ──────────────────────────────────────────────────────

    Tick,
    ClearStatus,
    FontLoaded(Result<(), iced::font::Error>),
}
