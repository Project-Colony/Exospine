use anyhow::Result;
use rusqlite::params;

use super::db::Database;

impl Database {
    pub fn update_read_status(&self, id: &str, is_read: bool) -> Result<()> {
        self.conn().execute(
            "UPDATE messages SET is_read = ?1 WHERE id = ?2",
            params![is_read as i32, id],
        )?;
        Ok(())
    }

    pub fn update_star_status(&self, id: &str, is_starred: bool) -> Result<()> {
        self.conn().execute(
            "UPDATE messages SET is_starred = ?1 WHERE id = ?2",
            params![is_starred as i32, id],
        )?;
        Ok(())
    }

    pub fn update_pinned_status(&self, id: &str, is_pinned: bool) -> Result<()> {
        self.conn().execute(
            "UPDATE messages SET is_pinned = ?1 WHERE id = ?2",
            params![is_pinned as i32, id],
        )?;
        Ok(())
    }

    pub fn update_snoozed_until(&self, id: &str, until: Option<&str>) -> Result<()> {
        self.conn().execute(
            "UPDATE messages SET snoozed_until = ?1 WHERE id = ?2",
            params![until, id],
        )?;
        Ok(())
    }

    /// Get IDs of snoozed messages that are due (snoozed_until <= now).
    pub fn get_due_snoozed_ids(&self) -> Result<Vec<String>> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = self.conn().prepare(
            "SELECT id FROM messages WHERE snoozed_until IS NOT NULL AND snoozed_until <= ?1",
        )?;
        let ids = stmt
            .query_map(params![now], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    pub fn update_flag_due_date(&self, id: &str, due_date: Option<&str>) -> Result<()> {
        self.conn().execute(
            "UPDATE messages SET flag_due_date = ?1 WHERE id = ?2",
            params![due_date, id],
        )?;
        Ok(())
    }
}
