use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::db::Database;

/// A saved version of a draft for version history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftVersion {
    pub draft_id: String,
    pub version: i32,
    pub timestamp: String,
    pub body: String,
    pub body_html: Option<String>,
    pub subject: String,
}

impl Database {
    /// Save a draft version. Keeps only the last 5 versions per draft_id.
    pub fn save_draft_version(
        &self,
        draft_id: &str,
        subject: &str,
        body: &str,
        body_html: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let max_version: i32 = self
            .conn()
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM draft_versions WHERE draft_id = ?1",
                params![draft_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let new_version = max_version + 1;
        self.conn().execute(
            "INSERT INTO draft_versions (draft_id, version, timestamp, body, body_html, subject)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![draft_id, new_version, now, body, body_html, subject],
        )?;
        // Prune: keep only the last 5 versions
        self.conn().execute(
            "DELETE FROM draft_versions WHERE draft_id = ?1 AND version <= ?2 - 5",
            params![draft_id, new_version],
        )?;
        Ok(())
    }

    /// Load all versions of a draft, ordered by version descending.
    pub fn load_draft_versions(&self, draft_id: &str) -> Result<Vec<DraftVersion>> {
        let mut stmt = self.conn().prepare(
            "SELECT draft_id, version, timestamp, body, body_html, subject
             FROM draft_versions WHERE draft_id = ?1 ORDER BY version DESC",
        )?;
        let versions = stmt
            .query_map(params![draft_id], |row| {
                Ok(DraftVersion {
                    draft_id: row.get(0)?,
                    version: row.get(1)?,
                    timestamp: row.get(2)?,
                    body: row.get(3)?,
                    body_html: row.get(4)?,
                    subject: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(versions)
    }

    /// Delete all versions of a draft.
    pub fn delete_draft_versions(&self, draft_id: &str) -> Result<()> {
        self.conn()
            .execute("DELETE FROM draft_versions WHERE draft_id = ?1", params![draft_id])?;
        Ok(())
    }
}
