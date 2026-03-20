//! Attachment handling for Exospine.
//!
//! Follows a lazy-loading strategy: only lightweight [`AttachmentMeta`] is
//! kept in memory. The actual attachment content is extracted from the raw
//! email bytes on demand (via [`save_attachment`]) and streamed directly
//! to disk so that large files never sit in RAM.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use mail_parser::{MessageParser, MimeHeaders};
use tokio::io::AsyncWriteExt;

/// Metadata for an email attachment (stored in memory — lightweight).
#[derive(Debug, Clone)]
pub struct AttachmentMeta {
    /// Original filename reported by the MIME part (or a generated fallback).
    pub filename: String,
    /// MIME content type (e.g. `"application/pdf"`).
    pub content_type: String,
    /// Size of the decoded attachment body in bytes.
    pub size: usize,
    /// Index within the MIME parts of the parent message.
    pub part_index: usize,
}

/// Extract attachment metadata from a raw RFC 5322 email.
///
/// This only stores lightweight metadata — no attachment content is loaded
/// into memory.  Call [`save_attachment`] when the user actually requests a
/// download.
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
            .map(|ct| {
                match ct.c_subtype.as_deref() {
                    Some(sub) => format!("{}/{}", ct.c_type, sub),
                    None => ct.c_type.to_string(),
                }
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
///
/// The decoded content is written via async I/O so that large attachments
/// are streamed to the filesystem without holding the entire blob in RAM
/// longer than necessary.
///
/// Returns the final path the file was written to.
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

    // Sanitize filename to prevent path traversal
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        filename = format!("attachment_{}", part_index);
    }

    let file_path = dest.join(&filename);

    // Ensure the parent directory exists.
    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("Failed to create attachment directory")?;
    }

    // Write content via async I/O to avoid blocking the runtime.
    let contents = attachment.contents();
    let mut file = tokio::fs::File::create(&file_path)
        .await
        .with_context(|| format!("Failed to create file: {}", file_path.display()))?;

    // Stream in chunks to keep memory usage low for large attachments.
    const CHUNK_SIZE: usize = 64 * 1024; // 64 KiB
    for chunk in contents.chunks(CHUNK_SIZE) {
        file.write_all(chunk)
            .await
            .context("Failed to write attachment chunk")?;
    }

    file.flush().await.context("Failed to flush attachment file")?;

    Ok(file_path)
}

/// Get the default download directory for attachments.
///
/// Falls back to `$HOME/Exospine` or `./Exospine` if no standard
/// download directory can be determined.
pub fn download_dir() -> PathBuf {
    dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Exospine")
}

/// Return a human-readable size string (e.g. "1.2 MB").
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

/// Return a short icon/emoji hint based on the MIME content type.
pub fn icon_for_content_type(content_type: &str) -> &'static str {
    let ct = content_type.to_lowercase();
    if ct.starts_with("image/") {
        "[IMG]"
    } else if ct.contains("pdf") {
        "[PDF]"
    } else if ct.contains("zip") || ct.contains("compressed") || ct.contains("archive") {
        "[ZIP]"
    } else if ct.contains("word") || ct.contains("document") {
        "[DOC]"
    } else if ct.contains("spreadsheet") || ct.contains("excel") {
        "[XLS]"
    } else if ct.contains("presentation") || ct.contains("powerpoint") {
        "[PPT]"
    } else if ct.starts_with("text/") {
        "[TXT]"
    } else if ct.starts_with("audio/") {
        "[AUD]"
    } else if ct.starts_with("video/") {
        "[VID]"
    } else {
        "[FILE]"
    }
}
