use anyhow::Result;
use rusqlite::params;

use super::db::Database;

impl Database {
    /// Add a category to a message.
    pub fn add_category(&self, mail_id: &str, category: &str) -> Result<()> {
        let current: String = self.conn().query_row(
            "SELECT categories FROM messages WHERE id = ?1",
            params![mail_id],
            |row| row.get(0),
        ).unwrap_or_else(|_| "[]".to_string());

        let mut cats: Vec<String> = serde_json::from_str(&current).unwrap_or_default();
        if !cats.iter().any(|c| c == category) {
            cats.push(category.to_string());
        }
        let json = serde_json::to_string(&cats).unwrap_or_default();
        self.conn().execute(
            "UPDATE messages SET categories = ?1 WHERE id = ?2",
            params![json, mail_id],
        )?;
        Ok(())
    }

    /// Remove a category from a message.
    pub fn remove_category(&self, mail_id: &str, category: &str) -> Result<()> {
        let current: String = self.conn().query_row(
            "SELECT categories FROM messages WHERE id = ?1",
            params![mail_id],
            |row| row.get(0),
        ).unwrap_or_else(|_| "[]".to_string());

        let mut cats: Vec<String> = serde_json::from_str(&current).unwrap_or_default();
        cats.retain(|c| c != category);
        let json = serde_json::to_string(&cats).unwrap_or_default();
        self.conn().execute(
            "UPDATE messages SET categories = ?1 WHERE id = ?2",
            params![json, mail_id],
        )?;
        Ok(())
    }

    /// Set categories for a message directly, bypassing parse->modify->serialize overhead.
    pub fn set_categories(&self, mail_id: &str, categories: &[String]) -> Result<()> {
        let json = serde_json::to_string(categories).unwrap_or_else(|_| "[]".to_string());
        self.conn().execute(
            "UPDATE messages SET categories = ?1 WHERE id = ?2",
            params![json, mail_id],
        )?;
        Ok(())
    }
}
