use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::db::Database;
use crate::app_state::ComposeDraft;

/// A scheduled email waiting to be sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledEmail {
    pub id: String,
    pub account_id: String,
    pub to_addr: String,
    pub cc: String,
    pub bcc: String,
    pub subject: String,
    pub body: String,
    pub scheduled_time: String,
}

impl Database {
    pub fn save_scheduled_email(&self, draft: &ComposeDraft, scheduled_time: &str) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn().execute(
            "INSERT INTO scheduled_emails (id, account_id, to_addr, cc, bcc, subject, body, scheduled_time)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                draft.account_id,
                draft.to,
                draft.cc,
                draft.bcc,
                draft.subject,
                draft.body,
                scheduled_time,
            ],
        )?;
        Ok(())
    }

    pub fn get_scheduled_emails(&self) -> Result<Vec<ScheduledEmail>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, account_id, to_addr, cc, bcc, subject, body, scheduled_time
             FROM scheduled_emails
             ORDER BY scheduled_time ASC",
        )?;
        let results = stmt
            .query_map([], |row| {
                Ok(ScheduledEmail {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    to_addr: row.get(2)?,
                    cc: row.get(3)?,
                    bcc: row.get(4)?,
                    subject: row.get(5)?,
                    body: row.get(6)?,
                    scheduled_time: row.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(results)
    }

    pub fn get_due_scheduled_emails(&self) -> Result<Vec<ScheduledEmail>> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = self.conn().prepare(
            "SELECT id, account_id, to_addr, cc, bcc, subject, body, scheduled_time
             FROM scheduled_emails
             WHERE scheduled_time <= ?1
             ORDER BY scheduled_time ASC",
        )?;
        let results = stmt
            .query_map(params![now], |row| {
                Ok(ScheduledEmail {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    to_addr: row.get(2)?,
                    cc: row.get(3)?,
                    bcc: row.get(4)?,
                    subject: row.get(5)?,
                    body: row.get(6)?,
                    scheduled_time: row.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(results)
    }

    pub fn delete_scheduled_email(&self, id: &str) -> Result<()> {
        self.conn()
            .execute("DELETE FROM scheduled_emails WHERE id = ?1", params![id])?;
        Ok(())
    }
}
