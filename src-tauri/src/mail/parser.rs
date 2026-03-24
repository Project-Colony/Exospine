use std::sync::OnceLock;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use mail_parser::MessageParser;
use regex::Regex;

use crate::app_state::MailEntry;
use crate::mail::attachments;
use crate::mail::security;
use crate::mail::spam_filter;

// ── Compiled regex patterns (compiled once, reused forever) ──────────

fn re_tags() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<[^>]+>").expect("invalid built-in regex: re_tags"))
}

fn re_attrs() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"\b(style|class|target|href|rel|align|bgcolor|valign|cellpadding|cellspacing|width|height|border|colspan|rowspan)="[^"]{0,2000}""#,
        )
        .expect("invalid built-in regex: re_attrs")
    })
}

fn re_css() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)\{[^}]*\}").expect("invalid built-in regex: re_css"))
}

fn re_ws() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[ \t]+").expect("invalid built-in regex: re_ws"))
}

fn re_nl() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\n{3,}").expect("invalid built-in regex: re_nl"))
}

// ── Public API ───────────────────────────────────────────────────────

/// Parse a raw email (RFC 5322) into a `MailEntry`.
pub fn parse_email(raw: &[u8], account_id: &str, folder: &str) -> Result<MailEntry> {
    let message = MessageParser::new()
        .parse(raw)
        .context("Failed to parse email")?;

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
        .map(|d| DateTime::from_timestamp(d.to_timestamp(), 0).unwrap_or_else(Utc::now))
        .unwrap_or_else(Utc::now);

    let raw_body_text = message
        .body_text(0)
        .map(|t| t.to_string())
        .unwrap_or_default();

    let body_html = message.body_html(0).map(|h| h.to_string());

    let effective_text = if raw_body_text.is_empty() {
        body_html
            .as_ref()
            .map(|h| html_to_text(h))
            .unwrap_or_default()
    } else if looks_like_html(&raw_body_text) {
        html_to_text(&raw_body_text)
    } else if let Some(ref html) = body_html {
        if raw_body_text.trim().is_empty() { html_to_text(html) } else { clean_text(&raw_body_text) }
    } else {
        clean_text(&raw_body_text)
    };

    let preview = generate_preview(&effective_text, 120);

    let has_attachments = message.attachment_count() > 0;
    let attachment_meta = attachments::extract_metadata(raw);

    // ── Threading headers ────────────────────────────────────────────
    let message_id = message
        .message_id()
        .map(|s| s.to_string())
        .unwrap_or_default();

    let in_reply_to: Option<String> = message
        .in_reply_to()
        .as_text_list()
        .and_then(|list| list.first().map(|s| s.to_string()));

    let references: Vec<String> = message
        .references()
        .as_text_list()
        .map(|list| list.iter().map(|s| s.to_string()).collect())
        .unwrap_or_default();

    // Compute thread_id: first reference in chain, or normalized subject
    let thread_id: String = if !references.is_empty() {
        references[0].clone()
    } else if let Some(ref irt) = in_reply_to {
        irt.clone()
    } else if !message_id.is_empty() {
        message_id.clone()
    } else {
        normalize_subject(&subject)
    };

    // ── Security analysis ────────────────────────────────────────────
    let reply_to = message
        .header("Reply-To")
        .and_then(|v| v.as_text().map(|s| s.to_string()));

    // Build raw headers string for Authentication-Results parsing.
    // We only need the header portion (before the first blank line).
    let raw_headers = std::str::from_utf8(raw)
        .unwrap_or("")
        .split("\r\n\r\n")
        .next()
        .unwrap_or("");

    let analysis = security::analyze_email(
        &from,
        reply_to.as_deref(),
        body_html.as_deref(),
        raw_headers,
    );

    // ── Read receipt header ─────────────────────────────────────────
    let read_receipt_to: Option<String> = message
        .header("Disposition-Notification-To")
        .and_then(|v| v.as_text().map(|s| s.to_string()));
    let read_receipt_requested = read_receipt_to.is_some();

    // ── Importance / priority headers ────────────────────────────
    let importance = {
        // Check Importance header first (standard), then X-Priority (de-facto)
        let imp_header = message
            .header("Importance")
            .and_then(|v| v.as_text().map(|s| s.to_lowercase()));
        let x_priority = message
            .header("X-Priority")
            .and_then(|v| v.as_text().map(|s| s.trim().to_string()));

        if let Some(ref imp) = imp_header {
            match imp.as_str() {
                "high" => "high".to_string(),
                "low" => "low".to_string(),
                _ => "normal".to_string(),
            }
        } else if let Some(ref xp) = x_priority {
            // X-Priority: 1 or 2 = high, 4 or 5 = low, 3 = normal
            match xp.chars().next() {
                Some('1') | Some('2') => "high".to_string(),
                Some('4') | Some('5') => "low".to_string(),
                _ => "normal".to_string(),
            }
        } else {
            "normal".to_string()
        }
    };

    let mut entry = MailEntry {
        id: String::new(),
        uid: 0,
        from,
        to,
        subject,
        date,
        preview,
        body_text: effective_text,
        body_html,
        is_read: false,
        is_starred: false,
        has_attachments,
        folder: folder.to_string(),
        account_id: account_id.to_string(),
        categories: Vec::new(),
        attachment_meta,
        phishing_warnings: analysis.phishing_warnings,
        auth_status: analysis.auth_status,
        sender_warnings: analysis.sender_warnings,
        message_id,
        in_reply_to,
        references,
        thread_id,
        spam_score: 0.0,
        is_pinned: false,
        snoozed_until: None,
        read_receipt_requested,
        read_receipt_to,
        importance,
        flag_due_date: None,
    };

    // Compute spam score
    entry.spam_score = spam_filter::spam_score(&entry);

    Ok(entry)
}

