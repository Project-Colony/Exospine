use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::db::Database;

/// Email analytics data for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAnalytics {
    pub total_received: u64,
    pub total_sent: u64,
    pub by_day: Vec<(String, u64)>,
    pub top_senders: Vec<(String, u64)>,
    pub by_hour: Vec<u64>,
}

impl Database {
    /// Get email analytics for a given account.
    pub fn get_analytics(&self, account_id: &str) -> Result<EmailAnalytics> {
        let total_received: u64 = self.conn().query_row(
            "SELECT COUNT(*) FROM messages WHERE account_id = ?1 AND folder NOT IN ('Sent', '[Gmail]/Sent Mail', 'Drafts', '[Gmail]/Drafts')",
            params![account_id],
            |row| row.get(0),
        ).unwrap_or(0);

        let total_sent: u64 = self.conn().query_row(
            "SELECT COUNT(*) FROM messages WHERE account_id = ?1 AND folder IN ('Sent', '[Gmail]/Sent Mail')",
            params![account_id],
            |row| row.get(0),
        ).unwrap_or(0);

        let mut stmt = self.conn().prepare(
            "SELECT DATE(date) as d, COUNT(*) as c FROM messages
             WHERE account_id = ?1 AND date >= datetime('now', '-30 days')
             GROUP BY d ORDER BY d",
        )?;
        let by_day: Vec<(String, u64)> = stmt
            .query_map(params![account_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap_or_default();

        let mut stmt = self.conn().prepare(
            "SELECT from_addr, COUNT(*) as c FROM messages
             WHERE account_id = ?1 GROUP BY from_addr ORDER BY c DESC LIMIT 10",
        )?;
        let top_senders: Vec<(String, u64)> = stmt
            .query_map(params![account_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap_or_default();

        let mut stmt = self.conn().prepare(
            "SELECT CAST(strftime('%H', date) AS INTEGER) as h, COUNT(*) as c FROM messages
             WHERE account_id = ?1 GROUP BY h ORDER BY h",
        )?;
        let hour_map: Vec<(u32, u64)> = stmt
            .query_map(params![account_id], |row| {
                Ok((row.get::<_, u32>(0)?, row.get::<_, u64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap_or_default();

        let mut by_hour = vec![0u64; 24];
        for (h, c) in hour_map {
            if (h as usize) < 24 {
                by_hour[h as usize] = c;
            }
        }

        Ok(EmailAnalytics {
            total_received,
            total_sent,
            by_day,
            top_senders,
            by_hour,
        })
    }

    /// Find duplicate messages within a folder. Messages are considered duplicates
    /// if they have the same subject, sender, and date within 1 minute.
    /// Returns pairs of (original_id, duplicate_id).
    pub fn find_duplicates(&self, account_id: &str, folder: &str) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn().prepare(
            "SELECT m1.id, m2.id
             FROM messages m1
             INNER JOIN messages m2 ON
                 m1.subject = m2.subject
                 AND m1.from_addr = m2.from_addr
                 AND m1.account_id = m2.account_id
                 AND m1.folder = m2.folder
                 AND m1.id < m2.id
                 AND ABS(strftime('%s', m1.date) - strftime('%s', m2.date)) <= 60
             WHERE m1.account_id = ?1 AND m1.folder = ?2
             ORDER BY m1.date DESC
             LIMIT 500",
        )?;

        let pairs = stmt
            .query_map(params![account_id, folder], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(pairs)
    }

    /// Get the unsubscribe URL for a message.
    pub fn get_unsubscribe_url(&self, mail_id: &str) -> Result<Option<String>> {
        let result: Option<String> = self
            .conn()
            .query_row(
                "SELECT unsubscribe_url FROM messages WHERE id = ?1",
                params![mail_id],
                |row| row.get(0),
            )
            .ok()
            .flatten();
        Ok(result)
    }
}
