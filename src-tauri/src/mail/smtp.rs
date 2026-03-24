//! SMTP mail sending for Exospine.

use anyhow::{Context, Result};
use lettre::message::header::ContentType;
use lettre::message::{Attachment, Mailbox, MessageBuilder, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::app_state::{Account, ComposeDraft};

/// Priority level for outgoing mail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Priority {
    High,
    Normal,
    Low,
}

impl Priority {
    const fn header_value(self) -> &'static str {
        match self {
            Self::High => "1 (Highest)",
            Self::Normal => "3 (Normal)",
            Self::Low => "5 (Lowest)",
        }
    }
}

/// Send an email using SMTP with TLS.
pub async fn send_mail(
    account: &Account,
    password: &str,
    draft: &ComposeDraft,
    reply_to_message_id: Option<&str>,
) -> Result<()> {
    let priority = match draft.importance.as_str() {
        "high" => Priority::High,
        "low" => Priority::Low,
        _ => Priority::Normal,
    };
    send_mail_with_priority(account, password, draft, reply_to_message_id, priority).await
}

/// Send an email with an explicit priority level.
pub async fn send_mail_with_priority(
    account: &Account,
    password: &str,
    draft: &ComposeDraft,
    reply_to_message_id: Option<&str>,
    priority: Priority,
) -> Result<()> {
    let mut builder = Message::builder().from(
        account
            .email
            .parse::<Mailbox>()
            .context("Invalid sender address")?,
    );

    for addr in parse_addresses(&draft.to) {
        let mbox: Mailbox = addr
            .parse()
            .with_context(|| format!("Invalid To address: {addr}"))?;
        builder = builder.to(mbox);
    }

    for addr in parse_addresses(&draft.cc) {
        let mbox: Mailbox = addr
            .parse()
            .with_context(|| format!("Invalid CC address: {addr}"))?;
        builder = builder.cc(mbox);
    }

    for addr in parse_addresses(&draft.bcc) {
        let mbox: Mailbox = addr
            .parse()
            .with_context(|| format!("Invalid BCC address: {addr}"))?;
        builder = builder.bcc(mbox);
    }

    builder = builder.subject(&draft.subject);
    builder = add_threading_headers(builder, reply_to_message_id);
    builder = add_priority_header(builder, priority);

    // Build the body part: multipart/alternative if HTML is provided, otherwise plain text.
    let has_html = draft
        .body_html
        .as_ref()
        .map_or(false, |h| !h.trim().is_empty());

    let email = if !has_html && draft.attachments.is_empty() {
        // Simple plain-text email (no HTML, no attachments).
        builder
            .header(ContentType::TEXT_PLAIN)
            .body(draft.body.clone())
            .context("Failed to build email message")?
    } else {
        // Build the text/html + text/plain alternative part.
        let body_part = if has_html {
            let html_content = draft.body_html.as_deref().unwrap_or("");
            let wrapped_html = format!(
                "<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body>{}</body></html>",
                html_content
            );
            let text_part = SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(draft.body.clone());
            let html_part = SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(wrapped_html);
            MultiPart::alternative()
                .singlepart(text_part)
                .singlepart(html_part)
        } else {
            // Plain text only, but we still need MultiPart because of attachments.
            MultiPart::alternative().singlepart(
                SinglePart::builder()
                    .header(ContentType::TEXT_PLAIN)
                    .body(draft.body.clone()),
            )
        };

        if draft.attachments.is_empty() {
            builder
                .multipart(body_part)
                .context("Failed to build multipart email message")?
        } else {
            let mut multipart = MultiPart::mixed().multipart(body_part);

            for att in &draft.attachments {
                let file_bytes = std::fs::read(&att.path)
                    .with_context(|| format!("Failed to read attachment: {}", att.path))?;
                let content_type: ContentType = att
                    .content_type
                    .parse()
                    .unwrap_or_else(|_| {
                        "application/octet-stream"
                            .parse()
                            .expect("valid MIME constant")
                    });
                let attachment =
                    Attachment::new(att.filename.clone()).body(file_bytes, content_type);
                multipart = multipart.singlepart(attachment);
            }

            builder
                .multipart(multipart)
                .context("Failed to build multipart email message")?
        }
    };

    let creds = Credentials::new(account.username.clone(), password.to_string());

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&account.smtp_host)
        .context("Failed to create SMTP transport")?
        .credentials(creds)
        .port(account.smtp_port)
        .build();

    mailer
        .send(email)
        .await
        .context("Failed to send email")?;

    tracing::info!(
        to = %draft.to,
        cc = %draft.cc,
        bcc = %draft.bcc,
        attachments = draft.attachments.len(),
        "Email sent successfully",
    );
    Ok(())
}

fn parse_addresses(field: &str) -> impl Iterator<Item = &str> {
    field
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn add_threading_headers(
    builder: MessageBuilder,
    reply_to_message_id: Option<&str>,
) -> MessageBuilder {
    match reply_to_message_id {
        Some(msg_id) if !msg_id.is_empty() => {
            let formatted = if msg_id.starts_with('<') {
                msg_id.to_owned()
            } else {
                format!("<{msg_id}>")
            };
            builder
                .header(InReplyTo(formatted.clone()))
                .header(References(formatted))
        }
        _ => builder,
    }
}

fn add_priority_header(builder: MessageBuilder, priority: Priority) -> MessageBuilder {
    if priority == Priority::Normal {
        return builder;
    }
    builder.header(XPriority(priority.header_value()))
}

// ── Custom header types for lettre ─────────────────────────────────────

#[derive(Debug, Clone)]
struct InReplyTo(String);

impl lettre::message::header::Header for InReplyTo {
    fn name() -> lettre::message::header::HeaderName {
        lettre::message::header::HeaderName::new_from_ascii_str("In-Reply-To")
    }

    fn parse(_: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self(String::new()))
    }

    fn display(&self) -> lettre::message::header::HeaderValue {
        lettre::message::header::HeaderValue::new(Self::name(), self.0.clone())
    }
}

#[derive(Debug, Clone)]
struct References(String);

impl lettre::message::header::Header for References {
    fn name() -> lettre::message::header::HeaderName {
        lettre::message::header::HeaderName::new_from_ascii_str("References")
    }

    fn parse(_: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self(String::new()))
    }

    fn display(&self) -> lettre::message::header::HeaderValue {
        lettre::message::header::HeaderValue::new(Self::name(), self.0.clone())
    }
}

#[derive(Debug, Clone)]
struct XPriority(&'static str);

impl lettre::message::header::Header for XPriority {
    fn name() -> lettre::message::header::HeaderName {
        lettre::message::header::HeaderName::new_from_ascii_str("X-Priority")
    }

    fn parse(_: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self("3 (Normal)"))
    }

    fn display(&self) -> lettre::message::header::HeaderValue {
        lettre::message::header::HeaderValue::new(Self::name(), self.0.to_owned())
    }
}

/// Guess the MIME content type for a file based on its extension.
#[allow(dead_code)]
pub fn guess_content_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "zip" => "application/zip",
        "gz" | "gzip" => "application/gzip",
        "tar" => "application/x-tar",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "csv" => "text/csv",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "wav" => "audio/wav",
        "webp" => "image/webp",
        "eml" => "message/rfc822",
        _ => "application/octet-stream",
    }
}
