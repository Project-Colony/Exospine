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

/// Percent-decoding for mailto URLs with proper UTF-8 handling.
///
/// Multi-byte UTF-8 characters are encoded as consecutive `%XX` pairs (e.g.
/// `%C3%A9` for `é`). This implementation collects raw decoded bytes and then
/// converts them to a UTF-8 string, replacing any invalid sequences with the
/// Unicode replacement character.
pub fn url_decode(s: &str) -> String {
    let mut bytes = Vec::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next();
            let lo = chars.next();
            match (hi, lo) {
                (Some(h), Some(l)) if h.is_ascii_hexdigit() && l.is_ascii_hexdigit() => {
                    let hex = [h, l];
                    // SAFETY: we just verified both bytes are ASCII hex digits
                    if let Ok(val) = u8::from_str_radix(
                        std::str::from_utf8(&hex).unwrap(),
                        16,
                    ) {
                        bytes.push(val);
                        continue;
                    }
                    // Fallthrough: push literal '%' + the two chars
                    bytes.push(b'%');
                    bytes.push(h);
                    bytes.push(l);
                }
                (Some(h), Some(l)) => {
                    // Not valid hex — emit literally
                    bytes.push(b'%');
                    bytes.push(h);
                    bytes.push(l);
                }
                (Some(h), None) => {
                    // Truncated: only one char after '%'
                    bytes.push(b'%');
                    bytes.push(h);
                }
                (None, _) => {
                    // Trailing '%' at end of string
                    bytes.push(b'%');
                }
            }
        } else if b == b'+' {
            bytes.push(b' ');
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|e| {
        String::from_utf8_lossy(e.as_bytes()).into_owned()
    })
}

/// Parse an `exospine://compose?to=...&subject=...` URL into MailtoData.
pub fn parse_exospine_url(url: &str) -> MailtoData {
    let mut data = MailtoData::default();
    // Strip the scheme: "exospine://compose?..." -> "compose?..."
    let stripped = url
        .strip_prefix("exospine://")
        .unwrap_or(url);

    // Find query string after '?'
    let query_part = stripped.find('?').map(|idx| &stripped[idx + 1..]);

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
