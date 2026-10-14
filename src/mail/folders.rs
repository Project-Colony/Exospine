//! Folder management operations for Exospine.

use anyhow::{Context, Result};
use futures::TryStreamExt;

use crate::mail::imap::ImapSession;
use crate::app_state::{Folder, FolderType};

/// Fetch all folders with their unread and total message counts.
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

        let attributes: Vec<String> = mb
            .attributes()
            .iter()
            .map(|attr| format!("{:?}", attr))
            .collect();
        let attr_refs: Vec<&str> = attributes.iter().map(|s| s.as_str()).collect();

        let folder_type = classify_folder(&name, &attr_refs);

        let is_noselect = attr_refs.iter().any(|a| {
            let lower = a.to_lowercase();
            lower.contains("noselect")
        });
        if is_noselect {
            // Skip non-selectable folders (e.g. "[Gmail]" parent)
            continue;
        }

        let (unread_count, total_count) =
            match session.status(&name, "(UNSEEN MESSAGES)").await {
                Ok(mailbox) => {
                    let unseen = mailbox.unseen.unwrap_or(0);
                    let messages = mailbox.exists;
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

pub fn classify_folder(name: &str, attributes: &[&str]) -> FolderType {
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

    let lower = name.to_lowercase();
    if lower == "inbox" {
        FolderType::Inbox
    } else if lower.contains("sent") {
        FolderType::Sent
    } else if lower.contains("draft") {
        FolderType::Drafts
    } else if lower.contains("trash") || lower.contains("deleted") || lower.contains("bin")
        || lower.contains("junk") || lower.contains("spam")
    {
        FolderType::Trash
    } else if lower.contains("starred") || lower.contains("flagged") {
        FolderType::Starred
    } else {
        FolderType::Custom
    }
}
