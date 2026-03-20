use crate::message::Message;
use crate::state::{Account, AccountDialogState, App, ComposeDraft, View};
use crate::ui::toast::ToastLevel;
use iced::Task;

// ── Helper functions ──────────────────────────────────────────────────

/// Retrieve a clone of the currently selected account, if any.
fn get_selected_account(app: &App) -> Option<Account> {
    app.selected_account
        .and_then(|idx| app.accounts.get(idx))
        .cloned()
}

/// Retrieve a clone of an account by its unique ID.
fn get_account_by_id(app: &App, id: &str) -> Option<Account> {
    app.accounts.iter().find(|a| a.id == id).cloned()
}

/// Retrieve the password for an account via `spawn_blocking` since keyring is sync I/O.
async fn fetch_password(account_id: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        crate::accounts::keyring_store::get_password(&account_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Recompute `app.filtered_indices` from the current search query and quick filter.
fn refilter(app: &mut App) {
    let by_query = crate::search::filter_entries(&app.mail_entries, &app.search_query);
    if app.quick_filter == crate::search::QuickFilter::All {
        app.filtered_indices = by_query;
    } else {
        let qf: std::collections::HashSet<usize> =
            crate::search::apply_quick_filter(&app.mail_entries, &app.quick_filter)
                .into_iter()
                .collect();
        app.filtered_indices = by_query.into_iter().filter(|i| qf.contains(i)).collect();
    }
}

// ── IMAP flag operations ──────────────────────────────────────────────

/// The type of IMAP flag/mail operation to perform in the background.
enum FlagOp {
    MarkRead,
    MarkUnread,
    AddStar,
    RemoveStar,
    Delete,
    Archive,
    Move(String),
}

/// Spawn an async task to sync a flag/mail change to the IMAP server.
/// Reads the UID and folder from the mail entry at `mail_idx` before the entry
/// is mutated, so call this **before** modifying the local entry.
fn sync_flag_task(app: &App, mail_idx: usize, op: FlagOp) -> Task<Message> {
    let entry = match app.mail_entries.get(mail_idx) {
        Some(e) => e,
        None => return Task::none(),
    };
    let uid = entry.id.clone();
    let folder = app
        .selected_folder
        .clone()
        .unwrap_or_else(|| entry.folder.clone());
    let account = match get_selected_account(app) {
        Some(a) => a,
        None => return Task::none(),
    };
    let account_id = account.id.clone();

    Task::perform(
        async move {
            let password = fetch_password(account_id).await?;
            let mut session = crate::mail::imap::connect(&account, &password)
                .await
                .map_err(|e| e.to_string())?;

            // Select the folder so UID commands target the right mailbox.
            session
                .select(&folder)
                .await
                .map_err(|e| format!("Select failed: {e}"))?;

            match op {
                FlagOp::MarkRead => {
                    crate::mail::imap::mark_as_read(&mut session, &uid)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                FlagOp::MarkUnread => {
                    crate::mail::imap::mark_as_unread(&mut session, &uid)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                FlagOp::AddStar => {
                    crate::mail::imap::set_flagged(&mut session, &uid)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                FlagOp::RemoveStar => {
                    crate::mail::imap::remove_flagged(&mut session, &uid)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                FlagOp::Delete => {
                    crate::mail::imap::delete_message(&mut session, &uid)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                FlagOp::Archive => {
                    crate::mail::imap::move_message(&mut session, &uid, "Archive")
                        .await
                        .map_err(|e| e.to_string())?;
                }
                FlagOp::Move(target) => {
                    crate::mail::imap::move_message(&mut session, &uid, &target)
                        .await
                        .map_err(|e| e.to_string())?;
                }
            }

            let _ = session.logout().await;
            Ok(())
        },
        Message::FlagSynced,
    )
}

// ── Main update function ──────────────────────────────────────────────

/// Handle all application messages and return the corresponding command.
pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        // ── Navigation ──────────────────────────────────────────────
        Message::SelectAccount(idx) => {
            if idx < app.accounts.len() {
                app.selected_account = Some(idx);
                app.selected_folder = Some("INBOX".to_string());
                app.selected_mail = None;
                app.mail_entries.clear();
                app.current_attachments.clear();
                refilter(app);

                // Fetch folders for the newly selected account.
                let account = app.accounts[idx].clone();
                let account_id = account.id.clone();

                let fetch_folders = Task::perform(
                    async move {
                        let password = fetch_password(account_id).await?;
                        let mut session = crate::mail::imap::connect(&account, &password)
                            .await
                            .map_err(|e| e.to_string())?;
                        let folders = crate::mail::imap::fetch_folders(&mut session)
                            .await
                            .map_err(|e| e.to_string())?;
                        let _ = session.logout().await;
                        Ok(folders)
                    },
                    Message::FoldersFetched,
                );

                // Also trigger a folder refresh to load INBOX messages.
                let refresh = Task::perform(async {}, |_| Message::RefreshFolder);

                return Task::batch([fetch_folders, refresh]);
            }
            Task::none()
        }
        Message::SelectFolder(folder) => {
            app.selected_folder = Some(folder);
            app.selected_mail = None;
            app.current_attachments.clear();
            Task::perform(async {}, |_| Message::RefreshFolder)
        }
        Message::SelectMail(idx) => {
            app.selected_mail = Some(idx);

            // Capture data we need before borrowing mutably.
            let needs_read_sync = app
                .mail_entries
                .get(idx)
                .map_or(false, |e| !e.is_read);
            let uid = app
                .mail_entries
                .get(idx)
                .map(|e| e.id.clone());
            let folder = app.selected_folder.clone();
            let account = get_selected_account(app);

            // Mark as read locally.
            if let Some(entry) = app.mail_entries.get_mut(idx) {
                if !entry.is_read {
                    entry.is_read = true;
                }
            }
            refilter(app);

            // Sync \Seen flag to the server if it was unread.
            if needs_read_sync {
                if let (Some(uid), Some(account), Some(folder)) = (uid, account, folder) {
                    let account_id = account.id.clone();
                    return Task::perform(
                        async move {
                            let password = fetch_password(account_id).await?;
                            let mut session =
                                crate::mail::imap::connect(&account, &password)
                                    .await
                                    .map_err(|e| e.to_string())?;
                            session
                                .select(&folder)
                                .await
                                .map_err(|e| e.to_string())?;
                            crate::mail::imap::mark_as_read(&mut session, &uid)
                                .await
                                .map_err(|e| e.to_string())?;
                            let _ = session.logout().await;
                            Ok(())
                        },
                        Message::FlagSynced,
                    );
                }
            }
            Task::none()
        }
        Message::SelectNextMail => {
            let len = app.mail_entries.len();
            if len > 0 {
                let next = match app.selected_mail {
                    Some(i) if i + 1 < len => i + 1,
                    Some(i) => i,
                    None => 0,
                };
                app.selected_mail = Some(next);
            }
            Task::none()
        }
        Message::SelectPrevMail => {
            if !app.mail_entries.is_empty() {
                let prev = match app.selected_mail {
                    Some(i) if i > 0 => i - 1,
                    Some(i) => i,
                    None => 0,
                };
                app.selected_mail = Some(prev);
            }
            Task::none()
        }
        Message::SwitchView(view) => {
            app.view = view;
            Task::none()
        }

        // ── Search & Filter ─────────────────────────────────────────
        Message::SearchChanged(query) => {
            app.search_query = query.clone();
            refilter(app);
            // Trigger server-side search when query is 3+ characters.
            if query.len() >= 3 {
                return Task::perform(async {}, move |_| Message::ServerSearch(query));
            }
            Task::none()
        }
        Message::ServerSearch(query) => {
            let account = get_selected_account(app);
            let folder = app.selected_folder.clone();
            if let (Some(account), Some(folder)) = (account, folder) {
                app.loading = true;
                app.status_message = Some("Searching...".to_string());
                let account_id = account.id.clone();
                return Task::perform(
                    async move {
                        let password = fetch_password(account_id).await?;
                        let mut session = crate::mail::imap::connect(&account, &password)
                            .await
                            .map_err(|e| e.to_string())?;
                        let results =
                            crate::mail::imap::search_messages(&mut session, &folder, &query, 50)
                                .await
                                .map_err(|e| e.to_string())?;
                        let _ = session.logout().await;
                        Ok(results)
                    },
                    Message::ServerSearchResults,
                );
            }
            Task::none()
        }
        Message::ServerSearchResults(result) => {
            app.loading = false;
            match result {
                Ok(mails) => {
                    app.mail_entries = mails;
                    app.status_message = None;
                    refilter(app);
                }
                Err(e) => {
                    app.status_message = Some(format!("Search error: {}", e));
                    app.push_toast(format!("Search error: {e}"), ToastLevel::Error);
                }
            }
            Task::none()
        }
        Message::FocusSearch => {
            // TODO: focus the search text_input widget via Id
            Task::none()
        }
        Message::SetQuickFilter(filter) => {
            app.quick_filter = filter;
            refilter(app);
            Task::none()
        }

        // ── Mail Actions ────────────────────────────────────────────
        Message::MarkAsRead(idx) => {
            let task = sync_flag_task(app, idx, FlagOp::MarkRead);
            if let Some(entry) = app.mail_entries.get_mut(idx) {
                entry.is_read = true;
            }
            refilter(app);
            task
        }
        Message::MarkAsUnread(idx) => {
            let task = sync_flag_task(app, idx, FlagOp::MarkUnread);
            if let Some(entry) = app.mail_entries.get_mut(idx) {
                entry.is_read = false;
            }
            refilter(app);
            task
        }
        Message::ToggleStar(idx) => {
            let starred = app
                .mail_entries
                .get(idx)
                .map_or(false, |e| e.is_starred);
            let op = if starred {
                FlagOp::RemoveStar
            } else {
                FlagOp::AddStar
            };
            let task = sync_flag_task(app, idx, op);
            if let Some(entry) = app.mail_entries.get_mut(idx) {
                entry.is_starred = !entry.is_starred;
            }
            refilter(app);
            task
        }
        Message::DeleteMail(idx) => {
            let task = sync_flag_task(app, idx, FlagOp::Delete);
            if idx < app.mail_entries.len() {
                app.mail_entries.remove(idx);
                app.selected_mail = None;
                app.current_attachments.clear();
            }
            refilter(app);
            task
        }
        Message::DeleteSelected => {
            if let Some(idx) = app.selected_mail {
                let task = sync_flag_task(app, idx, FlagOp::Delete);
                if idx < app.mail_entries.len() {
                    app.mail_entries.remove(idx);
                    app.current_attachments.clear();
                    if app.mail_entries.is_empty() {
                        app.selected_mail = None;
                    } else if idx >= app.mail_entries.len() {
                        app.selected_mail = Some(app.mail_entries.len() - 1);
                    }
                }
                refilter(app);
                return task;
            }
            Task::none()
        }
        Message::ArchiveMail(idx) => {
            let task = sync_flag_task(app, idx, FlagOp::Archive);
            if idx < app.mail_entries.len() {
                app.mail_entries.remove(idx);
                app.selected_mail = None;
                app.current_attachments.clear();
                app.push_toast("Message archived", ToastLevel::Success);
            }
            refilter(app);
            task
        }
        Message::MoveMail(idx, ref target_folder) => {
            let target = target_folder.clone();
            let task = sync_flag_task(app, idx, FlagOp::Move(target.clone()));
            if idx < app.mail_entries.len() {
                app.mail_entries.remove(idx);
                app.selected_mail = None;
            }
            refilter(app);
            task
        }
        Message::RefreshFolder => {
            app.loading = true;
            app.status_message = Some("Refreshing...".to_string());
            app.mail_page = 0;
            app.has_more_mails = true;

            let account = get_selected_account(app);
            let folder = app.selected_folder.clone();

            if let (Some(account), Some(folder)) = (account, folder) {
                let account_id = account.id.clone();
                Task::perform(
                    async move {
                        let password = fetch_password(account_id).await?;
                        let mut session =
                            crate::mail::connection::connect_with_retry(&account, &password, 3)
                                .await
                                .map_err(|e| e.to_string())?;
                        let mails =
                            crate::mail::imap::fetch_messages(&mut session, &folder, 50)
                                .await
                                .map_err(|e| e.to_string())?;
                        let _ = session.logout().await;
                        Ok(mails)
                    },
                    Message::MailsFetched,
                )
            } else {
                app.loading = false;
                app.status_message = None;
                Task::none()
            }
        }
        Message::LoadMoreMails => {
            // Load the next batch of older messages, offset by current count.
            let account = get_selected_account(app);
            let folder = app.selected_folder.clone();
            let current_count = app.mail_entries.len() as u32;

            if let (Some(account), Some(folder)) = (account, folder) {
                app.loading = true;
                let account_id = account.id.clone();
                return Task::perform(
                    async move {
                        let password = fetch_password(account_id).await?;
                        let mut session =
                            crate::mail::imap::connect(&account, &password)
                                .await
                                .map_err(|e| e.to_string())?;
                        // Fetch next 50 beyond what we already have.
                        let mails = crate::mail::imap::fetch_messages(
                            &mut session,
                            &folder,
                            current_count + 50,
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                        let _ = session.logout().await;
                        // Only return the messages beyond what we already have.
                        let new_mails = if (mails.len() as u32) > current_count {
                            mails
                                .into_iter()
                                .skip(current_count as usize)
                                .collect()
                        } else {
                            Vec::new()
                        };
                        Ok(new_mails)
                    },
                    Message::MoreMailsFetched,
                );
            }
            Task::none()
        }
        Message::FlagSynced(result) => {
            if let Err(e) = result {
                app.push_toast(format!("Sync error: {e}"), ToastLevel::Error);
            }
            Task::none()
        }

        // ── Compose ─────────────────────────────────────────────────
        Message::NewMail => {
            let account_id = app
                .selected_account
                .and_then(|i| app.accounts.get(i))
                .map(|a| a.id.clone())
                .unwrap_or_default();
            app.composing = Some(ComposeDraft {
                to: String::new(),
                cc: String::new(),
                bcc: String::new(),
                subject: String::new(),
                body: String::new(),
                reply_to: None,
                account_id,
                attachments: Vec::new(),
            });
            app.view = View::Compose;
            Task::none()
        }
        Message::Reply => {
            if let Some(idx) = app.selected_mail {
                if let Some(mail) = app.mail_entries.get(idx) {
                    let account_id = mail.account_id.clone();
                    app.composing = Some(ComposeDraft {
                        to: mail.from.clone(),
                        cc: String::new(),
                        bcc: String::new(),
                        subject: format!("Re: {}", mail.subject),
                        body: format!(
                            "\n\n--- Original Message ---\n{}",
                            mail.body_text
                        ),
                        reply_to: Some(mail.id.clone()),
                        account_id,
                        attachments: Vec::new(),
                    });
                    app.compose_attachment_path.clear();
                    app.view = View::Compose;
                }
            }
            Task::none()
        }
        Message::ReplyAll => {
            if let Some(idx) = app.selected_mail {
                if let Some(mail) = app.mail_entries.get(idx) {
                    let account_id = mail.account_id.clone();
                    let all_recipients = mail.to.join(", ");
                    app.composing = Some(ComposeDraft {
                        to: mail.from.clone(),
                        cc: all_recipients,
                        bcc: String::new(),
                        subject: format!("Re: {}", mail.subject),
                        body: format!(
                            "\n\n--- Original Message ---\n{}",
                            mail.body_text
                        ),
                        reply_to: Some(mail.id.clone()),
                        account_id,
                        attachments: Vec::new(),
                    });
                    app.compose_attachment_path.clear();
                    app.view = View::Compose;
                }
            }
            Task::none()
        }
        Message::Forward => {
            if let Some(idx) = app.selected_mail {
                if let Some(mail) = app.mail_entries.get(idx) {
                    let account_id = mail.account_id.clone();
                    app.composing = Some(ComposeDraft {
                        to: String::new(),
                        cc: String::new(),
                        bcc: String::new(),
                        subject: format!("Fwd: {}", mail.subject),
                        body: format!(
                            "\n\n--- Forwarded Message ---\nFrom: {}\nSubject: {}\n\n{}",
                            mail.from, mail.subject, mail.body_text
                        ),
                        reply_to: None,
                        account_id,
                        attachments: Vec::new(),
                    });
                    app.view = View::Compose;
                }
            }
            Task::none()
        }
        Message::ComposeToChanged(val) => {
            if let Some(ref mut draft) = app.composing {
                draft.to = val;
            }
            Task::none()
        }
        Message::ComposeCcChanged(val) => {
            if let Some(ref mut draft) = app.composing {
                draft.cc = val;
            }
            Task::none()
        }
        Message::ComposeBccChanged(val) => {
            if let Some(ref mut draft) = app.composing {
                draft.bcc = val;
            }
            Task::none()
        }
        Message::ComposeSubjectChanged(val) => {
            if let Some(ref mut draft) = app.composing {
                draft.subject = val;
            }
            Task::none()
        }
        Message::ComposeBodyChanged(val) => {
            if let Some(ref mut draft) = app.composing {
                draft.body = val;
            }
            Task::none()
        }
        Message::SendMail => {
            if let Some(draft) = app.composing.take() {
                app.view = View::Mail;
                app.status_message = Some("Sending...".to_string());

                let account = get_account_by_id(app, &draft.account_id);
                if let Some(account) = account {
                    let account_id = account.id.clone();
                    let reply_to = draft.reply_to.clone();
                    return Task::perform(
                        async move {
                            let password = fetch_password(account_id).await?;
                            crate::mail::smtp::send_mail(
                                &account,
                                &password,
                                &draft,
                                reply_to.as_deref(),
                            )
                            .await
                            .map_err(|e| e.to_string())
                        },
                        Message::MailSent,
                    );
                }
                app.status_message = Some("No account found".to_string());
            }
            Task::none()
        }
        Message::SaveDraft => {
            if let Some(ref draft) = app.composing {
                app.status_message = Some("Saving draft...".to_string());
                let account = get_account_by_id(app, &draft.account_id);
                let draft_rfc = format!(
                    "From: {from}\r\nTo: {to}\r\nSubject: {subject}\r\n\r\n{body}",
                    from = account
                        .as_ref()
                        .map_or("", |a| a.email.as_str()),
                    to = draft.to,
                    subject = draft.subject,
                    body = draft.body,
                );

                if let Some(account) = account {
                    let account_id = account.id.clone();
                    let rfc822_bytes = draft_rfc.into_bytes();
                    return Task::perform(
                        async move {
                            let password = fetch_password(account_id).await?;
                            let mut session =
                                crate::mail::imap::connect(&account, &password)
                                    .await
                                    .map_err(|e| e.to_string())?;
                            crate::mail::imap::append_message(
                                &mut session,
                                "Drafts",
                                &rfc822_bytes,
                            )
                            .await
                            .map_err(|e| e.to_string())?;
                            let _ = session.logout().await;
                            Ok(())
                        },
                        |r: Result<(), String>| match r {
                            Ok(()) => {
                                Message::MailSent(Ok(()))
                            }
                            Err(e) => {
                                Message::FlagSynced(Err(format!("Save draft failed: {e}")))
                            }
                        },
                    );
                }
                app.status_message = Some("Draft saved locally".to_string());
            }
            Task::none()
        }
        Message::DiscardDraft => {
            app.composing = None;
            app.compose_attachment_path.clear();
            app.view = View::Mail;
            Task::none()
        }

        // ── Compose Attachments ────────────────────────────────────
        Message::ComposeAttachmentPathChanged(val) => {
            app.compose_attachment_path = val;
            Task::none()
        }
        Message::AddAttachment => {
            let path_str = app.compose_attachment_path.trim().to_string();
            if path_str.is_empty() {
                return Task::none();
            }
            let path = std::path::PathBuf::from(&path_str);
            match std::fs::metadata(&path) {
                Ok(meta) if meta.is_file() => {
                    let filename = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path_str.clone());
                    let content_type =
                        crate::mail::smtp::guess_content_type(&path).to_string();
                    let size = meta.len() as usize;
                    if let Some(ref mut draft) = app.composing {
                        draft.attachments.push(crate::state::ComposeAttachment {
                            path,
                            filename,
                            content_type,
                            size,
                        });
                    }
                    app.compose_attachment_path.clear();
                    app.push_toast("Attachment added", ToastLevel::Success);
                }
                Ok(_) => {
                    app.push_toast("Path is not a file", ToastLevel::Error);
                }
                Err(e) => {
                    app.push_toast(format!("Cannot read file: {e}"), ToastLevel::Error);
                }
            }
            Task::none()
        }
        Message::AttachmentPicked(result) => {
            match result {
                Ok(paths) => {
                    for p in paths {
                        if let Ok(meta) = std::fs::metadata(&p) {
                            let filename = p
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            let content_type =
                                crate::mail::smtp::guess_content_type(&p).to_string();
                            let size = meta.len() as usize;
                            if let Some(ref mut draft) = app.composing {
                                draft.attachments.push(crate::state::ComposeAttachment {
                                    path: p,
                                    filename,
                                    content_type,
                                    size,
                                });
                            }
                        }
                    }
                }
                Err(e) => {
                    app.push_toast(format!("Attachment error: {e}"), ToastLevel::Error);
                }
            }
            Task::none()
        }
        Message::RemoveAttachment(idx) => {
            if let Some(ref mut draft) = app.composing {
                if idx < draft.attachments.len() {
                    let name = draft.attachments[idx].filename.clone();
                    draft.attachments.remove(idx);
                    app.push_toast(format!("Removed {name}"), ToastLevel::Info);
                }
            }
            Task::none()
        }

        // ── Async Results ───────────────────────────────────────────
        Message::MailsFetched(result) => {
            app.loading = false;
            match result {
                Ok(mails) => {
                    app.mail_entries = mails;
                    app.status_message = None;
                    app.last_sync = Some(std::time::Instant::now());
                    refilter(app);
                }
                Err(e) => {
                    app.status_message = Some(format!("Error: {}", e));
                    app.push_toast(format!("Fetch error: {e}"), ToastLevel::Error);
                }
            }
            Task::none()
        }
        Message::MoreMailsFetched(result) => {
            app.loading = false;
            match result {
                Ok(mut mails) => {
                    let fetched_count = mails.len() as u32;
                    app.has_more_mails = fetched_count >= app.mails_per_page;
                    app.mail_page += 1;
                    app.mail_entries.append(&mut mails);
                    app.status_message = None;
                    refilter(app);
                }
                Err(e) => {
                    app.status_message = Some(format!("Error: {}", e));
                    app.push_toast(format!("Load more error: {e}"), ToastLevel::Error);
                }
            }
            Task::none()
        }
        Message::FoldersFetched(result) => {
            match result {
                Ok(folders) => {
                    if let Some(idx) = app.selected_account {
                        if let Some(account) = app.accounts.get_mut(idx) {
                            account.folders = folders;
                        }
                    }
                }
                Err(e) => {
                    app.status_message = Some(format!("Error: {}", e));
                }
            }
            Task::none()
        }
        Message::MailSent(result) => {
            match result {
                Ok(()) => {
                    app.status_message = None;
                    app.push_toast("Message sent successfully", ToastLevel::Success);
                }
                Err(e) => {
                    app.status_message = None;
                    app.push_toast(format!("Send failed: {e}"), ToastLevel::Error);
                }
            }
            Task::none()
        }

        // ── OAuth2 ─────────────────────────────────────────────────
        Message::StartOAuth(_provider) => {
            app.push_toast("OAuth2 flow starting...", ToastLevel::Info);
            // TODO: launch browser-based OAuth2 flow
            Task::none()
        }
        Message::OAuthCompleted(result) => {
            match result {
                Ok((_access_token, _refresh_token)) => {
                    app.push_toast("OAuth2 authenticated", ToastLevel::Success);
                }
                Err(e) => {
                    app.push_toast(format!("OAuth2 failed: {e}"), ToastLevel::Error);
                }
            }
            Task::none()
        }

        // ── Account Management ──────────────────────────────────────
        Message::AddAccount => {
            app.account_dialog = Some(AccountDialogState::default());
            Task::none()
        }
        Message::CancelAddAccount => {
            app.account_dialog = None;
            Task::none()
        }
        Message::RemoveAccount(idx) => {
            if idx < app.accounts.len() {
                let removed = app.accounts.remove(idx);
                // Delete credentials from keyring in the background.
                let account_id = removed.id;
                tokio::task::spawn_blocking(move || {
                    let _ = crate::accounts::keyring_store::delete_password(&account_id);
                });
                app.selected_account = None;
                app.selected_folder = None;
                app.mail_entries.clear();
                app.current_attachments.clear();
                refilter(app);
            }
            Task::none()
        }
        Message::AccountAdded(result) => {
            match result {
                Ok(account) => {
                    app.accounts.push(account);
                    app.account_dialog = None;
                    app.push_toast("Account added", ToastLevel::Success);
                }
                Err(e) => {
                    if let Some(ref mut dialog) = app.account_dialog {
                        dialog.test_result = Some(Err(e.clone()));
                    }
                    app.push_toast(format!("Error: {e}"), ToastLevel::Error);
                }
            }
            Task::none()
        }

        // ── Account Dialog ──────────────────────────────────────────
        Message::DialogEmailChanged(val) => {
            if let Some(ref mut d) = app.account_dialog {
                d.email = val;
                d.update_provider();
            }
            Task::none()
        }
        Message::DialogNameChanged(val) => {
            if let Some(ref mut d) = app.account_dialog {
                d.display_name = val;
            }
            Task::none()
        }
        Message::DialogPasswordChanged(val) => {
            if let Some(ref mut d) = app.account_dialog {
                d.password = val;
                d.test_result = None;
            }
            Task::none()
        }
        Message::DialogImapHostChanged(val) => {
            if let Some(ref mut d) = app.account_dialog {
                d.imap_host = val;
                d.test_result = None;
            }
            Task::none()
        }
        Message::DialogImapPortChanged(val) => {
            if let Some(ref mut d) = app.account_dialog {
                d.imap_port = val;
                d.test_result = None;
            }
            Task::none()
        }
        Message::DialogSmtpHostChanged(val) => {
            if let Some(ref mut d) = app.account_dialog {
                d.smtp_host = val;
                d.test_result = None;
            }
            Task::none()
        }
        Message::DialogSmtpPortChanged(val) => {
            if let Some(ref mut d) = app.account_dialog {
                d.smtp_port = val;
                d.test_result = None;
            }
            Task::none()
        }
        Message::DialogToggleAdvanced => {
            if let Some(ref mut d) = app.account_dialog {
                d.show_advanced = !d.show_advanced;
            }
            Task::none()
        }
        Message::DialogTestConnection => {
            if let Some(ref mut d) = app.account_dialog {
                if !d.can_test() {
                    return Task::none();
                }
                d.testing = true;
                d.test_result = None;

                let imap_host = d.imap_host.clone();
                let imap_port = d.imap_port_u16();
                let smtp_host = d.smtp_host.clone();
                let smtp_port = d.smtp_port_u16();
                let username = d.email.clone();
                let password = d.password.clone();

                return Task::perform(
                    async move {
                        // Test IMAP and SMTP concurrently.
                        let (imap_r, smtp_r) = tokio::join!(
                            crate::accounts::setup::test_imap_connection(
                                &imap_host, imap_port, &username, &password,
                            ),
                            crate::accounts::setup::test_smtp_connection(
                                &smtp_host, smtp_port, &username, &password,
                            ),
                        );
                        imap_r.map_err(|e| e.to_string())?;
                        smtp_r.map_err(|e| e.to_string())?;
                        Ok(())
                    },
                    Message::DialogTestResult,
                );
            }
            Task::none()
        }
        Message::DialogTestResult(result) => {
            if let Some(ref mut d) = app.account_dialog {
                d.testing = false;
                d.test_result = Some(result);
            }
            Task::none()
        }
        Message::DialogSubmitAccount => {
            if let Some(dialog) = app.account_dialog.take() {
                if !dialog.can_submit() {
                    app.account_dialog = Some(dialog);
                    return Task::none();
                }
                let name = dialog.display_name.clone();
                let email = dialog.email.clone();
                let password = dialog.password.clone();
                let imap_host = dialog.imap_host.clone();
                let imap_port = dialog.imap_port_u16();
                let smtp_host = dialog.smtp_host.clone();
                let smtp_port = dialog.smtp_port_u16();

                return Task::perform(
                    async move {
                        crate::accounts::setup::create_account(
                            &name, &email, &password, &imap_host, imap_port,
                            &smtp_host, smtp_port,
                        )
                        .await
                        .map_err(|e| e.to_string())
                    },
                    Message::AccountAdded,
                );
            }
            Task::none()
        }

        // ── Attachments ─────────────────────────────────────────────
        Message::DownloadAttachment(_mail_idx, _part_idx) => {
            // TODO: implement full attachment download with raw message cache
            app.push_toast("Attachment download coming soon", ToastLevel::Info);
            Task::none()
        }
        Message::AttachmentDownloaded(result) => {
            match result {
                Ok(path) => {
                    app.push_toast(
                        format!("Saved to {}", path.display()),
                        ToastLevel::Success,
                    );
                }
                Err(e) => {
                    app.push_toast(format!("Download failed: {e}"), ToastLevel::Error);
                }
            }
            Task::none()
        }
        Message::OpenAttachment(path) => {
            #[cfg(target_os = "windows")]
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "", &path.display().to_string()])
                .spawn();
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg(&path).spawn();
            #[cfg(target_os = "linux")]
            let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
            Task::none()
        }

        // ── Confirmation Dialog ─────────────────────────────────────
        Message::ShowConfirm(dialog) => {
            app.confirm_dialog = Some(dialog);
            Task::none()
        }
        Message::ConfirmYes(action) => {
            app.confirm_dialog = None;
            match action {
                crate::ui::confirm_dialog::ConfirmAction::DeleteAccount(idx) => {
                    return update(app, Message::RemoveAccount(idx));
                }
                crate::ui::confirm_dialog::ConfirmAction::EmptyTrash => {
                    return update(app, Message::EmptyTrash);
                }
                crate::ui::confirm_dialog::ConfirmAction::DiscardDraft => {
                    app.composing = None;
                    app.view = View::Mail;
                }
                crate::ui::confirm_dialog::ConfirmAction::DeleteMail(idx) => {
                    return update(app, Message::DeleteMail(idx));
                }
            }
            Task::none()
        }
        Message::ConfirmCancel => {
            app.confirm_dialog = None;
            Task::none()
        }

        // ── Toast Notifications ─────────────────────────────────────
        Message::DismissToast(id) => {
            app.toasts.retain(|t| t.id != id);
            Task::none()
        }
        Message::TickToasts => {
            app.prune_toasts();
            Task::none()
        }

        // ── Keyboard ────────────────────────────────────────────────
        Message::EscapePressed => {
            // Close context menu first if it's open.
            if app.context_menu.is_some() {
                app.context_menu = None;
                return Task::none();
            }
            match app.view {
                View::Compose => {
                    app.composing = None;
                    app.view = View::Mail;
                }
                View::Settings => {
                    app.view = View::Mail;
                }
                View::Mail => {
                    if app.confirm_dialog.is_some() {
                        app.confirm_dialog = None;
                    } else if app.account_dialog.is_some() {
                        app.account_dialog = None;
                    } else {
                        app.selected_mail = None;
                    }
                }
            }
            Task::none()
        }
        Message::KeyboardEvent(_) => Task::none(),

        // ── Settings ────────────────────────────────────────────────
        Message::OpenSettings => {
            app.view = View::Settings;
            Task::none()
        }
        Message::CloseSettings => {
            app.view = View::Mail;
            Task::none()
        }

        // ── Folder Management ───────────────────────────────────────
        Message::CreateFolder(name) => {
            let account = get_selected_account(app);
            if let Some(account) = account {
                let account_id = account.id.clone();
                return Task::perform(
                    async move {
                        let password = fetch_password(account_id).await?;
                        let mut session =
                            crate::mail::imap::connect(&account, &password)
                                .await
                                .map_err(|e| e.to_string())?;
                        crate::mail::folders::create_folder(&mut session, &name)
                            .await
                            .map_err(|e| e.to_string())?;
                        let _ = session.logout().await;
                        Ok(())
                    },
                    Message::FolderOpCompleted,
                );
            }
            Task::none()
        }
        Message::RenameFolder(old_name, new_name) => {
            let account = get_selected_account(app);
            if let Some(account) = account {
                let account_id = account.id.clone();
                return Task::perform(
                    async move {
                        let password = fetch_password(account_id).await?;
                        let mut session =
                            crate::mail::imap::connect(&account, &password)
                                .await
                                .map_err(|e| e.to_string())?;
                        crate::mail::folders::rename_folder(&mut session, &old_name, &new_name)
                            .await
                            .map_err(|e| e.to_string())?;
                        let _ = session.logout().await;
                        Ok(())
                    },
                    Message::FolderOpCompleted,
                );
            }
            Task::none()
        }
        Message::DeleteFolder(name) => {
            let account = get_selected_account(app);
            if let Some(account) = account {
                let account_id = account.id.clone();
                return Task::perform(
                    async move {
                        let password = fetch_password(account_id).await?;
                        let mut session =
                            crate::mail::imap::connect(&account, &password)
                                .await
                                .map_err(|e| e.to_string())?;
                        crate::mail::folders::delete_folder(&mut session, &name)
                            .await
                            .map_err(|e| e.to_string())?;
                        let _ = session.logout().await;
                        Ok(())
                    },
                    Message::FolderOpCompleted,
                );
            }
            Task::none()
        }
        Message::EmptyTrash => {
            let account = get_selected_account(app);
            if let Some(account) = account {
                let account_id = account.id.clone();
                return Task::perform(
                    async move {
                        let password = fetch_password(account_id).await?;
                        let mut session =
                            crate::mail::imap::connect(&account, &password)
                                .await
                                .map_err(|e| e.to_string())?;
                        crate::mail::folders::empty_trash(&mut session, "Trash")
                            .await
                            .map_err(|e| e.to_string())?;
                        let _ = session.logout().await;
                        Ok(())
                    },
                    Message::FolderOpCompleted,
                );
            }
            Task::none()
        }
        Message::FolderOpCompleted(result) => {
            match result {
                Ok(()) => {
                    app.push_toast("Folder operation completed", ToastLevel::Success);
                    // Re-fetch folder list and current folder messages.
                    let refresh_mails = Task::perform(async {}, |_| Message::RefreshFolder);
                    let account = get_selected_account(app);
                    let fetch_folders = if let Some(account) = account {
                        let account_id = account.id.clone();
                        Task::perform(
                            async move {
                                let password = fetch_password(account_id).await?;
                                let mut session =
                                    crate::mail::imap::connect(&account, &password)
                                        .await
                                        .map_err(|e| e.to_string())?;
                                let folders = crate::mail::imap::fetch_folders(&mut session)
                                    .await
                                    .map_err(|e| e.to_string())?;
                                let _ = session.logout().await;
                                Ok(folders)
                            },
                            Message::FoldersFetched,
                        )
                    } else {
                        Task::none()
                    };
                    return Task::batch([refresh_mails, fetch_folders]);
                }
                Err(e) => {
                    app.push_toast(format!("Folder error: {e}"), ToastLevel::Error);
                }
            }
            Task::none()
        }

        // ── Onboarding ──────────────────────────────────────────────
        Message::OnboardingNext => {
            if app.onboarding_step == 1 {
                // Step 1 -> open add-account dialog, then advance
                app.account_dialog = Some(AccountDialogState::default());
            }
            app.onboarding_step = app.onboarding_step.saturating_add(1);
            if app.onboarding_step > 2 {
                app.first_launch = false;
            }
            Task::none()
        }
        Message::OnboardingSkip => {
            app.first_launch = false;
            Task::none()
        }
        Message::OnboardingComplete => {
            app.first_launch = false;
            Task::none()
        }

        // ── Context Menu ───────────────────────────────────────────
        Message::ShowContextMenu(menu) => {
            app.context_menu = Some(menu);
            Task::none()
        }
        Message::CloseContextMenu => {
            app.context_menu = None;
            Task::none()
        }

        // ── IMAP IDLE ──────────────────────────────────────────────
        Message::IdleNewMail => {
            app.push_toast("New mail received", ToastLevel::Info);
            Task::perform(async {}, |_| Message::RefreshFolder)
        }
        Message::IdleError(ref e) => {
            tracing::warn!("IDLE error, falling back to polling: {}", e);
            // On IDLE failure the periodic Tick handler will continue to poll.
            Task::none()
        }
        Message::ToggleIdle => {
            app.idle_enabled = !app.idle_enabled;
            let state_label = if app.idle_enabled { "enabled" } else { "disabled" };
            app.push_toast(
                format!("IDLE push notifications {}", state_label),
                ToastLevel::Info,
            );
            Task::none()
        }

        // ── OAuth2 ──────────────────────────────────────────────────
        Message::StartOAuth(provider) => {
            app.status_message = Some("Starting OAuth2 sign-in...".to_string());
            // Use a placeholder client_id — in production this should come from config.
            let client_id = String::new();
            Task::perform(
                async move {
                    let token_response =
                        crate::accounts::oauth2_flow::start_oauth_flow(provider, client_id)
                            .await
                            .map_err(|e| e.to_string())?;
                    let refresh = token_response
                        .refresh_token
                        .unwrap_or_default();
                    Ok((token_response.access_token, refresh))
                },
                Message::OAuthCompleted,
            )
        }
        Message::OAuthCompleted(result) => {
            app.status_message = None;
            match result {
                Ok((access_token, refresh_token)) => {
                    // Store the refresh token on the currently-selected account
                    // (or the account being set up via the dialog).
                    if let Some(idx) = app.selected_account {
                        if let Some(account) = app.accounts.get_mut(idx) {
                            account.auth_method = crate::state::AuthMethod::OAuth2 {
                                refresh_token: refresh_token.clone(),
                            };
                        }
                    }
                    app.push_toast("OAuth2 sign-in successful", ToastLevel::Success);
                    tracing::info!(
                        "OAuth2 completed, access_token len={}, refresh_token len={}",
                        access_token.len(),
                        refresh_token.len()
                    );
                }
                Err(e) => {
                    app.push_toast(format!("OAuth2 failed: {e}"), ToastLevel::Error);
                }
            }
            Task::none()
        }

        // ── System ──────────────────────────────────────────────────
        Message::Tick => {
            // Periodic refresh: check if enough time has elapsed since last sync.
            let interval = std::time::Duration::from_secs(app.settings.check_interval_secs);
            let should_sync = match app.last_sync {
                Some(last) => last.elapsed() >= interval,
                None => true, // never synced yet
            };

            if should_sync
                && !app.loading
                && app.selected_account.is_some()
                && app.selected_folder.is_some()
            {
                // When IDLE is enabled, use a lightweight poll (SELECT + EXISTS count
                // comparison) instead of a full message fetch. This avoids re-downloading
                // all messages every tick — only triggering a full RefreshFolder when
                // the server reports new arrivals.
                if app.idle_enabled {
                    let account = get_selected_account(app);
                    let folder = app.selected_folder.clone();
                    let known_count = app.last_known_count;

                    if let (Some(account), Some(folder)) = (account, folder) {
                        let account_id = account.id.clone();
                        return Task::perform(
                            async move {
                                let password = fetch_password(account_id).await?;
                                let mut session =
                                    crate::mail::imap::connect(&account, &password)
                                        .await
                                        .map_err(|e| e.to_string())?;
                                let has_new = crate::mail::idle::poll_for_new_mail(
                                    &mut session,
                                    &folder,
                                    known_count,
                                )
                                .await
                                .map_err(|e| e.to_string())?;
                                let _ = session.logout().await;
                                Ok(has_new)
                            },
                            |result: Result<bool, String>| match result {
                                Ok(true) => Message::IdleNewMail,
                                Ok(false) => Message::ClearStatus,
                                Err(e) => Message::IdleError(e),
                            },
                        );
                    }
                }

                return Task::perform(async {}, |_| Message::RefreshFolder);
            }
            Task::none()
        }
        Message::ClearStatus => {
            app.status_message = None;
            Task::none()
        }
        Message::FontLoaded(_) => Task::none(),

        // ── Compose attachments ────────────────────────────────────
        Message::ComposeAttachmentPathChanged(val) => {
            app.compose_attachment_path = val;
            Task::none()
        }
        Message::AddAttachment => Task::none(),
        Message::AttachmentPicked(result) => {
            if let Ok(paths) = result {
                if let Some(ref mut draft) = app.composing {
                    for path in paths {
                        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        let size = std::fs::metadata(&path).map(|m| m.len() as usize).unwrap_or(0);
                        draft.attachments.push(crate::state::ComposeAttachment {
                            path, filename,
                            content_type: "application/octet-stream".to_string(),
                            size,
                        });
                    }
                }
            }
            Task::none()
        }
        Message::RemoveAttachment(idx) => {
            if let Some(ref mut draft) = app.composing {
                if idx < draft.attachments.len() { draft.attachments.remove(idx); }
            }
            Task::none()
        }
        Message::ServerSearch(_) => Task::none(),
        Message::ServerSearchResults(result) => {
            if let Ok(mails) = result { app.mail_entries = mails; refilter(app); }
            Task::none()
        }
        Message::OnboardingNext => { app.onboarding_step += 1; Task::none() }
        Message::OnboardingSkip | Message::OnboardingComplete => {
            app.first_launch = false; app.onboarding_step = 0; Task::none()
        }
        Message::ShowContextMenu(menu) => { app.context_menu = Some(menu); Task::none() }
        Message::CloseContextMenu => { app.context_menu = None; Task::none() }
        Message::IdleNewMail => Task::perform(async {}, |_| Message::RefreshFolder),
        Message::IdleError(e) => {
            app.push_toast(format!("IDLE error: {e}"), ToastLevel::Error);
            app.idle_enabled = false; Task::none()
        }
        Message::ToggleIdle => { app.idle_enabled = !app.idle_enabled; Task::none() }
        Message::StartOAuth(_) => { app.push_toast("OAuth2 coming soon", ToastLevel::Info); Task::none() }
        Message::OAuthCompleted(result) => {
            match result {
                Ok(_) => app.push_toast("OAuth2 successful", ToastLevel::Success),
                Err(e) => app.push_toast(format!("OAuth2 error: {e}"), ToastLevel::Error),
            }
            Task::none()
        }

    }
}
