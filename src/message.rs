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
    LoadMoreMails,
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

    // ── Compose attachments ──────────────────────────────────────────

    /// Text input for the attachment file path changed.
    ComposeAttachmentPathChanged(String),
    /// User clicked "Add" to attach the file at the typed path.
    AddAttachment,
    /// Result of reading the file metadata after adding an attachment.
    AttachmentPicked(Result<Vec<std::path::PathBuf>, String>),
    /// Remove a compose attachment by index.
    RemoveAttachment(usize),

    // ── Server-side search ────────────────────────────────────────────

    /// Trigger a server-side IMAP SEARCH with the given query.
    ServerSearch(String),
    /// Results from a server-side IMAP SEARCH.
    ServerSearchResults(Result<Vec<MailEntry>, String>),

    // ── IMAP / SMTP async results ───────────────────────────────────

    MailsFetched(Result<Vec<MailEntry>, String>),
    MoreMailsFetched(Result<Vec<MailEntry>, String>),
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

    // ── Signatures & Templates ────────────────────────────────────

    /// Update the signature text for the currently selected account.
    EditSignature(String),
    /// Persist the current account signature (in-memory for now).
    SaveSignature,
    /// Apply a template by index to the current compose draft.
    ApplyTemplate(usize),
    /// Start a new compose with a template by index.
    ComposeWithTemplate(usize),

    // ── Settings ────────────────────────────────────────────────────

    OpenSettings,
    CloseSettings,
    SettingsThemeChanged(String),
    SettingsFontSizeChanged(String),
    SettingsCheckIntervalChanged(String),
    SettingsToggleNotifications,
    SettingsReadingPaneChanged(String),
    SettingsDensityChanged(String),
    SettingsLanguageChanged(String),
    SaveSettings,

    // ── Folder management ───────────────────────────────────────────

    CreateFolder(String),
    RenameFolder(String, String),
    DeleteFolder(String),
    EmptyTrash,
    FolderOpCompleted(Result<(), String>),

    // ── Onboarding ─────────────────────────────────────────────────

    /// Advance to the next onboarding step.
    OnboardingNext,
    /// Skip onboarding entirely.
    OnboardingSkip,
    /// Finish onboarding (set first_launch = false).
    OnboardingComplete,

    // ── Context menu ───────────────────────────────────────────────

    /// Show a context menu at a given position for a target.
    ShowContextMenu(crate::state::ContextMenu),
    /// Dismiss the context menu.
    CloseContextMenu,

    // ── IMAP IDLE ────────────────────────────────────────────────────

    /// Emitted when IDLE or polling detects new mail in the active folder.
    IdleNewMail,
    /// Emitted when IDLE encounters an error (will fall back to polling).
    IdleError(String),
    /// Toggle IDLE push notifications on or off.
    ToggleIdle,

    // ── OAuth2 ──────────────────────────────────────────────────────

    /// Start the OAuth2 browser flow for the given provider.
    StartOAuth(crate::accounts::provider::Provider),
    /// OAuth2 flow completed: Ok((access_token, refresh_token)) or Err(message).
    OAuthCompleted(Result<(String, String), String>),

    // ── HTML view ──────────────────────────────────────────────────

    /// Toggle between plain-text and HTML display of the selected email.
    ToggleHtmlView,
    /// Allow loading external images in the current HTML email.
    AllowExternalImages,

    // ── Import / Export ────────────────────────────────────────────

    /// Trigger file selection for importing mail (EML or mbox).
    ImportMail,
    /// Export a specific mail entry by index.
    ExportMail(usize),
    /// Import operation completed with parsed mail entries (or error).
    ImportCompleted(Result<Vec<MailEntry>, String>),
    /// Export operation completed with the output file path (or error).
    ExportCompleted(Result<std::path::PathBuf, String>),

    // ── System ──────────────────────────────────────────────────────

    Tick,
    ClearStatus,
    FontLoaded(Result<(), iced::font::Error>),
}
