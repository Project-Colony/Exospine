//! Folder management operations for Exospine.
//!
//! Provides CRUD operations on IMAP mailbox folders plus helpers for
//! fetching folder metadata (unread / total counts) and classifying
//! folders by their IMAP SPECIAL-USE attributes or by name.

use anyhow::{Context, Result};
use futures::TryStreamExt;

use crate::mail::imap::ImapSession;
use crate::state::{Folder, FolderType};

/// Fetch all folders with their unread and total message counts.
///
/// Issues `LIST "" "*"` followed by `STATUS` for each mailbox to populate
/// the [`Folder::unread_count`] and [`Folder::total_count`] fields.
pub async fn fetch_folders_with_counts(session: &mut ImapSession) -> Result<Vec<Folder>> {
    let mailboxes: Vec<_> = session
        .list(None, Some("*"))
        .await
        .context("Failed to LIST mailboxes")?
        .try_collect::<Vec<_>>()
        .await
        .context("Failed to collect mailbox list")?;

    let mut folders = Vec::with_capacity(mailboxes.len());

    for mb in &mailboxes {
        let name = mb.name().to_string();

        // Collect IMAP name attributes (e.g. \Sent, \Trash, \Noselect).
        let attributes: Vec<String> = mb
            .attributes()
            .iter()
            .map(|attr| format!("{:?}", attr))
            .collect();
        let attr_refs: Vec<&str> = attributes.iter().map(|s| s.as_str()).collect();

        let folder_type = classify_folder(&name, &attr_refs);

        // Skip \Noselect folders — they are hierarchy placeholders.
        let is_noselect = attr_refs.iter().any(|a| {
            let lower = a.to_lowercase();
            lower.contains("noselect")
        });
        if is_noselect {
            folders.push(Folder {
                name,
                unread_count: 0,
                total_count: 0,
                folder_type,
            });
            continue;
        }

        // Fetch UNSEEN and MESSAGES counts via STATUS.
        let (unread_count, total_count) =
            match session.status(&name, "(UNSEEN MESSAGES)").await {
                Ok(mailbox) => {
                    let unseen = mailbox.unseen.unwrap_or(0) as u32;
                    let messages = mailbox.exists as u32;
                    (unseen, messages)
                }
                Err(e) => {
                    tracing::warn!("STATUS failed for {}: {}", name, e);
                    (0, 0)
                }
            };

        folders.push(Folder {
            name,
            unread_count,
            total_count,
            folder_type,
        });
    }

    Ok(folders)
}

/// Create a new folder on the IMAP server.
pub async fn create_folder(session: &mut ImapSession, name: &str) -> Result<()> {
    session
        .create(name)
        .await
        .with_context(|| format!("Failed to create folder: {}", name))?;
    Ok(())
}

/// Rename a folder on the IMAP server.
pub async fn rename_folder(
    session: &mut ImapSession,
    old_name: &str,
    new_name: &str,
) -> Result<()> {
    session
        .rename(old_name, new_name)
        .await
        .with_context(|| format!("Failed to rename folder {} -> {}", old_name, new_name))?;
    Ok(())
}

/// Delete a folder on the IMAP server.
pub async fn delete_folder(session: &mut ImapSession, name: &str) -> Result<()> {
    session
        .delete(name)
        .await
        .with_context(|| format!("Failed to delete folder: {}", name))?;
    Ok(())
}

/// Expunge all messages in the Trash folder.
///
/// Selects the trash folder, flags every message with `\Deleted`, and
/// issues an `EXPUNGE` to permanently remove them.
pub async fn empty_trash(session: &mut ImapSession, trash_folder: &str) -> Result<()> {
    session
        .select(trash_folder)
        .await
        .with_context(|| format!("Failed to select trash folder: {}", trash_folder))?;

    // Flag all messages as deleted.
    let all = "1:*";
    session
        .uid_store(all, "+FLAGS (\\Deleted)")
        .await
        .context("Failed to flag messages for deletion")?
        .try_collect::<Vec<_>>()
        .await
        .context("Failed to collect store response")?;

    // Permanently remove flagged messages.
    session
        .expunge()
        .await
        .context("Failed to expunge trash")?
        .try_collect::<Vec<_>>()
        .await
        .context("Failed to collect expunge response")?;

    Ok(())
}

/// Detect the semantic type of a folder from its name and IMAP attributes.
///
/// Checks IMAP SPECIAL-USE attributes first (RFC 6154), then falls back
/// to name-based heuristics.
pub fn classify_folder(name: &str, attributes: &[&str]) -> FolderType {
    // Check IMAP SPECIAL-USE attributes first.
    for attr in attributes {
        let lower = attr.to_lowercase();
        if lower.contains("inbox") {
            return FolderType::Inbox;
        }
        if lower.contains("sent") {
            return FolderType::Sent;
        }
        if lower.contains("drafts") || lower.contains("draft") {
            return FolderType::Drafts;
        }
        if lower.contains("trash") || lower.contains("bin") {
            return FolderType::Trash;
        }
        if lower.contains("junk") || lower.contains("spam") {
            return FolderType::Trash;
        }
        if lower.contains("flagged") || lower.contains("starred") {
            return FolderType::Starred;
        }
    }

    // Fallback: name-based detection.
    let lower = name.to_lowercase();
    if lower == "inbox" {
        FolderType::Inbox
    } else if lower.contains("sent") {
        FolderType::Sent
    } else if lower.contains("draft") {
        FolderType::Drafts
    } else if lower.contains("trash") || lower.contains("deleted") || lower.contains("bin") {
        FolderType::Trash
    } else if lower.contains("junk") || lower.contains("spam") {
        FolderType::Trash
    } else if lower.contains("starred") || lower.contains("flagged") {
        FolderType::Starred
    } else {
        FolderType::Custom
    }
}
