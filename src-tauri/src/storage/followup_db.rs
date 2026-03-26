use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::db::Database;

/// A follow-up tracker entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Followup {
    pub mail_id: String,
    pub expected_from: String,
    pub created_at: String,
    pub due_date: String,
    pub resolved: bool,
}

impl Database {
    /// Add a follow-up for a mail (waiting for a reply from expected_from).
    pub fn add_followup(&self, mail_id: &str, expected_from: &str, due_date: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn().execute(
            "INSERT OR REPLACE INTO followups (mail_id, expected_from, created_at, due_date, resolved)
             VALUES (?1, ?2, ?3, ?4, 0)",
            params![mail_id, expected_from, now, due_date],
        )?;
        Ok(())
    }

    /// Get all follow-ups (unresolved first, then by due_date).
    pub fn get_followups(&self) -> Result<Vec<Followup>> {
        let mut stmt = self.conn().prepare(
            "SELECT mail_id, expected_from, created_at, due_date, resolved
             FROM followups
             ORDER BY resolved ASC, due_date ASC",
        )?;
        let results = stmt
            .query_map([], |row| {
                Ok(Followup {
                    mail_id: row.get(0)?,
                    expected_from: row.get(1)?,
                    created_at: row.get(2)?,
                    due_date: row.get(3)?,
                    resolved: row.get::<_, i32>(4)? != 0,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(results)
    }

    /// Mark a follow-up as resolved.
    pub fn resolve_followup(&self, mail_id: &str) -> Result<()> {
        self.conn().execute(
            "UPDATE followups SET resolved = 1 WHERE mail_id = ?1",
            params![mail_id],
        )?;
        Ok(())
    }

    /// Delete a follow-up.
    pub fn delete_followup(&self, mail_id: &str) -> Result<()> {
        self.conn()
            .execute("DELETE FROM followups WHERE mail_id = ?1", params![mail_id])?;
        Ok(())
    }

    /// Check if any unresolved followups have been replied to.
    /// Looks for messages from `expected_from` whose subject matches the original mail's subject.
    /// Returns a list of mail_ids that have been resolved.
    pub fn check_resolved_followups(&self, account_id: &str) -> Result<Vec<String>> {
        // Get all unresolved followups
        let mut stmt = self.conn().prepare(
            "SELECT f.mail_id, f.expected_from, m.subject
             FROM followups f
             JOIN messages m ON m.id = f.mail_id
             WHERE f.resolved = 0",
        )?;
        let pending: Vec<(String, String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut resolved = Vec::new();
        for (mail_id, expected_from, subject) in &pending {
            // Look for a reply: from expected_from, subject contains the original subject
            let reply_pattern = format!("%{}%", subject);
            let from_pattern = format!("%{}%", expected_from);
            let found: bool = self
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM messages
                     WHERE account_id = ?1 AND from_addr LIKE ?2 AND subject LIKE ?3",
                    params![account_id, from_pattern, reply_pattern],
                    |row| row.get::<_, i32>(0),
                )
                .unwrap_or(0)
                > 0;
            if found {
                self.resolve_followup(mail_id)?;
                resolved.push(mail_id.clone());
            }
        }
        Ok(resolved)
    }
}
