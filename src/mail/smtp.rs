//! SMTP mail sending for Exospine.
//!
//! Builds a MIME message from a [`ComposeDraft`] and sends it via the
//! account's configured SMTP server. Supports:
//!
//! - Multiple comma-separated To / CC / BCC recipients
//! - `In-Reply-To` and `References` headers for threading
//! - `X-Priority` header

use std::path::Path;

use anyhow::{Context, Result};
use lettre::message::header::ContentType;
use lettre::message::{Attachment, Body, Mailbox, MessageBuilder, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::state::{Account, ComposeDraft};

/// Priority level for outgoing mail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    High,
    Normal,
    Low,
}

impl Priority {
    /// X-Priority header value (1 = high, 3 = normal, 5 = low).
    const fn header_value(self) -> &'static str {
        match self {
            Self::High => "1 (Highest)",
            Self::Normal => "3 (Normal)",
            Self::Low => "5 (Lowest)",
        }
    }
}

/// Send an email using SMTP with TLS.
///
/// * `account` – sender account with SMTP connection details.
/// * `password` – SMTP password (retrieved from keyring).
/// * `draft` – the composed message with To / CC / BCC / subject / body.
/// * `reply_to_message_id` – if this is a reply, the `Message-Id` of the
///   original message. Sets `In-Reply-To` and `References` headers.
pub async fn send_mail(
    account: &Account,
    password: &str,
    draft: &ComposeDraft,
    reply_to_message_id: Option<&str>,
) -> Result<()> {
    send_mail_with_priority(account, password, draft, reply_to_message_id, Priority::Normal).await
}

/// Send an email with an explicit priority level.
pub async fn send_mail_with_priority(
    account: &Account,
    password: &str,
    draft: &ComposeDraft,
    reply_to_message_id: Option<&str>,
    priority: Priority,
) -> Result<()> {
    let mut builder = Message::builder()
        .from(
            account
                .email
                .parse::<Mailbox>()
                .context("Invalid sender address")?,
        );

    // ── To recipients ──────────────────────────────────────────────
    for addr in parse_addresses(&draft.to) {
        let mbox: Mailbox = addr
            .parse()
            .with_context(|| format!("Invalid To address: {addr}"))?;
        builder = builder.to(mbox);
    }

    // ── CC recipients ──────────────────────────────────────────────
    for addr in parse_addresses(&draft.cc) {
        let mbox: Mailbox = addr
            .parse()
            .with_context(|| format!("Invalid CC address: {addr}"))?;
        builder = builder.cc(mbox);
    }

    // ── BCC recipients ─────────────────────────────────────────────
    for addr in parse_addresses(&draft.bcc) {
        let mbox: Mailbox = addr
            .parse()
            .with_context(|| format!("Invalid BCC address: {addr}"))?;
        builder = builder.bcc(mbox);
    }

    // ── Subject ────────────────────────────────────────────────────
    builder = builder.subject(&draft.subject);

    // ── Threading headers ──────────────────────────────────────────
    builder = add_threading_headers(builder, reply_to_message_id);

    // ── Priority header ────────────────────────────────────────────
    builder = add_priority_header(builder, priority);

    // ── Build and send ─────────────────────────────────────────────
    let email = if draft.attachments.is_empty() {
        // Plain text message (no attachments).
        builder
            .header(ContentType::TEXT_PLAIN)
            .body(draft.body.clone())
            .context("Failed to build email message")?
    } else {
        // Multipart/mixed message with attachments.
        let text_part = SinglePart::builder()
            .header(ContentType::TEXT_PLAIN)
            .body(draft.body.clone());

        let mut multipart = MultiPart::mixed().singlepart(text_part);

        for att in &draft.attachments {
            let file_bytes = std::fs::read(&att.path)
                .with_context(|| format!("Failed to read attachment: {}", att.path.display()))?;
            let content_type: ContentType = att
                .content_type
                .parse()
                .unwrap_or(ContentType::parse("application/octet-stream").unwrap());
            let attachment = Attachment::new(att.filename.clone())
                .body(file_bytes, content_type);
            multipart = multipart.singlepart(attachment);
        }

        builder
            .multipart(multipart)
            .context("Failed to build multipart email message")?
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

// ── Helpers ────────────────────────────────────────────────────────────

/// Parse a comma-separated address string into individual trimmed
/// addresses, skipping empty entries. Returns an iterator to avoid
/// allocating a `Vec` in the common single-recipient case.
fn parse_addresses(field: &str) -> impl Iterator<Item = &str> {
    field
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Add `In-Reply-To` and `References` headers when replying.
fn add_threading_headers(
    builder: MessageBuilder,
    reply_to_message_id: Option<&str>,
) -> MessageBuilder {
    match reply_to_message_id {
        Some(msg_id) if !msg_id.is_empty() => {
            // Wrap in angle brackets if not already present.
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

/// Add `X-Priority` header (only when non-normal).
fn add_priority_header(builder: MessageBuilder, priority: Priority) -> MessageBuilder {
    if priority == Priority::Normal {
        return builder;
    }
    builder.header(XPriority(priority.header_value()))
}

// ── Custom header types for lettre ─────────────────────────────────────

/// `In-Reply-To` header.
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

/// `References` header.
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

/// `X-Priority` header.
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
/// Returns a reasonable default of `application/octet-stream` for unknown types.
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
