//! URL parsing for mailto: and exospine:// protocol handlers.

use std::sync::{LazyLock, Mutex};

/// Parsed mailto: URL components.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct MailtoData {
    pub to: String,
    pub subject: String,
    pub body: String,
    pub cc: String,
    pub bcc: String,
}

/// Global storage for a pending mailto: compose request, picked up by the frontend.
pub static PENDING_MAILTO: LazyLock<Mutex<Option<MailtoData>>> =
    LazyLock::new(|| Mutex::new(None));

/// Parse a `mailto:` URL into its components.
pub fn parse_mailto(url: &str) -> MailtoData {
    let mut data = MailtoData::default();
    let stripped = url.strip_prefix("mailto:").unwrap_or(url);

    // Split address part from query string
    let (addr_part, query_part) = if let Some(idx) = stripped.find('?') {
        (&stripped[..idx], Some(&stripped[idx + 1..]))
    } else {
        (stripped, None)
    };

    // URL-decode the address
    data.to = url_decode(addr_part);

    // Parse query parameters
    if let Some(query) = query_part {
        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                let decoded = url_decode(value);
                match key.to_lowercase().as_str() {
                    "subject" => data.subject = decoded,
                    "body" => data.body = decoded,
                    "cc" => data.cc = decoded,
                    "bcc" => data.bcc = decoded,
                    "to" => {
                        if !data.to.is_empty() {
                            data.to.push_str(", ");
                        }
                        data.to.push_str(&decoded);
                    }
                    _ => {}
                }
            }
        }
    }

    data
}

/// Simple percent-decoding for mailto URLs.
pub fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().unwrap_or(0);
            let lo = chars.next().unwrap_or(0);
            let hex = [hi, lo];
            if let Ok(s) = std::str::from_utf8(&hex) {
                if let Ok(val) = u8::from_str_radix(s, 16) {
                    result.push(val as char);
                    continue;
                }
            }
            result.push('%');
            result.push(hi as char);
            result.push(lo as char);
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    result
}

/// Parse an `exospine://compose?to=...&subject=...` URL into MailtoData.
pub fn parse_exospine_url(url: &str) -> MailtoData {
    let mut data = MailtoData::default();
    // Strip the scheme: "exospine://compose?..." -> "compose?..."
    let stripped = url
        .strip_prefix("exospine://")
        .unwrap_or(url);

    // Find query string after '?'
    let query_part = if let Some(idx) = stripped.find('?') {
        Some(&stripped[idx + 1..])
    } else {
        None
    };

    if let Some(query) = query_part {
        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                let decoded = url_decode(value);
                match key.to_lowercase().as_str() {
                    "to" => {
                        if !data.to.is_empty() {
                            data.to.push_str(", ");
                        }
                        data.to.push_str(&decoded);
                    }
                    "subject" => data.subject = decoded,
                    "body" => data.body = decoded,
                    "cc" => data.cc = decoded,
                    "bcc" => data.bcc = decoded,
                    _ => {}
                }
            }
        }
    }

    data
}
