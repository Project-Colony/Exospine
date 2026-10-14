//! Desktop notification support using notify-rust.

use crate::app_state::MailEntry;

/// Send a desktop notification for a single new mail.
pub fn notify_new_mail(mail: &MailEntry) {
    let from = if mail.from.is_empty() {
        "Unknown sender"
    } else {
        &mail.from
    };
    let subject = if mail.subject.is_empty() {
        "(no subject)"
    } else {
        &mail.subject
    };

    if let Err(e) = notify_rust::Notification::new()
        .summary("New email")
        .body(&format!("From: {}\nSubject: {}", from, subject))
        .appname("Exospine")
        .show()
    {
        tracing::warn!("Failed to show desktop notification: {}", e);
    }
}

/// Send a summary notification for multiple new mails.
pub fn notify_new_mails_batch(mails: &[MailEntry]) {
    if mails.is_empty() {
        return;
    }
    if mails.len() == 1 {
        notify_new_mail(&mails[0]);
        return;
    }

    let body = format!(
        "{} new emails received.\nLatest from: {}",
        mails.len(),
        if mails[0].from.is_empty() {
            "Unknown sender"
        } else {
            &mails[0].from
        }
    );

    if let Err(e) = notify_rust::Notification::new()
        .summary("New email")
        .body(&body)
        .appname("Exospine")
        .show()
    {
        tracing::warn!("Failed to show desktop notification: {}", e);
    }
}
