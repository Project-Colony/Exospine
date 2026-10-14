//! Desktop notification support using notify-rust.

use crate::app_state::MailEntry;
use crate::i18n::tr;

fn sender(mail: &MailEntry) -> &str {
    if mail.from.is_empty() {
        tr("Unknown sender", "Expéditeur inconnu")
    } else {
        &mail.from
    }
}

/// Send a desktop notification for a single new mail.
pub fn notify_new_mail(mail: &MailEntry) {
    let subject = if mail.subject.is_empty() {
        tr("(no subject)", "(sans objet)")
    } else {
        &mail.subject
    };
    let body = tr("From: {from}\nSubject: {subject}", "De : {from}\nObjet : {subject}")
        .replace("{from}", sender(mail))
        .replace("{subject}", subject);

    if let Err(e) = notify_rust::Notification::new()
        .summary(tr("New email", "Nouveau message"))
        .body(&body)
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

    let body = tr(
        "{count} new emails received.\nLatest from: {from}",
        "{count} nouveaux messages reçus.\nLe dernier vient de : {from}",
    )
    .replace("{count}", &mails.len().to_string())
    .replace("{from}", sender(&mails[0]));

    if let Err(e) = notify_rust::Notification::new()
        .summary(tr("New email", "Nouveau message"))
        .body(&body)
        .appname("Exospine")
        .show()
    {
        tracing::warn!("Failed to show desktop notification: {}", e);
    }
}
