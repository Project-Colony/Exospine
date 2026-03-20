use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::state::{Account, MailEntry};

/// SQLite-backed local storage for accounts and cached messages.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open (or create) the database at the platform data directory.
    pub fn open() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("exospine");
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create data dir: {:?}", data_dir))?;

        let db_path = data_dir.join("mail.db");
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open database: {:?}", db_path))?;

        let db = Self { conn };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS accounts (
                id       TEXT PRIMARY KEY,
                name     TEXT NOT NULL,
                email    TEXT NOT NULL,
                imap_host TEXT NOT NULL,
                imap_port INTEGER NOT NULL,
                smtp_host TEXT NOT NULL,
                smtp_port INTEGER NOT NULL,
                username  TEXT NOT NULL,
                use_tls   INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS messages (
                id          TEXT PRIMARY KEY,
                account_id  TEXT NOT NULL,
                folder      TEXT NOT NULL,
                from_addr   TEXT NOT NULL,
                to_addrs    TEXT NOT NULL,
                subject     TEXT NOT NULL,
                date        TEXT NOT NULL,
                preview     TEXT NOT NULL DEFAULT '',
                body_text   TEXT NOT NULL DEFAULT '',
                body_html   TEXT,
                is_read     INTEGER NOT NULL DEFAULT 0,
                is_starred  INTEGER NOT NULL DEFAULT 0,
                has_attachments INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_messages_account_folder
                ON messages(account_id, folder);

            CREATE INDEX IF NOT EXISTS idx_messages_account_folder_date
                ON messages(account_id, folder, date DESC);",
        )?;
        Ok(())
    }

    // ── Accounts ────────────────────────────────────────────────────

    pub fn save_account(&self, account: &Account) -> Result<()> {
        self.conn.execute(
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
        let mut stmt = self.conn.prepare(
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
                    auth_method: crate::state::AuthMethod::Basic,
                    folders: Vec::new(),
                    signature: String::new(),
                    signature_html: None,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(accounts)
    }

    pub fn delete_account(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
        self.conn
            .execute("DELETE FROM messages WHERE account_id = ?1", params![id])?;
        Ok(())
    }

    // ── Messages ────────────────────────────────────────────────────

    pub fn save_messages(&self, messages: &[MailEntry]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO messages
                    (id, account_id, folder, from_addr, to_addrs, subject, date, preview, body_text, body_html, is_read, is_starred, has_attachments)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            )?;

            for m in messages {
                let to_json = serde_json::to_string(&m.to).unwrap_or_default();
                stmt.execute(params![
                    m.id,
                    m.account_id,
                    m.folder,
                    m.from,
                    to_json,
                    m.subject,
                    m.date.to_rfc3339(),
                    m.preview,
                    m.body_text,
                    m.body_html,
                    m.is_read as i32,
                    m.is_starred as i32,
                    m.has_attachments as i32,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn load_messages(&self, account_id: &str, folder: &str) -> Result<Vec<MailEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, account_id, folder, from_addr, to_addrs, subject, date, preview, body_text, body_html, is_read, is_starred, has_attachments
             FROM messages
             WHERE account_id = ?1 AND folder = ?2
             ORDER BY date DESC",
        )?;

        let entries = stmt
            .query_map(params![account_id, folder], |row| {
                let to_json: String = row.get(4)?;
                let to: Vec<String> =
                    serde_json::from_str(&to_json).unwrap_or_default();
                let date_str: String = row.get(6)?;
                let date = chrono::DateTime::parse_from_rfc3339(&date_str)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());

                Ok(MailEntry {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    folder: row.get(2)?,
                    from: row.get(3)?,
                    to,
                    subject: row.get(5)?,
                    date,
                    preview: row.get(7)?,
                    body_text: row.get(8)?,
                    body_html: row.get(9)?,
                    is_read: row.get::<_, i32>(10)? != 0,
                    is_starred: row.get::<_, i32>(11)? != 0,
                    has_attachments: row.get::<_, i32>(12)? != 0,
                    categories: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    pub fn update_read_status(&self, id: &str, is_read: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET is_read = ?1 WHERE id = ?2",
            params![is_read as i32, id],
        )?;
        Ok(())
    }

    pub fn update_star_status(&self, id: &str, is_starred: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET is_starred = ?1 WHERE id = ?2",
            params![is_starred as i32, id],
        )?;
        Ok(())
    }

    pub fn delete_message(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM messages WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn search_messages(&self, query: &str) -> Result<Vec<MailEntry>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT id, account_id, folder, from_addr, to_addrs, subject, date, preview, body_text, body_html, is_read, is_starred, has_attachments
             FROM messages
             WHERE subject LIKE ?1 OR from_addr LIKE ?1 OR body_text LIKE ?1
             ORDER BY date DESC
             LIMIT 100",
        )?;

        let entries = stmt
            .query_map(params![pattern], |row| {
                let to_json: String = row.get(4)?;
                let to: Vec<String> =
                    serde_json::from_str(&to_json).unwrap_or_default();
                let date_str: String = row.get(6)?;
                let date = chrono::DateTime::parse_from_rfc3339(&date_str)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());

                Ok(MailEntry {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    folder: row.get(2)?,
                    from: row.get(3)?,
                    to,
                    subject: row.get(5)?,
                    date,
                    preview: row.get(7)?,
                    body_text: row.get(8)?,
                    body_html: row.get(9)?,
                    is_read: row.get::<_, i32>(10)? != 0,
                    is_starred: row.get::<_, i32>(11)? != 0,
                    has_attachments: row.get::<_, i32>(12)? != 0,
                    categories: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entries)
    }
}
