use crate::message::Message;
use crate::state::{App, ComposeDraft, View};
use crate::ui::toast::ToastLevel;
use iced::Task;

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
                return Task::perform(async {}, |_| Message::RefreshFolder);
            }
            Task::none()
        }
        Message::SelectFolder(folder) => {
            app.selected_folder = Some(folder);
            app.selected_mail = None;
            Task::perform(async {}, |_| Message::RefreshFolder)
        }
        Message::SelectMail(idx) => {
            app.selected_mail = Some(idx);
            if let Some(entry) = app.mail_entries.get_mut(idx) {
                entry.is_read = true;
            }
            Task::none()
        }
        Message::SwitchView(view) => {
            app.view = view;
            Task::none()
        }

        // ── Search ──────────────────────────────────────────────────
        Message::SearchChanged(query) => {
            app.search_query = query;
            let mut indices = crate::search::filter_entries(&app.mail_entries, &app.search_query);
            // Further filter by quick filter using HashSet for O(1) lookup
            if app.quick_filter != crate::search::QuickFilter::All {
                let qf: std::collections::HashSet<usize> =
                    crate::search::apply_quick_filter(&app.mail_entries, &app.quick_filter)
                        .into_iter()
                        .collect();
                indices.retain(|i| qf.contains(i));
            }
            app.filtered_indices = indices;
            Task::none()
        }

        // ── Mail Actions ────────────────────────────────────────────
        Message::MarkAsRead(idx) => {
            let task = sync_flag_task(app, idx, FlagOp::MarkRead);
            if let Some(entry) = app.mail_entries.get_mut(idx) {
                entry.is_read = true;
            }
            task
        }
        Message::MarkAsUnread(idx) => {
            let task = sync_flag_task(app, idx, FlagOp::MarkUnread);
            if let Some(entry) = app.mail_entries.get_mut(idx) {
                entry.is_read = false;
            }
            task
        }
        Message::ToggleStar(idx) => {
            let starred = app.mail_entries.get(idx).map_or(false, |e| e.is_starred);
            let op = if starred { FlagOp::RemoveStar } else { FlagOp::AddStar };
            let task = sync_flag_task(app, idx, op);
            if let Some(entry) = app.mail_entries.get_mut(idx) {
                entry.is_starred = !entry.is_starred;
            }
            task
        }
        Message::DeleteMail(idx) => {
            let task = sync_flag_task(app, idx, FlagOp::Delete);
            if idx < app.mail_entries.len() {
                app.mail_entries.remove(idx);
                app.selected_mail = None;
            }
            task
        }
        Message::ArchiveMail(idx) => {
            let task = sync_flag_task(app, idx, FlagOp::Archive);
            if idx < app.mail_entries.len() {
                app.mail_entries.remove(idx);
                app.selected_mail = None;
                app.status_message = Some("Message archived".to_string());
            }
            task
        }
        Message::MoveMail(idx, ref target_folder) => {
            let target = target_folder.clone();
            let task = sync_flag_task(app, idx, FlagOp::Move(target.clone()));
            if let Some(entry) = app.mail_entries.get_mut(idx) {
                entry.folder = target;
            }
            task
        }
        Message::RefreshFolder => {
            app.loading = true;
            app.status_message = Some("Refreshing...".to_string());

            let account = app.selected_account.and_then(|i| app.accounts.get(i)).cloned();
            let folder = app.selected_folder.clone();

            if let (Some(account), Some(folder)) = (account, folder) {
                Task::perform(
                    async move {
                        let password = crate::accounts::keyring_store::get_password(&account.id)
                            .map_err(|e| e.to_string())?;
                        let mut session = crate::mail::imap::connect(&account, &password)
                            .await
                            .map_err(|e| e.to_string())?;
                        let mails = crate::mail::imap::fetch_messages(&mut session, &folder, 50)
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
                        body: format!("\n\n--- Original Message ---\n{}", mail.body_text),
                        reply_to: Some(mail.id.clone()),
                        account_id,
                    });
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
                        body: format!("\n\n--- Original Message ---\n{}", mail.body_text),
                        reply_to: Some(mail.id.clone()),
                        account_id,
                    });
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

                let account = app.accounts.iter().find(|a| a.id == draft.account_id).cloned();

                if let Some(account) = account {
                    let reply_to = draft.reply_to.clone();
                    Task::perform(
                        async move {
                            let password = crate::accounts::keyring_store::get_password(&account.id)
                                .map_err(|e| e.to_string())?;
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
                    )
                } else {
                    app.status_message = Some("No account found".to_string());
                    Task::none()
                }
            } else {
                Task::none()
            }
        }
        Message::SaveDraft => {
            if let Some(ref draft) = app.composing {
                app.status_message = Some("Saving draft...".to_string());
                let account = app.accounts.iter().find(|a| a.id == draft.account_id).cloned();
                let draft_rfc = format!(
                    "From: {from}\r\nTo: {to}\r\nSubject: {subject}\r\n\r\n{body}",
                    from = account.as_ref().map_or("", |a| a.email.as_str()),
                    to = draft.to,
                    subject = draft.subject,
                    body = draft.body,
                );

                if let Some(account) = account {
                    return Task::perform(
                        async move {
                            let password = crate::accounts::keyring_store::get_password(&account.id)
                                .map_err(|e| e.to_string())?;
                            let mut session = crate::mail::imap::connect(&account, &password)
                                .await
                                .map_err(|e| e.to_string())?;
                            crate::mail::imap::append_message(&mut session, "Drafts", draft_rfc.as_bytes())
                                .await
                                .map_err(|e| e.to_string())?;
                            let _ = session.logout().await;
                            Ok(())
                        },
                        |r: Result<(), String>| match r {
                            Ok(()) => Message::ClearStatus,
                            Err(e) => Message::FlagSynced(Err(e)),
                        },
                    );
                }
                app.status_message = Some("Draft saved locally".to_string());
            }
            Task::none()
        }
        Message::DiscardDraft => {
            app.composing = None;
            app.view = View::Mail;
            Task::none()
        }

        // ── Async Results ───────────────────────────────────────────
        Message::MailsFetched(result) => {
            app.loading = false;
            match result {
                Ok(mails) => {
                    app.mail_entries = mails;
                    app.status_message = None;
                }
                Err(e) => {
                    app.status_message = Some(format!("Error: {}", e));
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
                    app.status_message = Some("Message sent successfully".to_string());
                }
                Err(e) => {
                    app.status_message = Some(format!("Send failed: {}", e));
                }
            }
            Task::none()
        }

        // ── Account Management ──────────────────────────────────────
        Message::AddAccount => {
            // TODO: open account setup dialog
            app.status_message = Some("Account setup coming soon".to_string());
            Task::none()
        }
        Message::RemoveAccount(idx) => {
            if idx < app.accounts.len() {
                app.accounts.remove(idx);
                app.selected_account = None;
                app.selected_folder = None;
                app.mail_entries.clear();
            }
            Task::none()
        }
        Message::AccountAdded(result) => {
            match result {
                Ok(account) => {
                    app.accounts.push(account);
                    app.status_message = Some("Account added".to_string());
                }
                Err(e) => {
                    app.status_message = Some(format!("Error: {}", e));
                }
            }
            Task::none()
        }

        // ── Settings ────────────────────────────────────────────────
        Message::OpenSettings => {
            app.view = View::Settings;
            Task::none()
        }
        Message::CloseSettings => {
            app.view = View::Mail;
            Task::none()
        }

        // ── System ──────────────────────────────────────────────────
        Message::Tick => {
            // TODO: periodic mail check
            Task::none()
        }
        Message::ClearStatus => {
            app.status_message = None;
            Task::none()
        }
        Message::FontLoaded(_) => Task::none(),

        // ── Keyboard shortcuts ────────────────────────────────────
        Message::DeleteSelected => {
            if let Some(idx) = app.selected_mail {
                if idx < app.mail_entries.len() {
                    app.mail_entries.remove(idx);
                    // Keep selection in range
                    if app.mail_entries.is_empty() {
                        app.selected_mail = None;
                    } else if idx >= app.mail_entries.len() {
                        app.selected_mail = Some(app.mail_entries.len() - 1);
                    }
                }
            }
            Task::none()
        }
        Message::FocusSearch => {
            // TODO: focus the search text_input widget via Id
            Task::none()
        }
        Message::EscapePressed => {
            match app.view {
                View::Compose => {
                    app.composing = None;
                    app.view = View::Mail;
                }
                View::Settings => {
                    app.view = View::Mail;
                }
                View::Mail => {
                    app.selected_mail = None;
                }
            }
            Task::none()
        }
        Message::SelectNextMail => {
            let len = app.mail_entries.len();
            if len > 0 {
                let next = match app.selected_mail {
                    Some(i) if i + 1 < len => i + 1,
                    Some(i) => i, // already at end
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
                    Some(i) => i, // already at start
                    None => 0,
                };
                app.selected_mail = Some(prev);
            }
            Task::none()
        }

        // ── Search & Filter ─────────────────────────────────────────
        Message::SetQuickFilter(filter) => {
            app.quick_filter = filter;
            app.filtered_indices = crate::search::apply_quick_filter(&app.mail_entries, &app.quick_filter);
            Task::none()
        }
        Message::FlagSynced(result) => {
            if let Err(e) = result {
                app.push_toast(format!("Sync error: {e}"), ToastLevel::Error);
            }
            Task::none()
        }

        // ── Account dialog ──────────────────────────────────────────
        Message::CancelAddAccount => {
            app.account_dialog = None;
            Task::none()
        }
        Message::DialogEmailChanged(val) => {
            if let Some(ref mut d) = app.account_dialog {
                d.email = val;
                d.update_provider();
            }
            Task::none()
        }
        Message::DialogNameChanged(val) => {
            if let Some(ref mut d) = app.account_dialog { d.display_name = val; }
            Task::none()
        }
        Message::DialogPasswordChanged(val) => {
            if let Some(ref mut d) = app.account_dialog { d.password = val; }
            Task::none()
        }
        Message::DialogImapHostChanged(val) => {
            if let Some(ref mut d) = app.account_dialog { d.imap_host = val; d.test_result = None; }
            Task::none()
        }
        Message::DialogImapPortChanged(val) => {
            if let Some(ref mut d) = app.account_dialog { d.imap_port = val; d.test_result = None; }
            Task::none()
        }
        Message::DialogSmtpHostChanged(val) => {
            if let Some(ref mut d) = app.account_dialog { d.smtp_host = val; d.test_result = None; }
            Task::none()
        }
        Message::DialogSmtpPortChanged(val) => {
            if let Some(ref mut d) = app.account_dialog { d.smtp_port = val; d.test_result = None; }
            Task::none()
        }
        Message::DialogToggleAdvanced => {
            if let Some(ref mut d) = app.account_dialog { d.show_advanced = !d.show_advanced; }
            Task::none()
        }
        Message::DialogTestConnection => {
            if let Some(ref mut d) = app.account_dialog {
                if !d.can_test() { return Task::none(); }
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
                        crate::accounts::setup::test_imap_connection(&imap_host, imap_port, &username, &password)
                            .await.map_err(|e| e.to_string())?;
                        crate::accounts::setup::test_smtp_connection(&smtp_host, smtp_port, &username, &password)
                            .await.map_err(|e| e.to_string())?;
                        Ok(())
                    },
                    Message::DialogTestResult,
                );
            }
            Task::none()
        }
        Message::DialogTestResult(result) => {
            if let Some(ref mut d) = app.account_dialog { d.testing = false; d.test_result = Some(result); }
            Task::none()
        }
        Message::DialogSubmitAccount => {
            if let Some(dialog) = app.account_dialog.take() {
                if !dialog.can_submit() { app.account_dialog = Some(dialog); return Task::none(); }
                let name = dialog.display_name.clone();
                let email = dialog.email.clone();
                let password = dialog.password.clone();
                let imap_host = dialog.imap_host.clone();
                let imap_port = dialog.imap_port_u16();
                let smtp_host = dialog.smtp_host.clone();
                let smtp_port = dialog.smtp_port_u16();
                return Task::perform(
                    async move {
                        crate::accounts::setup::create_account(&name, &email, &password, &imap_host, imap_port, &smtp_host, smtp_port)
                            .await.map_err(|e| e.to_string())
                    },
                    Message::AccountAdded,
                );
            }
            Task::none()
        }

        // ── Attachments ─────────────────────────────────────────────
        Message::DownloadAttachment(_mail_idx, _part_idx) => {
            app.push_toast("Attachment download coming soon", ToastLevel::Info);
            Task::none()
        }
        Message::AttachmentDownloaded(result) => {
            match result {
                Ok(path) => app.push_toast(format!("Saved to {}", path.display()), ToastLevel::Success),
                Err(e) => app.push_toast(format!("Download failed: {e}"), ToastLevel::Error),
            }
            Task::none()
        }
        Message::OpenAttachment(path) => {
            #[cfg(target_os = "windows")]
            let _ = std::process::Command::new("cmd").args(["/C", "start", "", &path.display().to_string()]).spawn();
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg(&path).spawn();
            #[cfg(target_os = "linux")]
            let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
            Task::none()
        }

        // ── Confirmation dialog ─────────────────────────────────────
        Message::ShowConfirm(dialog) => { app.confirm_dialog = Some(dialog); Task::none() }
        Message::ConfirmYes(action) => {
            app.confirm_dialog = None;
            match action {
                crate::ui::confirm_dialog::ConfirmAction::DeleteAccount(idx) => {
                    if idx < app.accounts.len() { app.accounts.remove(idx); app.selected_account = None; app.mail_entries.clear(); }
                }
                crate::ui::confirm_dialog::ConfirmAction::EmptyTrash => { app.push_toast("Trash emptied", ToastLevel::Success); }
                crate::ui::confirm_dialog::ConfirmAction::DiscardDraft => { app.composing = None; app.view = View::Mail; }
                crate::ui::confirm_dialog::ConfirmAction::DeleteMail(idx) => {
                    if idx < app.mail_entries.len() { app.mail_entries.remove(idx); app.selected_mail = None; }
                }
            }
            Task::none()
        }
        Message::ConfirmCancel => { app.confirm_dialog = None; Task::none() }

        // ── Toasts ──────────────────────────────────────────────────
        Message::DismissToast(id) => { app.toasts.retain(|t| t.id != id); Task::none() }
        Message::TickToasts => { app.prune_toasts(); Task::none() }

        // ── Keyboard ────────────────────────────────────────────────
        Message::KeyboardEvent(_) => Task::none(),

        // ── Folder management ───────────────────────────────────────
        Message::CreateFolder(_) | Message::RenameFolder(_, _) | Message::DeleteFolder(_) | Message::EmptyTrash => {
            app.push_toast("Folder ops coming soon", ToastLevel::Info);
            Task::none()
        }
        Message::FolderOpCompleted(result) => {
            match result {
                Ok(()) => app.push_toast("Done", ToastLevel::Success),
                Err(e) => app.push_toast(format!("Error: {e}"), ToastLevel::Error),
            }
            Task::none()
        }
    }
}

