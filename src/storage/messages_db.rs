use anyhow::Result;
use rusqlite::params;

use super::db::{row_to_mail_entry_full, row_to_mail_entry_headers, Database};
use crate::app_state::MailEntry;

impl Database {
    pub fn save_messages(&self, messages: &[MailEntry]) -> Result<()> {
        // Split into chunks of 500 to prevent long DB locks.
        const CHUNK_SIZE: usize = 500;
        for chunk in messages.chunks(CHUNK_SIZE) {
            self.save_messages_chunk(chunk)?;
        }
        Ok(())
    }

    /// Save a single chunk of messages in one transaction.
    fn save_messages_chunk(&self, messages: &[MailEntry]) -> Result<()> {
        let tx = self.conn().unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO messages
                    (id, account_id, folder, from_addr, to_addrs, subject, date, preview, body_text, body_html, is_read, is_starred, has_attachments, uid, message_id, in_reply_to, refs, thread_id, spam_score, categories, is_pinned, snoozed_until, importance, read_receipt_requested, read_receipt_to, flag_due_date, unsubscribe_url)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27)",
            )?;

            for m in messages {
                let to_json = serde_json::to_string(&m.to).unwrap_or_default();
                let refs_json = serde_json::to_string(&m.references).unwrap_or_default();
                let cats_json = serde_json::to_string(&m.categories).unwrap_or_default();
                stmt.execute(params![
                    m.id,
                    m.account_id,
                    m.folder,
                    m.from,
                    to_json,
                    m.subject,
                    m.date.to_rfc3339(),
                    m.preview,
                    m.body_text,
                    m.body_html,
                    m.is_read as i32,
                    m.is_starred as i32,
                    m.has_attachments as i32,
                    m.uid,
                    m.message_id,
                    m.in_reply_to,
                    refs_json,
                    m.thread_id,
                    m.spam_score,
                    cats_json,
                    m.is_pinned as i32,
                    m.snoozed_until,
                    m.importance,
                    m.read_receipt_requested as i32,
                    m.read_receipt_to,
                    m.flag_due_date,
                    m.unsubscribe_url,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn load_messages(&self, account_id: &str, folder: &str) -> Result<Vec<MailEntry>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, account_id, folder, from_addr, to_addrs, subject, date, preview, body_text, body_html, is_read, is_starred, has_attachments, uid, message_id, in_reply_to, refs, thread_id, spam_score, categories, is_pinned, snoozed_until, importance, read_receipt_requested, read_receipt_to, flag_due_date, unsubscribe_url
             FROM messages
             WHERE account_id = ?1 AND folder = ?2
             ORDER BY date DESC",
        )?;

        let entries = stmt
            .query_map(params![account_id, folder], |row| {
                row_to_mail_entry_full(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    pub fn load_message_headers_page(
        &self,
        account_id: &str,
        folder: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MailEntry>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, account_id, folder, from_addr, to_addrs, subject, date, preview, is_read, is_starred, has_attachments, uid, message_id, in_reply_to, refs, thread_id, spam_score, categories, is_pinned, snoozed_until, importance, read_receipt_requested, read_receipt_to, flag_due_date, unsubscribe_url
             FROM messages
             WHERE account_id = ?1 AND folder = ?2
             ORDER BY date DESC
             LIMIT ?3 OFFSET ?4",
        )?;

        let entries = stmt
            .query_map(params![account_id, folder, limit, offset], |row| {
                row_to_mail_entry_headers(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    /// Load account_id, folder, and uid for a message (used for IMAP fallback).
    pub fn load_message_meta(&self, id: &str) -> Result<(String, String, u32)> {
        let mut stmt = self
            .conn()
            .prepare("SELECT account_id, folder, uid FROM messages WHERE id = ?1")?;

        let result = stmt.query_row(params![id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u32>(2)?,
            ))
        })?;

        Ok(result)
    }

    pub fn load_message_body(&self, id: &str) -> Result<(String, Option<String>)> {
        let mut stmt = self
            .conn()
            .prepare("SELECT body_text, body_html FROM messages WHERE id = ?1")?;

        let result = stmt.query_row(params![id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;

        Ok(result)
    }

    pub fn delete_message(&self, id: &str) -> Result<()> {
        self.secure_delete_message(id)
    }

    /// Securely delete a message by overwriting sensitive fields before removal.
    /// This prevents recovery of email content from the SQLite database file.
    pub fn secure_delete_message(&self, id: &str) -> Result<()> {
        // Step 1: Overwrite sensitive fields with empty/placeholder data
        self.conn().execute(
            "UPDATE messages SET body_text = '', body_html = NULL, subject = '[deleted]', from_addr = '', to_addrs = '[]', preview = '' WHERE id = ?1",
            params![id],
        )?;
        // Step 2: Actually delete the row
        self.conn()
            .execute("DELETE FROM messages WHERE id = ?1", params![id])?;
        // Note: VACUUM is intentionally not called here as it is expensive.
        // It can be triggered separately during maintenance.
        Ok(())
    }

    pub fn max_uid(&self, account_id: &str, folder: &str) -> Result<u32> {
        let uid: u32 = self.conn().query_row(
            "SELECT COALESCE(MAX(uid), 0) FROM messages WHERE account_id = ?1 AND folder = ?2",
            params![account_id, folder],
            |row| row.get(0),
        )?;
        Ok(uid)
    }

    /// Return all cached UIDs for a given account + folder.
    pub fn cached_uids(&self, account_id: &str, folder: &str) -> Result<Vec<u32>> {
        let mut stmt = self.conn().prepare(
            "SELECT uid FROM messages WHERE account_id = ?1 AND folder = ?2 AND uid > 0",
        )?;
        let uids = stmt
            .query_map(params![account_id, folder], |row| row.get::<_, u32>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(uids)
    }

    pub fn message_count(&self, account_id: &str, folder: &str) -> Result<u32> {
        let count: u32 = self.conn().query_row(
            "SELECT COUNT(*) FROM messages WHERE account_id = ?1 AND folder = ?2",
            params![account_id, folder],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Delete all messages from a given sender in a folder. Returns UIDs of deleted messages.
    pub fn sweep_sender(&self, account_id: &str, folder: &str, sender_email: &str) -> Result<Vec<String>> {
        let pattern = format!("%{}%", sender_email);
        let mut stmt = self.conn().prepare(
            "SELECT id, uid FROM messages WHERE account_id = ?1 AND folder = ?2 AND from_addr LIKE ?3",
        )?;
        let entries: Vec<(String, String)> = stmt
            .query_map(params![account_id, folder, pattern], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let uids: Vec<String> = entries.iter().map(|(_, uid)| uid.clone()).collect();

        for (id, _) in &entries {
            self.delete_message(id)?;
        }

        Ok(uids)
    }
}
