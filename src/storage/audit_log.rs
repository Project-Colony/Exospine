//! Security audit log for Exospine.
//!
//! Logs security-relevant events to `security.log` in the data directory.
//! Each line is formatted as: `[timestamp] EVENT_TYPE | details`

use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;

/// Append a security event to the audit log.
///
/// Event types:
/// - `OAUTH_LOGIN` — successful OAuth login (email, provider)
/// - `OAUTH_FAIL` — failed OAuth attempt (reason)
/// - `PHISHING_DETECTED` — phishing link found (mail subject, link)
/// - `HOMOGRAPH_DETECTED` — homograph domain (sender, domain)
/// - `ATTACHMENT_BLOCKED` — dangerous attachment blocked (filename)
/// - `ATTACHMENT_SCANNED` — Defender scan result (filename, clean/threat)
/// - `AUTH_FAIL` — SPF/DKIM failure (sender)
/// - `LINK_OPENED` — external link opened (URL)
pub fn log_event(event_type: &str, details: &str) {
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    let log_path = &crate::paths::layout().security_log;
    if let Some(dir) = log_path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "[{}] {} | {}", timestamp, event_type, details);
    }
}

/// Read the last `n` lines from the security log.
/// Returns the lines in chronological order (oldest first).
pub fn read_last_entries(n: usize) -> Vec<String> {
    let content = match std::fs::read_to_string(&crate::paths::layout().security_log) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let lines: Vec<String> = content
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    let start = if lines.len() > n { lines.len() - n } else { 0 };
    lines[start..].to_vec()
}
