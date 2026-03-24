//! Attachment handling for Exospine.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use mail_parser::{MessageParser, MimeHeaders};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

/// Metadata for an email attachment (stored in memory -- lightweight).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentMeta {
    pub filename: String,
    pub content_type: String,
    pub size: usize,
    pub part_index: usize,
}

/// Extract attachment metadata from a raw RFC 5322 email.
pub fn extract_metadata(raw: &[u8]) -> Vec<AttachmentMeta> {
    let Some(message) = MessageParser::new().parse(raw) else {
        return Vec::new();
    };

    let mut metas = Vec::new();

    for (part_index, attachment) in message.attachments().enumerate() {
        let filename = attachment
            .attachment_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("attachment_{}", part_index));

        let content_type = attachment
            .content_type()
            .map(|ct| match ct.c_subtype.as_deref() {
                Some(sub) => format!("{}/{}", ct.c_type, sub),
                None => ct.c_type.to_string(),
            })
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let size = attachment.contents().len();

        metas.push(AttachmentMeta {
            filename,
            content_type,
            size,
            part_index,
        });
    }

    metas
}

/// Save a specific attachment to disk by extracting it from the raw email.
#[allow(dead_code)]
pub async fn save_attachment(raw: &[u8], part_index: usize, dest: &Path) -> Result<PathBuf> {
    let message = MessageParser::new()
        .parse(raw)
        .context("Failed to parse email for attachment extraction")?;

    let attachment = message
        .attachments()
        .nth(part_index)
        .with_context(|| format!("No attachment at part index {}", part_index))?;

    let mut filename = attachment
        .attachment_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("attachment_{}", part_index));

    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        filename = format!("attachment_{}", part_index);
    }

    let file_path = dest.join(&filename);

    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("Failed to create attachment directory")?;
    }

    let contents = attachment.contents();
    let mut file = tokio::fs::File::create(&file_path)
        .await
        .with_context(|| format!("Failed to create file: {}", file_path.display()))?;

    const CHUNK_SIZE: usize = 64 * 1024;
    for chunk in contents.chunks(CHUNK_SIZE) {
        file.write_all(chunk)
            .await
            .context("Failed to write attachment chunk")?;
    }

    file.flush()
        .await
        .context("Failed to flush attachment file")?;

    Ok(file_path)
}

/// Get the default download directory for attachments.
#[allow(dead_code)]
pub fn download_dir() -> PathBuf {
    dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Exospine")
}

/// Return a human-readable size string (e.g. "1.2 MB").
#[allow(dead_code)]
pub fn format_size(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = 1024 * KB;
    const GB: usize = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
