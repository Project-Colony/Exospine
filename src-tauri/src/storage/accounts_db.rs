use anyhow::Result;
use rusqlite::params;

use super::db::Database;
use crate::app_state::Account;

impl Database {
    pub fn save_account(&self, account: &Account) -> Result<()> {
        self.conn().execute(
            "INSERT OR REPLACE INTO accounts
                (id, name, email, imap_host, imap_port, smtp_host, smtp_port, username, use_tls)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                account.id,
                account.name,
                account.email,
                account.imap_host,
                account.imap_port,
                account.smtp_host,
                account.smtp_port,
                account.username,
                account.use_tls as i32,
            ],
        )?;
        Ok(())
    }

    pub fn load_accounts(&self) -> Result<Vec<Account>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, name, email, imap_host, imap_port, smtp_host, smtp_port, username, use_tls
             FROM accounts",
        )?;

        let accounts = stmt
            .query_map([], |row| {
                Ok(Account {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    email: row.get(2)?,
                    imap_host: row.get(3)?,
                    imap_port: row.get::<_, u32>(4)? as u16,
                    smtp_host: row.get(5)?,
                    smtp_port: row.get::<_, u32>(6)? as u16,
                    username: row.get(7)?,
                    use_tls: row.get::<_, i32>(8)? != 0,
                    auth_method: crate::app_state::AuthMethod::Basic,
                    folders: Vec::new(),
                    signature: String::new(),
                    signature_html: None,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(accounts)
    }

    pub fn delete_account(&self, id: &str) -> Result<()> {
        self.conn()
            .execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
        self.conn()
            .execute("DELETE FROM messages WHERE account_id = ?1", params![id])?;
        Ok(())
    }
}
