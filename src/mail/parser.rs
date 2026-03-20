use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use mail_parser::MessageParser;

use crate::state::MailEntry;

/// Parse a raw email (RFC 5322) into a `MailEntry`.
pub fn parse_email(raw: &[u8], account_id: &str, folder: &str) -> Result<MailEntry> {
    let message =
        MessageParser::new().parse(raw).context("Failed to parse email")?;

    let from = message
        .from()
        .and_then(|addrs| addrs.first())
        .map(|addr| {
            let email = addr.address.as_deref().unwrap_or("");
            match &addr.name {
                Some(name) => format!("{} <{}>", name, email),
                None => email.to_string(),
            }
        })
        .unwrap_or_else(|| "(unknown)".to_string());

    let to: Vec<String> = message
        .to()
        .map(|addrs| {
            addrs
                .iter()
                .filter_map(|a| a.address.as_ref().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let subject = message
        .subject()
        .unwrap_or("(no subject)")
        .to_string();

    let date: DateTime<Utc> = message
        .date()
        .map(|d| {
            DateTime::from_timestamp(d.to_timestamp(), 0)
                .unwrap_or_else(Utc::now)
        })
        .unwrap_or_else(Utc::now);

    let body_text = message
        .body_text(0)
        .map(|t| t.to_string())
        .unwrap_or_default();

    let body_html = message.body_html(0).map(|h| h.to_string());

    // Generate preview: use plain text, or convert HTML
    let preview_source = if !body_text.is_empty() {
        body_text.clone()
    } else if let Some(ref html) = body_html {
        html_to_text(html)
    } else {
        String::new()
    };
    let preview: String = preview_source.chars().take(120).collect();

    let has_attachments = message.attachment_count() > 0;

    Ok(MailEntry {
        id: String::new(), // UID set by caller
        from,
        to,
        subject,
        date,
        preview,
        body_text: if body_text.is_empty() {
            body_html
                .as_ref()
                .map(|h| html_to_text(h))
                .unwrap_or_default()
        } else {
            body_text
        },
        body_html,
        is_read: false,
        is_starred: false,
        has_attachments,
        folder: folder.to_string(),
        account_id: account_id.to_string(),
    })
}

/// Convert HTML to readable plain text.
pub fn html_to_text(html: &str) -> String {
    html2text::from_read(html.as_bytes(), 80)
        .unwrap_or_else(|_| html.to_string())
}
