use anyhow::Result;
use rusqlite::params;

use super::db::{parse_address, Database};
use crate::app_state::MailEntry;

/// (email, name, frequency, phone, company, notes)
pub type ContactRow = (String, String, u32, String, String, String);

impl Database {
    /// Upsert a contact: insert or update, incrementing frequency.
    pub fn upsert_contact(&self, email: &str, name: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn().execute(
            "INSERT INTO contacts (email, name, last_used, frequency)
             VALUES (?1, ?2, ?3, 1)
             ON CONFLICT(email) DO UPDATE SET
               name = CASE WHEN ?2 != '' THEN ?2 ELSE contacts.name END,
               last_used = ?3,
               frequency = contacts.frequency + 1",
            params![email, name, now],
        )?;
        Ok(())
    }

    /// Search contacts by name or email, ordered by frequency descending.
    pub fn search_contacts(&self, query: &str) -> Result<Vec<(String, String, u32)>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn().prepare(
            "SELECT email, name, frequency FROM contacts
             WHERE email LIKE ?1 OR name LIKE ?1
             ORDER BY frequency DESC
             LIMIT 20",
        )?;
        let results = stmt
            .query_map(params![pattern], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u32>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(results)
    }

    /// Get all contacts, ordered by name.
    pub fn get_all_contacts(&self) -> Result<Vec<ContactRow>> {
        let mut stmt = self.conn().prepare(
            "SELECT email, name, frequency, phone, company, notes FROM contacts
             ORDER BY name ASC, email ASC",
        )?;
        let results = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u32>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(results)
    }

    /// Update a contact's details.
    pub fn update_contact(
        &self,
        email: &str,
        name: &str,
        phone: &str,
        company: &str,
        notes: &str,
    ) -> Result<()> {
        self.conn().execute(
            "UPDATE contacts SET name = ?1, phone = ?2, company = ?3, notes = ?4 WHERE email = ?5",
            params![name, phone, company, notes, email],
        )?;
        Ok(())
    }

    /// Delete a contact by email.
    pub fn delete_contact(&self, email: &str) -> Result<()> {
        self.conn()
            .execute("DELETE FROM contacts WHERE email = ?1", params![email])?;
        Ok(())
    }

    /// Extract and upsert contacts from a list of mail entries.
    pub fn extract_contacts_from_mails(&self, mails: &[MailEntry]) -> Result<()> {
        let tx = self.conn().unchecked_transaction()?;
        {
            let now = chrono::Utc::now().to_rfc3339();
            let mut stmt = tx.prepare(
                "INSERT INTO contacts (email, name, last_used, frequency)
                 VALUES (?1, ?2, ?3, 1)
                 ON CONFLICT(email) DO UPDATE SET
                   name = CASE WHEN ?2 != '' THEN ?2 ELSE contacts.name END,
                   last_used = ?3,
                   frequency = contacts.frequency + 1",
            )?;

            for mail in mails {
                let (from_name, from_email) = parse_address(&mail.from);
                if !from_email.is_empty() {
                    stmt.execute(params![from_email, from_name, now])?;
                }
                for addr in &mail.to {
                    let (to_name, to_email) = parse_address(addr);
                    if !to_email.is_empty() {
                        stmt.execute(params![to_email, to_name, now])?;
                    }
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Count total unread messages across all accounts' INBOX.
    pub fn count_unread(&self) -> Result<u32> {
        let count: u32 = self.conn().query_row(
            "SELECT COUNT(*) FROM messages WHERE is_read = 0 AND folder = 'INBOX'",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}