// ── IMAP flag sync helpers ──────────────────────────────────────────────

/// The type of IMAP flag operation to perform in the background.
enum FlagOp {
    MarkRead,
    MarkUnread,
    AddStar,
    RemoveStar,
    Delete,
    Archive,
    Move(String),
}

/// Spawn an async task to sync a flag change to the IMAP server.
/// Returns `Task::none()` if the account or mail entry is missing.
fn sync_flag_task(app: &App, mail_idx: usize, op: FlagOp) -> Task<Message> {
    let entry = match app.mail_entries.get(mail_idx) {
        Some(e) => e,
        None => return Task::none(),
    };
    let uid = entry.id.clone();
    let folder = entry.folder.clone();
    let account = match app.selected_account.and_then(|i| app.accounts.get(i)) {
        Some(a) => a.clone(),
        None => return Task::none(),
    };

    Task::perform(
        async move {
            let password = crate::accounts::keyring_store::get_password(&account.id)
                .map_err(|e| e.to_string())?;
            let mut session = crate::mail::imap::connect(&account, &password)
                .await
                .map_err(|e| e.to_string())?;

            // Select the folder so UID commands work
            session.select(&folder).await.map_err(|e| format!("Select failed: {e}"))?;

            match op {
                FlagOp::MarkRead => {
                    crate::mail::imap::mark_as_read(&mut session, &uid)
                        .await.map_err(|e| e.to_string())?;
                }
                FlagOp::MarkUnread => {
                    crate::mail::imap::mark_as_unread(&mut session, &uid)
                        .await.map_err(|e| e.to_string())?;
                }
                FlagOp::AddStar => {
                    crate::mail::imap::set_flagged(&mut session, &uid)
                        .await.map_err(|e| e.to_string())?;
                }
                FlagOp::RemoveStar => {
                    crate::mail::imap::remove_flagged(&mut session, &uid)
                        .await.map_err(|e| e.to_string())?;
                }
                FlagOp::Delete => {
                    crate::mail::imap::delete_message(&mut session, &uid)
                        .await.map_err(|e| e.to_string())?;
                }
                FlagOp::Archive => {
                    // Archive = move to "Archive" folder
                    crate::mail::imap::move_message(&mut session, &uid, "Archive")
                        .await.map_err(|e| e.to_string())?;
                }
                FlagOp::Move(target) => {
                    crate::mail::imap::move_message(&mut session, &uid, &target)
                        .await.map_err(|e| e.to_string())?;
                }
            }

            let _ = session.logout().await;
            Ok(())
        },
        Message::FlagSynced,
    )
}
