use anyhow::Result;
use rusqlite::params;

use super::db::{row_to_mail_entry_full, Database};
use crate::app_state::MailEntry;

impl Database {
    pub fn search_messages_in_folder(&self, query: &str, account_id: &str, folder: &str) -> Result<Vec<MailEntry>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn().prepare(
            "SELECT id, account_id, folder, from_addr, to_addrs, subject, date, preview, body_text, body_html, is_read, is_starred, has_attachments, uid, message_id, in_reply_to, refs, thread_id, spam_score, categories, is_pinned, snoozed_until
             FROM messages
             WHERE account_id = ?1 AND folder = ?2 AND (subject LIKE ?3 OR from_addr LIKE ?3 OR preview LIKE ?3)
             ORDER BY date DESC
             LIMIT 100",
        )?;

        let entries = stmt
            .query_map(params![account_id, folder, pattern], |row| {
                row_to_mail_entry_full(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    pub fn search_messages(&self, query: &str) -> Result<Vec<MailEntry>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn().prepare(
            "SELECT id, account_id, folder, from_addr, to_addrs, subject, date, preview, body_text, body_html, is_read, is_starred, has_attachments, uid, message_id, in_reply_to, refs, thread_id, spam_score, categories, is_pinned, snoozed_until
             FROM messages
             WHERE subject LIKE ?1 OR from_addr LIKE ?1 OR body_text LIKE ?1
             ORDER BY date DESC
             LIMIT 100",
        )?;

        let entries = stmt
            .query_map(params![pattern], |row| {
                row_to_mail_entry_full(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    /// Execute a dynamic SQL search query with positional string parameters.
    /// The SQL must select the same columns as `row_to_mail_entry_full`.
    pub fn search_messages_dynamic(&self, sql: &str, params: &[String]) -> Result<Vec<MailEntry>> {
        let mut stmt = self.conn().prepare(sql)?;
        let entries = stmt
            .query_map(
                params.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect::<Vec<_>>().as_slice(),
                row_to_mail_entry_full,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(entries)
    }
}
