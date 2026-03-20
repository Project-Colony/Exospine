//! Search, filtering, and sorting for mail entries.
//!
//! All functions operate on slices and return index vectors rather than
//! cloning [`MailEntry`] values, keeping memory usage minimal.

use crate::state::MailEntry;

/// Filter mail entries by a search query, returning indices of matches.
///
/// Uses case-insensitive substring matching against the `from`, `subject`,
/// and `preview` fields. Returns all indices when `query` is empty.
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

/// Predefined quick-filter categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickFilter {
    /// Show all messages.
    All,
    /// Show only unread messages.
    Unread,
    /// Show only starred/flagged messages.
    Starred,
    /// Show only messages with attachments.
    HasAttachments,
}

/// Apply a [`QuickFilter`] and return matching indices.
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
pub enum SortBy {
    /// Newest first.
    DateDesc,
    /// Oldest first.
    DateAsc,
    /// Alphabetical by sender.
    Sender,
    /// Alphabetical by subject.
    Subject,
}

/// Sort a mutable slice of indices in place according to `sort`.
///
/// This avoids allocating a new vector — the caller's index buffer is
/// reordered directly.
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