/// Generate a preview string from text in a single pass.
/// Strips control characters, collapses whitespace, and truncates to `max_chars`.
fn generate_preview(text: &str, max_chars: usize) -> String {
    let mut result = String::with_capacity(max_chars);
    let mut prev_was_space = true; // start true to skip leading whitespace

    for ch in text.chars() {
        if result.len() >= max_chars {
            break;
        }
        if ch.is_whitespace() || (ch.is_control() && ch != ' ') {
            if !prev_was_space && result.len() < max_chars {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            result.push(ch);
            prev_was_space = false;
        }
    }

    // Trim trailing space
    if result.ends_with(' ') {
        result.pop();
    }
    result
}

/// Convert HTML to readable plain text.
pub fn html_to_text(html: &str) -> String {
    let raw = html2text::from_read(html.as_bytes(), 80).unwrap_or_else(|_| html.to_string());
    clean_text(&raw)
}

fn clean_text(text: &str) -> String {
    let text = re_tags().replace_all(text, "");
    let text = re_attrs().replace_all(&text, "");
    let text = re_css().replace_all(&text, "");
    let text = re_ws().replace_all(&text, " ");
    re_nl().replace_all(&text, "\n\n").trim().to_string()
}

/// Strip reply/forward prefixes and normalize for thread grouping.
fn normalize_subject(subject: &str) -> String {
    static RE_PREFIX: OnceLock<Regex> = OnceLock::new();
    let re = RE_PREFIX.get_or_init(|| {
        Regex::new(r"(?i)^(re|fwd?|tr)\s*:\s*").expect("invalid normalize_subject regex")
    });
    let mut s = subject.to_string();
    loop {
        let trimmed = re.replace(&s, "").to_string();
        if trimmed == s {
            break;
        }
        s = trimmed;
    }
    s.trim().to_lowercase()
}

fn re_looks_like_html() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)<!doctype|<html|<br\s*/?>|<div|<p[\s>]|<table|</a>|<span|<img|<td|<tr|<head|<body|<style|&nbsp;|&amp;|style="|target=""#)
            .expect("invalid regex: re_looks_like_html")
    })
}

