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

    // Fallback: if content_type is generic octet-stream, infer from filename extension
    for meta in &mut metas {
        if meta.content_type == "application/octet-stream" {
            if let Some(better) = mime_from_extension(&meta.filename) {
                meta.content_type = better.to_string();
            }
        }
    }

    metas
}

/// Map common file extensions to MIME types.
fn mime_from_extension(filename: &str) -> Option<&'static str> {
    let ext = filename.rsplit('.').next()?.to_lowercase();
    match ext.as_str() {
        "txt" => Some("text/plain"),
        "pdf" => Some("application/pdf"),
        "csv" => Some("text/csv"),
        "json" => Some("application/json"),
        "md" => Some("text/markdown"),
        "xml" => Some("text/xml"),
        "html" | "htm" => Some("text/html"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "svg" => Some("image/svg+xml"),
        "zip" => Some("application/zip"),
        "gz" | "gzip" => Some("application/gzip"),
        "tar" => Some("application/x-tar"),
        "doc" => Some("application/msword"),
        "docx" => Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        "xls" => Some("application/vnd.ms-excel"),
        "xlsx" => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        "ppt" => Some("application/vnd.ms-powerpoint"),
        "pptx" => Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
        "mp3" => Some("audio/mpeg"),
        "mp4" => Some("video/mp4"),
        "wav" => Some("audio/wav"),
        "webp" => Some("image/webp"),
        "ico" => Some("image/x-icon"),
        "ics" => Some("text/calendar"),
        "eml" => Some("message/rfc822"),
        _ => None,
    }
}

/// Save a specific attachment to disk by extracting it from the raw email.
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
pub fn download_dir() -> PathBuf {
    dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Exospine")
}

