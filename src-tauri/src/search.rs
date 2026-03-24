//! Search, filtering, and sorting for mail entries.
//!
//! These utilities are kept for future use when client-side search/filter
//! is wired into the frontend.

use crate::app_state::{MailEntry, QuickFilter};

/// Filter mail entries by a search query, returning indices of matches.
#[allow(dead_code)]
pub fn filter_entries(entries: &[MailEntry], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..entries.len()).collect();
    }
    let query_lower = query.to_lowercase();
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e.from.to_lowercase().contains(&query_lower)
                || e.subject.to_lowercase().contains(&query_lower)
                || e.preview.to_lowercase().contains(&query_lower)
        })
        .map(|(i, _)| i)
        .collect()
}

/// Apply a [`QuickFilter`] and return matching indices.
#[allow(dead_code)]
pub fn apply_quick_filter(entries: &[MailEntry], filter: &QuickFilter) -> Vec<usize> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| match filter {
            QuickFilter::All => true,
            QuickFilter::Unread => !e.is_read,
            QuickFilter::Starred => e.is_starred,
            QuickFilter::HasAttachments => e.has_attachments,
        })
        .map(|(i, _)| i)
        .collect()
}

/// Sort criteria for the mail list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SortBy {
    DateDesc,
    DateAsc,
    Sender,
    Subject,
}

/// Sort a mutable slice of indices in place according to `sort`.
#[allow(dead_code)]
pub fn sort_indices(entries: &[MailEntry], indices: &mut [usize], sort: &SortBy) {
    indices.sort_by(|&a, &b| {
        let ea = &entries[a];
        let eb = &entries[b];
        match sort {
            SortBy::DateDesc => eb.date.cmp(&ea.date),
            SortBy::DateAsc => ea.date.cmp(&eb.date),
            SortBy::Sender => ea.from.cmp(&eb.from),
            SortBy::Subject => ea.subject.cmp(&eb.subject),
        }
    });
}
