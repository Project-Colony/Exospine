//! Conversation threading for mail entries.
//!
//! Groups messages by normalised subject (stripping `Re:`, `Fwd:`, `FW:`
//! prefixes) so the UI can present a threaded/conversation view.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::state::MailEntry;

/// A conversation thread: a group of related emails sharing a subject.
#[derive(Debug, Clone)]
pub struct Thread {
    /// The normalised (lowercase) subject shared by all messages.
    pub subject: String,
    /// Indices into the original `mail_entries` slice.
    pub entries: Vec<usize>,
    /// The most recent date among the thread's messages.
    pub latest_date: DateTime<Utc>,
    /// Number of unread messages in the thread.
    pub unread_count: u32,
}

/// Strip leading `Re:`, `Fwd:`, and `FW:` prefixes from a subject line.
///
/// The comparison is case-insensitive but the returned slice preserves
/// the original casing of the remaining text.
pub fn normalize_subject(subject: &str) -> &str {
    let mut s = subject.trim();
    loop {
        let lower = s.to_lowercase();
        if lower.starts_with("re:") {
            s = s[3..].trim_start();
        } else if lower.starts_with("fwd:") || lower.starts_with("fw:") {
            // Use split_once so we skip exactly one colon regardless of prefix length.
            s = s.split_once(':').map(|(_, r)| r.trim_start()).unwrap_or(s);
        } else {
            break;
        }
    }
    s
}

/// Group mail entries into conversation threads sorted by latest date (newest first).
pub fn build_threads(entries: &[MailEntry]) -> Vec<Thread> {
    let mut map: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, entry) in entries.iter().enumerate() {
        let key = normalize_subject(&entry.subject).to_lowercase();
        map.entry(key).or_default().push(i);
    }

    let mut threads: Vec<Thread> = map
        .into_iter()
        .map(|(subject, indices)| {
            let latest_date = indices
                .iter()
                .map(|&i| entries[i].date)
                .max()
                .unwrap_or_else(Utc::now);
            let unread_count = indices.iter().filter(|&&i| !entries[i].is_read).count() as u32;
            Thread {
                subject,
                entries: indices,
                latest_date,
                unread_count,
            }
        })
        .collect();

    threads.sort_by(|a, b| b.latest_date.cmp(&a.latest_date));
    threads
}