fn looks_like_html(s: &str) -> bool {
    re_looks_like_html().is_match(s.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_email() {
        let raw = b"From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: Hello\r\nDate: Mon, 1 Jan 2024 12:00:00 +0000\r\n\r\nThis is the body.";
        let entry = parse_email(raw, "acc1", "INBOX").unwrap();
        assert_eq!(entry.from, "sender@example.com");
        assert_eq!(entry.subject, "Hello");
        assert!(entry.body_text.contains("This is the body"));
        assert_eq!(entry.folder, "INBOX");
        assert_eq!(entry.account_id, "acc1");
    }

    #[test]
    fn test_parse_html_email() {
        let raw = b"From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: HTML Test\r\nDate: Mon, 1 Jan 2024 12:00:00 +0000\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<html><body><p>Hello <b>World</b></p></body></html>";
        let entry = parse_email(raw, "acc1", "INBOX").unwrap();
        assert_eq!(entry.subject, "HTML Test");
        assert!(entry.body_html.is_some());
        assert!(entry.body_text.contains("Hello"));
        assert!(entry.body_text.contains("World"));
    }

    #[test]
    fn test_parse_multipart_email() {
        let raw = b"From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: Multipart\r\nDate: Mon, 1 Jan 2024 12:00:00 +0000\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"boundary123\"\r\n\r\n--boundary123\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nPlain text version\r\n--boundary123\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<html><body><p>HTML version</p></body></html>\r\n--boundary123--";
        let entry = parse_email(raw, "acc1", "INBOX").unwrap();
        assert_eq!(entry.subject, "Multipart");
        assert!(entry.body_html.is_some());
        assert!(
            entry.body_text.contains("Plain text version")
                || entry.body_text.contains("HTML version")
        );
    }

    #[test]
    fn test_extract_date() {
        let raw = b"From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: Date Test\r\nDate: Tue, 15 Oct 2024 10:30:00 +0000\r\n\r\nBody";
        let entry = parse_email(raw, "acc1", "INBOX").unwrap();
        assert_eq!(entry.date.format("%Y-%m-%d").to_string(), "2024-10-15");
    }

    #[test]
    fn test_clean_text() {
        let result = clean_text("<p>Hello</p> <b>World</b>");
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
        assert!(!result.contains("<p>"));
        assert!(!result.contains("<b>"));

        let result = clean_text("Hello     World");
        assert_eq!(result, "Hello World");

        let result = clean_text("Hello\n\n\n\n\nWorld");
        assert_eq!(result, "Hello\n\nWorld");
    }

    #[test]
    fn test_html_to_text() {
        let html =
            "<html><body><p>Hello <b>World</b></p><br><p>Second paragraph</p></body></html>";
        let text = html_to_text(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
        assert!(text.contains("Second paragraph"));
    }

    #[test]
    fn test_normalize_subject() {
        assert_eq!(normalize_subject("Re: Hello"), "hello");
        assert_eq!(normalize_subject("Fwd: Hello"), "hello");
        assert_eq!(normalize_subject("Re: Re: Fw: Hello"), "hello");
        assert_eq!(normalize_subject("Hello"), "hello");
        // Leading whitespace is preserved by the regex (anchored at ^),
        // but trim() at the end handles trailing whitespace
        assert_eq!(normalize_subject("Re: Hello  "), "hello");
    }

    #[test]
    fn test_looks_like_html_positive() {
        assert!(looks_like_html("<html><body>test</body></html>"));
        assert!(looks_like_html("<p>text</p>"));
        assert!(looks_like_html("some text<br>more text"));
        assert!(looks_like_html("text with &nbsp; space"));
    }

    #[test]
    fn test_looks_like_html_negative() {
        assert!(!looks_like_html("Plain text email"));
        assert!(!looks_like_html("No HTML tags here"));
    }

    #[test]
    fn test_parse_email_no_subject() {
        let raw = b"From: sender@example.com\r\nTo: recipient@example.com\r\nDate: Mon, 1 Jan 2024 12:00:00 +0000\r\n\r\nBody";
        let entry = parse_email(raw, "acc1", "INBOX").unwrap();
        assert_eq!(entry.subject, "(no subject)");
    }

    #[test]
    fn test_parse_email_threading_headers() {
        let raw = b"From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: Re: Discussion\r\nDate: Mon, 1 Jan 2024 12:00:00 +0000\r\nMessage-ID: <msg2@example.com>\r\nIn-Reply-To: <msg1@example.com>\r\nReferences: <msg0@example.com> <msg1@example.com>\r\n\r\nBody";
        let entry = parse_email(raw, "acc1", "INBOX").unwrap();
        assert_eq!(entry.message_id, "msg2@example.com");
        assert_eq!(entry.in_reply_to.as_deref(), Some("msg1@example.com"));
        assert_eq!(entry.references.len(), 2);
        assert_eq!(entry.thread_id, "msg0@example.com");
    }

    #[test]
    fn test_parse_email_preview_length() {
        let body = "A ".repeat(200);
        let raw = format!(
            "From: sender@example.com\r\nTo: recipient@example.com\r\nSubject: Preview Test\r\nDate: Mon, 1 Jan 2024 12:00:00 +0000\r\n\r\n{}",
            body
        );
        let entry = parse_email(raw.as_bytes(), "acc1", "INBOX").unwrap();
        assert!(entry.preview.len() <= 120);
    }
}
