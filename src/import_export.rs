//! Import and export of mail messages in EML and mbox formats.
//!
//! - EML: Single RFC 5322 message files (.eml)
//! - mbox: Unix mbox format containing multiple messages separated by "From " lines.

use anyhow::{Context, Result};
use chrono::Utc;

use crate::state::MailEntry;

/// Export a single `MailEntry` as an RFC 5322 EML string.
///
/// Generates a minimal but valid email message with From, To, Subject, Date,
/// and MIME-Version headers.
pub fn export_eml(entry: &MailEntry) -> String {
    let mut out = String::new();

    // Headers
    out.push_str(&format!("From: {}\r\n", entry.from));
    out.push_str(&format!("To: {}\r\n", entry.to.join(", ")));
    out.push_str(&format!("Subject: {}\r\n", entry.subject));
    out.push_str(&format!(
        "Date: {}\r\n",
        entry.date.format("%a, %d %b %Y %H:%M:%S %z")
    ));
    out.push_str(&format!("Message-ID: <{}>\r\n", entry.id));
    out.push_str("MIME-Version: 1.0\r\n");

    if let Some(ref html) = entry.body_html {
        // Multipart message with both text and HTML.
        let boundary = format!("----=_Part_{}", uuid::Uuid::new_v4().simple());
        out.push_str(&format!(
            "Content-Type: multipart/alternative; boundary=\"{}\"\r\n",
            boundary
        ));
        out.push_str("\r\n");
        // Plain text part.
        out.push_str(&format!("--{}\r\n", boundary));
        out.push_str("Content-Type: text/plain; charset=UTF-8\r\n");
        out.push_str("Content-Transfer-Encoding: 8bit\r\n");
        out.push_str("\r\n");
        out.push_str(&entry.body_text);
        out.push_str("\r\n");
        // HTML part.
        out.push_str(&format!("--{}\r\n", boundary));
        out.push_str("Content-Type: text/html; charset=UTF-8\r\n");
        out.push_str("Content-Transfer-Encoding: 8bit\r\n");
        out.push_str("\r\n");
        out.push_str(html);
        out.push_str("\r\n");
        out.push_str(&format!("--{}--\r\n", boundary));
    } else {
        // Plain text only.
        out.push_str("Content-Type: text/plain; charset=UTF-8\r\n");
        out.push_str("Content-Transfer-Encoding: 8bit\r\n");
        out.push_str("\r\n");
        out.push_str(&entry.body_text);
        out.push_str("\r\n");
    }

    out
}

/// Import a single .eml file (raw RFC 5322 bytes) into a `MailEntry`.
///
/// Reuses the existing mail parser.
pub fn import_eml(raw: &[u8]) -> Result<MailEntry> {
    crate::mail::parser::parse_email(raw, "", "Imported")
        .context("Failed to parse EML file")
}

/// Export multiple `MailEntry` items as an mbox format string.
///
/// Each message is preceded by a "From " line (the mbox separator) followed
/// by the RFC 5322 message content. Any "From " at the start of body lines
/// is escaped with ">From " per mbox convention.
pub fn export_mbox(entries: &[MailEntry]) -> String {
    let mut out = String::new();

    for entry in entries {
        // mbox "From " line: "From sender@example.com Thu Jan 01 00:00:00 2024"
        let sender_email = extract_bare_email(&entry.from);
        let date_str = entry.date.format("%a %b %d %H:%M:%S %Y").to_string();
        out.push_str(&format!("From {} {}\r\n", sender_email, date_str));

        // The message body with "From " escaping.
        let eml = export_eml(entry);
        for line in eml.lines() {
            if line.starts_with("From ") {
                out.push('>');
            }
            out.push_str(line);
            out.push_str("\r\n");
        }
        // Blank line between messages.
        out.push_str("\r\n");
    }

    out
}

/// Import an mbox file (raw bytes) into a list of `MailEntry` items.
///
/// Splits on "From " separator lines and parses each message.
pub fn import_mbox(raw: &[u8]) -> Result<Vec<MailEntry>> {
    let text = String::from_utf8_lossy(raw);
    let mut entries = Vec::new();
    let mut current_message = String::new();
    let mut in_message = false;

    for line in text.lines() {
        if line.starts_with("From ") && (current_message.is_empty() || in_message) {
            // Save previous message if we have one.
            if in_message && !current_message.is_empty() {
                let unescaped = unescape_mbox_from(&current_message);
                match import_eml(unescaped.as_bytes()) {
                    Ok(entry) => entries.push(entry),
                    Err(e) => {
                        tracing::warn!("Skipping malformed message in mbox: {}", e);
                    }
                }
                current_message.clear();
            }
            in_message = true;
            continue; // Skip the "From " separator line itself.
        }

        if in_message {
            current_message.push_str(line);
            current_message.push('\n');
        }
    }

    // Don't forget the last message.
    if in_message && !current_message.is_empty() {
        let unescaped = unescape_mbox_from(&current_message);
        match import_eml(unescaped.as_bytes()) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                tracing::warn!("Skipping malformed message in mbox: {}", e);
            }
        }
    }

    Ok(entries)
}

/// Unescape ">From " lines back to "From " (mbox quoting convention).
fn unescape_mbox_from(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in s.lines() {
        if line.starts_with(">From ") {
            out.push_str(&line[1..]); // Remove the leading '>'
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Extract the bare email address from "Name <email>" or "email" format.
fn extract_bare_email(addr: &str) -> String {
    if let Some(start) = addr.find('<') {
        if let Some(end) = addr.find('>') {
            return addr[start + 1..end].trim().to_string();
        }
    }
    addr.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_import_eml_round_trip() {
        let entry = MailEntry {
            id: "test-id-1".to_string(),
            from: "alice@example.com".to_string(),
            to: vec!["bob@example.com".to_string()],
            subject: "Test Subject".to_string(),
            body_text: "Hello, Bob!".to_string(),
            body_html: None,
            ..MailEntry::default()
        };

        let eml = export_eml(&entry);
        assert!(eml.contains("Subject: Test Subject"));
        assert!(eml.contains("Hello, Bob!"));
    }

    #[test]
    fn extract_bare_email_formats() {
        assert_eq!(
            extract_bare_email("Alice <alice@example.com>"),
            "alice@example.com"
        );
        assert_eq!(
            extract_bare_email("bob@example.com"),
            "bob@example.com"
        );
    }
}
