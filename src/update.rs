use crate::message::Message;
use crate::state::{App, ComposeDraft, View};
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
            Task::none()
        }

        // ── Mail Actions ────────────────────────────────────────────
        Message::MarkAsRead(idx) => {
            if let Some(entry) = app.mail_entries.get_mut(idx) {
                entry.is_read = true;
            }
            Task::none()
        }
        Message::MarkAsUnread(idx) => {
            if let Some(entry) = app.mail_entries.get_mut(idx) {
                entry.is_read = false;
            }
            Task::none()
        }
        Message::ToggleStar(idx) => {
            if let Some(entry) = app.mail_entries.get_mut(idx) {
                entry.is_starred = !entry.is_starred;
            }
            Task::none()
        }
        Message::DeleteMail(idx) => {
            if idx < app.mail_entries.len() {
                app.mail_entries.remove(idx);
                app.selected_mail = None;
            }
            Task::none()
        }
        Message::ArchiveMail(idx) => {
            if idx < app.mail_entries.len() {
                app.mail_entries.remove(idx);
                app.selected_mail = None;
                app.status_message = Some("Message archived".to_string());
            }
            Task::none()
        }
        Message::MoveMail(idx, target_folder) => {
            if let Some(entry) = app.mail_entries.get_mut(idx) {
                entry.folder = target_folder;
            }
            Task::none()
        }
        Message::RefreshFolder => {
            app.loading = true;
            app.status_message = Some("Refreshing...".to_string());
            // TODO: spawn async IMAP fetch
            // For now, just clear loading state
            app.loading = false;
            app.status_message = None;
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
            if let Some(_draft) = app.composing.take() {
                app.view = View::Mail;
                app.status_message = Some("Sending...".to_string());
                // TODO: spawn async SMTP send
                app.status_message = Some("Message sent".to_string());
            }
            Task::none()
        }
        Message::SaveDraft => {
            if app.composing.is_some() {
                app.status_message = Some("Draft saved".to_string());
                // TODO: save to drafts folder
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
    }
}
