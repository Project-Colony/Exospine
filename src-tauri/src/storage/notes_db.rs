use anyhow::Result;
use rusqlite::params;

use super::db::Database;

impl Database {
    /// Save or update a note for a mail message.
    pub fn save_note(&self, mail_id: &str, note: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn().execute(
            "INSERT OR REPLACE INTO notes (mail_id, note, updated_at)
             VALUES (?1, ?2, ?3)",
            params![mail_id, note, now],
        )?;
        Ok(())
    }

    /// Get a note for a mail message. Returns None if no note exists.
    pub fn get_note(&self, mail_id: &str) -> Result<Option<String>> {
        let result: Option<String> = self
            .conn()
            .query_row(
                "SELECT note FROM notes WHERE mail_id = ?1",
                params![mail_id],
                |row| row.get(0),
            )
            .ok();
        Ok(result)
    }

    /// Delete a note for a mail message.
    pub fn delete_note(&self, mail_id: &str) -> Result<()> {
        self.conn()
            .execute("DELETE FROM notes WHERE mail_id = ?1", params![mail_id])?;
        Ok(())
    }

    /// Check if a mail has a note.
    pub fn has_note(&self, mail_id: &str) -> bool {
        self.conn()
            .query_row(
                "SELECT 1 FROM notes WHERE mail_id = ?1 AND note != ''",
                params![mail_id],
                |_| Ok(()),
            )
            .is_ok()
    }
}
