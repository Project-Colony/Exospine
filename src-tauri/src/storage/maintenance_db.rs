use anyhow::Result;
use rusqlite::params;

use super::db::Database;

impl Database {
    /// Run VACUUM to reclaim disk space and defragment the database.
    pub fn vacuum(&self) -> Result<()> {
        self.conn().execute_batch("VACUUM")?;
        // Record the timestamp so we don't vacuum again too soon
        let now = chrono::Utc::now().timestamp();
        self.conn()
            .execute_batch(&format!("PRAGMA user_version = {};", Self::SCHEMA_VERSION))?;
        // Store last vacuum time in a metadata table
        self.conn().execute_batch(
            "CREATE TABLE IF NOT EXISTS _metadata (key TEXT PRIMARY KEY, value TEXT);",
        )?;
        self.conn().execute(
            "INSERT OR REPLACE INTO _metadata (key, value) VALUES ('last_vacuum', ?1)",
            params![now.to_string()],
        )?;
        Ok(())
    }

    /// Check whether a VACUUM is due (more than 7 days since last one).
    pub fn should_vacuum(&self) -> bool {
        // Ensure metadata table exists
        let _ = self.conn().execute_batch(
            "CREATE TABLE IF NOT EXISTS _metadata (key TEXT PRIMARY KEY, value TEXT);",
        );
        let last: Option<String> = self
            .conn()
            .query_row(
                "SELECT value FROM _metadata WHERE key = 'last_vacuum'",
                [],
                |row| row.get(0),
            )
            .ok();
        match last {
            Some(ts_str) => {
                if let Ok(ts) = ts_str.parse::<i64>() {
                    let now = chrono::Utc::now().timestamp();
                    let week = 7 * 24 * 3600;
                    now - ts > week
                } else {
                    true
                }
            }
            None => true, // Never vacuumed
        }
    }

    /// Check and run VACUUM if needed (> 7 days since last run).
    /// Safe to call on every startup -- only vacuums when due.
    pub fn vacuum_if_needed(&self) -> Result<()> {
        if self.should_vacuum() {
            tracing::info!("VACUUM is due (> 7 days since last run), running...");
            self.vacuum()?;
            tracing::info!("VACUUM completed successfully");
        } else {
            tracing::debug!("VACUUM not needed yet");
        }
        Ok(())
    }

    /// Delete messages older than the specified number of days.
    /// Returns the number of deleted rows.
    pub fn cleanup_old_messages(&self, days: u32) -> Result<usize> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_str = cutoff.format("%Y-%m-%dT%H:%M:%S").to_string();
        let deleted = self.conn().execute(
            "DELETE FROM messages WHERE date < ?1",
            params![cutoff_str],
        )?;
        Ok(deleted)
    }

    /// Delete all data from all tables (messages, accounts, contacts, etc.)
    /// and run VACUUM. Used by the secure wipe feature.
    pub fn wipe_all(&self) -> Result<()> {
        self.conn().execute_batch(
            "DELETE FROM messages;
             DELETE FROM accounts;
             DELETE FROM contacts;
             DELETE FROM scheduled_emails;
             DELETE FROM draft_versions;
             DELETE FROM followups;
             DELETE FROM tasks;
             DELETE FROM notes;
             DELETE FROM _metadata;
             VACUUM;",
        )?;
        Ok(())
    }
}
