use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::app_state::{Account, ComposeDraft, MailEntry};

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

/// SQLite-backed local storage for accounts and cached messages.
pub struct Database {
    conn: Connection,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database").finish_non_exhaustive()
    }
}

impl Database {
    /// Open an in-memory database (for testing).
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("Failed to open in-memory database")?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        let db = Self { conn };
        db.init_tables()?;
        db.migrate()?;
        Ok(db)
    }

    pub fn open() -> Result<Self> {
        let data_dir = crate::config::data_dir();
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create data dir: {:?}", data_dir))?;

        let db_path = data_dir.join("mail.db");
        tracing::info!("Opening database at {}", db_path.display());
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open database: {:?}", db_path))?;

        // Enable WAL mode for better concurrent read/write performance
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;

        let db = Self { conn };
        db.init_tables()?;
        db.migrate()?;
        tracing::info!("Database initialized successfully");
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
                has_attachments INTEGER NOT NULL DEFAULT 0,
                uid         INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_messages_account_folder
                ON messages(account_id, folder);

            CREATE INDEX IF NOT EXISTS idx_messages_account_folder_date
                ON messages(account_id, folder, date DESC);

            CREATE TABLE IF NOT EXISTS contacts (
                email TEXT PRIMARY KEY,
                name TEXT NOT NULL DEFAULT '',
                last_used TEXT NOT NULL DEFAULT '',
                frequency INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS scheduled_emails (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                to_addr TEXT NOT NULL,
                cc TEXT NOT NULL DEFAULT '',
                bcc TEXT NOT NULL DEFAULT '',
                subject TEXT NOT NULL DEFAULT '',
                body TEXT NOT NULL DEFAULT '',
                scheduled_time TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS draft_versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                draft_id TEXT NOT NULL,
                version INTEGER NOT NULL,
                timestamp TEXT NOT NULL,
                body TEXT NOT NULL DEFAULT '',
                body_html TEXT,
                subject TEXT NOT NULL DEFAULT '',
                UNIQUE(draft_id, version)
            );

            CREATE INDEX IF NOT EXISTS idx_draft_versions_draft_id
                ON draft_versions(draft_id, version DESC);",
        )?;
        Ok(())
    }

    /// Current schema version. Bump this when adding new migrations.
    const SCHEMA_VERSION: u32 = 6;

    fn migrate(&self) -> Result<()> {
        let current_version: u32 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;

        if current_version >= Self::SCHEMA_VERSION {
            return Ok(());
        }

        // Migration 1: add uid column
        if current_version < 1 {
            let has_uid: bool = self
                .conn
                .prepare("SELECT uid FROM messages LIMIT 0")
                .is_ok();
            if !has_uid {
                self.conn
                    .execute_batch("ALTER TABLE messages ADD COLUMN uid INTEGER NOT NULL DEFAULT 0")?;
            }
        }

        // Migration 2: add threading columns
        if current_version < 2 {
            let has_thread_id: bool = self
                .conn
                .prepare("SELECT thread_id FROM messages LIMIT 0")
                .is_ok();
            if !has_thread_id {
                self.conn.execute_batch(
                    "ALTER TABLE messages ADD COLUMN message_id TEXT NOT NULL DEFAULT '';
                     ALTER TABLE messages ADD COLUMN in_reply_to TEXT;
                     ALTER TABLE messages ADD COLUMN refs TEXT NOT NULL DEFAULT '[]';
                     ALTER TABLE messages ADD COLUMN thread_id TEXT NOT NULL DEFAULT '';
                     ALTER TABLE messages ADD COLUMN spam_score REAL NOT NULL DEFAULT 0.0;
                     ALTER TABLE messages ADD COLUMN categories TEXT NOT NULL DEFAULT '[]';",
                )?;
            }
        }

        // Migration 3: add is_pinned column
        if current_version < 3 {
            let has_pinned: bool = self
                .conn
                .prepare("SELECT is_pinned FROM messages LIMIT 0")
                .is_ok();
            if !has_pinned {
                self.conn.execute_batch(
                    "ALTER TABLE messages ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0",
                )?;
            }
        }

        // Migration 4: add snoozed_until column
        if current_version < 4 {
            let has_snoozed: bool = self
                .conn
                .prepare("SELECT snoozed_until FROM messages LIMIT 0")
                .is_ok();
            if !has_snoozed {
                self.conn.execute_batch(
                    "ALTER TABLE messages ADD COLUMN snoozed_until TEXT",
                )?;
            }
        }

        // Migration 5: add importance, read_receipt, flag_due_date columns
        if current_version < 5 {
            let has_importance: bool = self
                .conn
                .prepare("SELECT importance FROM messages LIMIT 0")
                .is_ok();
            if !has_importance {
                self.conn.execute_batch(
                    "ALTER TABLE messages ADD COLUMN importance TEXT NOT NULL DEFAULT 'normal';
                     ALTER TABLE messages ADD COLUMN read_receipt_requested INTEGER NOT NULL DEFAULT 0;
                     ALTER TABLE messages ADD COLUMN read_receipt_to TEXT;
                     ALTER TABLE messages ADD COLUMN flag_due_date TEXT;",
                )?;
            }
        }

        // Migration 6: add phone, company, notes columns to contacts
        if current_version < 6 {
            let has_phone: bool = self
                .conn
                .prepare("SELECT phone FROM contacts LIMIT 0")
                .is_ok();
            if !has_phone {
                self.conn.execute_batch(
                    "ALTER TABLE contacts ADD COLUMN phone TEXT NOT NULL DEFAULT '';
                     ALTER TABLE contacts ADD COLUMN company TEXT NOT NULL DEFAULT '';
                     ALTER TABLE contacts ADD COLUMN notes TEXT NOT NULL DEFAULT '';",
                )?;
            }
        }

        // Update schema version to current
        self.conn.execute_batch(&format!(
            "PRAGMA user_version = {};",
            Self::SCHEMA_VERSION
        ))?;

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
        self.conn
            .execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
        self.conn
            .execute("DELETE FROM messages WHERE account_id = ?1", params![id])?;
        Ok(())
    }

    // ── Messages ────────────────────────────────────────────────────

    pub fn save_messages(&self, messages: &[MailEntry]) -> Result<()> {
        // Split into chunks of 500 to prevent long DB locks.
        const CHUNK_SIZE: usize = 500;
        for chunk in messages.chunks(CHUNK_SIZE) {
            self.save_messages_chunk(chunk)?;
        }
        Ok(())
    }

    /// Save a single chunk of messages in one transaction.
    fn save_messages_chunk(&self, messages: &[MailEntry]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO messages
                    (id, account_id, folder, from_addr, to_addrs, subject, date, preview, body_text, body_html, is_read, is_starred, has_attachments, uid, message_id, in_reply_to, refs, thread_id, spam_score, categories, is_pinned, snoozed_until, importance, read_receipt_requested, read_receipt_to, flag_due_date)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
            )?;

            for m in messages {
                let to_json = serde_json::to_string(&m.to).unwrap_or_default();
                let refs_json = serde_json::to_string(&m.references).unwrap_or_default();
                let cats_json = serde_json::to_string(&m.categories).unwrap_or_default();
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
                    m.uid,
                    m.message_id,
                    m.in_reply_to,
                    refs_json,
                    m.thread_id,
                    m.spam_score,
                    cats_json,
                    m.is_pinned as i32,
                    m.snoozed_until,
                    m.importance,
                    m.read_receipt_requested as i32,
                    m.read_receipt_to,
                    m.flag_due_date,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn load_messages(&self, account_id: &str, folder: &str) -> Result<Vec<MailEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, account_id, folder, from_addr, to_addrs, subject, date, preview, body_text, body_html, is_read, is_starred, has_attachments, uid, message_id, in_reply_to, refs, thread_id, spam_score, categories, is_pinned, snoozed_until, importance, read_receipt_requested, read_receipt_to, flag_due_date
             FROM messages
             WHERE account_id = ?1 AND folder = ?2
             ORDER BY date DESC",
        )?;

        let entries = stmt
            .query_map(params![account_id, folder], |row| {
                row_to_mail_entry_full(row)
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
        self.secure_delete_message(id)
    }

    /// Securely delete a message by overwriting sensitive fields before removal.
    /// This prevents recovery of email content from the SQLite database file.
    pub fn secure_delete_message(&self, id: &str) -> Result<()> {
        // Step 1: Overwrite sensitive fields with empty/placeholder data
        self.conn.execute(
            "UPDATE messages SET body_text = '', body_html = NULL, subject = '[deleted]', from_addr = '', to_addrs = '[]', preview = '' WHERE id = ?1",
            params![id],
        )?;
        // Step 2: Actually delete the row
        self.conn
            .execute("DELETE FROM messages WHERE id = ?1", params![id])?;
        // Note: VACUUM is intentionally not called here as it is expensive.
        // It can be triggered separately during maintenance.
        Ok(())
    }

    pub fn load_message_headers_page(
        &self,
        account_id: &str,
        folder: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MailEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, account_id, folder, from_addr, to_addrs, subject, date, preview, is_read, is_starred, has_attachments, uid, message_id, in_reply_to, refs, thread_id, spam_score, categories, is_pinned, snoozed_until, importance, read_receipt_requested, read_receipt_to, flag_due_date
             FROM messages
             WHERE account_id = ?1 AND folder = ?2
             ORDER BY date DESC
             LIMIT ?3 OFFSET ?4",
        )?;

        let entries = stmt
            .query_map(params![account_id, folder, limit, offset], |row| {
                row_to_mail_entry_headers(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    /// Load account_id, folder, and uid for a message (used for IMAP fallback).
    pub fn load_message_meta(&self, id: &str) -> Result<(String, String, u32)> {
        let mut stmt = self
            .conn
            .prepare("SELECT account_id, folder, uid FROM messages WHERE id = ?1")?;

        let result = stmt.query_row(params![id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u32>(2)?,
            ))
        })?;

        Ok(result)
    }

    pub fn load_message_body(&self, id: &str) -> Result<(String, Option<String>)> {
        let mut stmt = self
            .conn
            .prepare("SELECT body_text, body_html FROM messages WHERE id = ?1")?;

        let result = stmt.query_row(params![id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;

        Ok(result)
    }

    pub fn max_uid(&self, account_id: &str, folder: &str) -> Result<u32> {
        let uid: u32 = self.conn.query_row(
            "SELECT COALESCE(MAX(uid), 0) FROM messages WHERE account_id = ?1 AND folder = ?2",
            params![account_id, folder],
            |row| row.get(0),
        )?;
        Ok(uid)
    }

    /// Return all cached UIDs for a given account + folder.
    pub fn cached_uids(&self, account_id: &str, folder: &str) -> Result<Vec<u32>> {
        let mut stmt = self.conn.prepare(
            "SELECT uid FROM messages WHERE account_id = ?1 AND folder = ?2 AND uid > 0",
        )?;
        let uids = stmt
            .query_map(params![account_id, folder], |row| row.get::<_, u32>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(uids)
    }

    pub fn message_count(&self, account_id: &str, folder: &str) -> Result<u32> {
        let count: u32 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE account_id = ?1 AND folder = ?2",
            params![account_id, folder],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn search_messages_in_folder(&self, query: &str, account_id: &str, folder: &str) -> Result<Vec<MailEntry>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT id, account_id, folder, from_addr, to_addrs, subject, date, preview, body_text, body_html, is_read, is_starred, has_attachments, uid, message_id, in_reply_to, refs, thread_id, spam_score, categories, is_pinned, snoozed_until
             FROM messages
             WHERE account_id = ?1 AND folder = ?2 AND (subject LIKE ?3 OR from_addr LIKE ?3 OR preview LIKE ?3)
             ORDER BY date DESC
             LIMIT 100",
        )?;

        let entries = stmt
            .query_map(params![account_id, folder, pattern], |row| {
                row_to_mail_entry_full(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    pub fn search_messages(&self, query: &str) -> Result<Vec<MailEntry>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT id, account_id, folder, from_addr, to_addrs, subject, date, preview, body_text, body_html, is_read, is_starred, has_attachments, uid, message_id, in_reply_to, refs, thread_id, spam_score, categories, is_pinned, snoozed_until
             FROM messages
             WHERE subject LIKE ?1 OR from_addr LIKE ?1 OR body_text LIKE ?1
             ORDER BY date DESC
             LIMIT 100",
        )?;

        let entries = stmt
            .query_map(params![pattern], |row| {
                row_to_mail_entry_full(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    /// Execute a dynamic SQL search query with positional string parameters.
    /// The SQL must select the same columns as `row_to_mail_entry_full`.
    pub fn search_messages_dynamic(&self, sql: &str, params: &[String]) -> Result<Vec<MailEntry>> {
        let mut stmt = self.conn.prepare(sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();
        let entries = stmt
            .query_map(param_refs.as_slice(), |row| row_to_mail_entry_full(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    // ── Sweep sender ─────────────────────────────────────────────────

    /// Delete all messages from a given sender in a folder. Returns UIDs of deleted messages.
    pub fn sweep_sender(&self, account_id: &str, folder: &str, sender_email: &str) -> Result<Vec<String>> {
        let pattern = format!("%{}%", sender_email);
        let mut stmt = self.conn.prepare(
            "SELECT id, uid FROM messages WHERE account_id = ?1 AND folder = ?2 AND from_addr LIKE ?3",
        )?;
        let entries: Vec<(String, String)> = stmt
            .query_map(params![account_id, folder, pattern], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let uids: Vec<String> = entries.iter().map(|(_, uid)| uid.clone()).collect();

        for (id, _) in &entries {
            self.delete_message(id)?;
        }

        Ok(uids)
    }

    // ── Categories ──────────────────────────────────────────────────

    /// Add a category to a message.
    pub fn add_category(&self, mail_id: &str, category: &str) -> Result<()> {
        let current: String = self.conn.query_row(
            "SELECT categories FROM messages WHERE id = ?1",
            params![mail_id],
            |row| row.get(0),
        ).unwrap_or_else(|_| "[]".to_string());

        let mut cats: Vec<String> = serde_json::from_str(&current).unwrap_or_default();
        if !cats.iter().any(|c| c == category) {
            cats.push(category.to_string());
        }
        let json = serde_json::to_string(&cats).unwrap_or_default();
        self.conn.execute(
            "UPDATE messages SET categories = ?1 WHERE id = ?2",
            params![json, mail_id],
        )?;
        Ok(())
    }

    /// Remove a category from a message.
    pub fn remove_category(&self, mail_id: &str, category: &str) -> Result<()> {
        let current: String = self.conn.query_row(
            "SELECT categories FROM messages WHERE id = ?1",
            params![mail_id],
            |row| row.get(0),
        ).unwrap_or_else(|_| "[]".to_string());

        let mut cats: Vec<String> = serde_json::from_str(&current).unwrap_or_default();
        cats.retain(|c| c != category);
        let json = serde_json::to_string(&cats).unwrap_or_default();
        self.conn.execute(
            "UPDATE messages SET categories = ?1 WHERE id = ?2",
            params![json, mail_id],
        )?;
        Ok(())
    }

    // ── Contacts ─────────────────────────────────────────────────────

    /// Upsert a contact: insert or update, incrementing frequency.
    pub fn upsert_contact(&self, email: &str, name: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
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
        let mut stmt = self.conn.prepare(
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
    pub fn get_all_contacts(&self) -> Result<Vec<(String, String, u32, String, String, String)>> {
        let mut stmt = self.conn.prepare(
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
        self.conn.execute(
            "UPDATE contacts SET name = ?1, phone = ?2, company = ?3, notes = ?4 WHERE email = ?5",
            params![name, phone, company, notes, email],
        )?;
        Ok(())
    }

    /// Delete a contact by email.
    pub fn delete_contact(&self, email: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM contacts WHERE email = ?1", params![email])?;
        Ok(())
    }

    /// Count total unread messages across all accounts' INBOX.
    pub fn count_unread(&self) -> Result<u32> {
        let count: u32 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE is_read = 0 AND folder = 'INBOX'",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Extract and upsert contacts from a list of mail entries.
    pub fn extract_contacts_from_mails(&self, mails: &[MailEntry]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
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

    // ── Pin ──────────────────────────────────────────────────────────

    pub fn update_pinned_status(&self, id: &str, is_pinned: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET is_pinned = ?1 WHERE id = ?2",
            params![is_pinned as i32, id],
        )?;
        Ok(())
    }

    // ── Snooze ───────────────────────────────────────────────────────

    pub fn update_snoozed_until(&self, id: &str, until: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET snoozed_until = ?1 WHERE id = ?2",
            params![until, id],
        )?;
        Ok(())
    }

    /// Get IDs of snoozed messages that are due (snoozed_until <= now).
    pub fn get_due_snoozed_ids(&self) -> Result<Vec<String>> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = self.conn.prepare(
            "SELECT id FROM messages WHERE snoozed_until IS NOT NULL AND snoozed_until <= ?1",
        )?;
        let ids = stmt
            .query_map(params![now], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    // ── Flag follow-up ─────────────────────────────────────────────

    pub fn update_flag_due_date(&self, id: &str, due_date: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET flag_due_date = ?1 WHERE id = ?2",
            params![due_date, id],
        )?;
        Ok(())
    }

    // ── Scheduled Emails ─────────────────────────────────────────────

    pub fn save_scheduled_email(&self, draft: &ComposeDraft, scheduled_time: &str) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
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
        let mut stmt = self.conn.prepare(
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
        let mut stmt = self.conn.prepare(
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
        self.conn
            .execute("DELETE FROM scheduled_emails WHERE id = ?1", params![id])?;
        Ok(())
    }

    // ── Draft Versioning ────────────────────────────────────────────

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
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM draft_versions WHERE draft_id = ?1",
                params![draft_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let new_version = max_version + 1;
        self.conn.execute(
            "INSERT INTO draft_versions (draft_id, version, timestamp, body, body_html, subject)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![draft_id, new_version, now, body, body_html, subject],
        )?;
        // Prune: keep only the last 5 versions
        self.conn.execute(
            "DELETE FROM draft_versions WHERE draft_id = ?1 AND version <= ?2 - 5",
            params![draft_id, new_version],
        )?;
        Ok(())
    }

    /// Load all versions of a draft, ordered by version descending.
    pub fn load_draft_versions(&self, draft_id: &str) -> Result<Vec<DraftVersion>> {
        let mut stmt = self.conn.prepare(
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
        self.conn
            .execute("DELETE FROM draft_versions WHERE draft_id = ?1", params![draft_id])?;
        Ok(())
    }

    // ── Email Analytics ─────────────────────────────────────────────

    /// Get email analytics for a given account.
    pub fn get_analytics(&self, account_id: &str) -> Result<EmailAnalytics> {
        let total_received: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE account_id = ?1 AND folder NOT IN ('Sent', '[Gmail]/Sent Mail', 'Drafts', '[Gmail]/Drafts')",
            params![account_id],
            |row| row.get(0),
        ).unwrap_or(0);

        let total_sent: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE account_id = ?1 AND folder IN ('Sent', '[Gmail]/Sent Mail')",
            params![account_id],
            |row| row.get(0),
        ).unwrap_or(0);

        let mut stmt = self.conn.prepare(
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

        let mut stmt = self.conn.prepare(
            "SELECT from_addr, COUNT(*) as c FROM messages
             WHERE account_id = ?1 GROUP BY from_addr ORDER BY c DESC LIMIT 10",
        )?;
        let top_senders: Vec<(String, u64)> = stmt
            .query_map(params![account_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap_or_default();

        let mut stmt = self.conn.prepare(
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

    /// Delete messages older than the specified number of days.
    /// Returns the number of deleted rows.
    pub fn cleanup_old_messages(&self, days: u32) -> Result<usize> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_str = cutoff.format("%Y-%m-%dT%H:%M:%S").to_string();
        let deleted = self.conn.execute(
            "DELETE FROM messages WHERE date < ?1",
            params![cutoff_str],
        )?;
        Ok(deleted)
    }
}

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

/// Email analytics data for the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAnalytics {
    pub total_received: u64,
    pub total_sent: u64,
    pub by_day: Vec<(String, u64)>,
    pub top_senders: Vec<(String, u64)>,
    pub by_hour: Vec<u64>,
}

/// Parse an email address string like "Name <email@example.com>" into (name, email).
fn parse_address(addr: &str) -> (String, String) {
    let addr = addr.trim();
    if let Some(start) = addr.find('<') {
        if let Some(end) = addr.find('>') {
            let email = addr[start + 1..end].trim().to_lowercase();
            let name = addr[..start].trim().trim_matches('"').to_string();
            return (name, email);
        }
    }
    if addr.contains('@') {
        return (String::new(), addr.to_lowercase());
    }
    (String::new(), String::new())
}

// ── Helper functions for row-to-MailEntry conversion ────────────────

/// Parse extra threading/spam/category columns from a row.
struct ExtraCols {
    message_id: String,
    in_reply_to: Option<String>,
    references: Vec<String>,
    thread_id: String,
    spam_score: f32,
    categories: Vec<String>,
    is_pinned: bool,
    snoozed_until: Option<String>,
    importance: String,
    read_receipt_requested: bool,
    read_receipt_to: Option<String>,
    flag_due_date: Option<String>,
}

fn parse_extra_columns(row: &rusqlite::Row, base: usize) -> ExtraCols {
    let message_id: String = row.get::<_, String>(base).unwrap_or_default();
    let in_reply_to: Option<String> = row.get::<_, Option<String>>(base + 1).unwrap_or(None);
    let refs_json: String = row.get::<_, String>(base + 2).unwrap_or_else(|_| "[]".to_string());
    let references: Vec<String> = serde_json::from_str(&refs_json).unwrap_or_default();
    let thread_id: String = row.get::<_, String>(base + 3).unwrap_or_default();
    let spam_score: f32 = row.get::<_, f64>(base + 4).unwrap_or(0.0) as f32;
    let cats_json: String = row.get::<_, String>(base + 5).unwrap_or_else(|_| "[]".to_string());
    let categories: Vec<String> = serde_json::from_str(&cats_json).unwrap_or_default();
    let is_pinned: bool = row.get::<_, i32>(base + 6).unwrap_or(0) != 0;
    let snoozed_until: Option<String> = row.get::<_, Option<String>>(base + 7).unwrap_or(None);
    let importance: String = row.get::<_, String>(base + 8).unwrap_or_else(|_| "normal".to_string());
    let read_receipt_requested: bool = row.get::<_, i32>(base + 9).unwrap_or(0) != 0;
    let read_receipt_to: Option<String> = row.get::<_, Option<String>>(base + 10).unwrap_or(None);
    let flag_due_date: Option<String> = row.get::<_, Option<String>>(base + 11).unwrap_or(None);
    ExtraCols { message_id, in_reply_to, references, thread_id, spam_score, categories, is_pinned, snoozed_until, importance, read_receipt_requested, read_receipt_to, flag_due_date }
}

/// Convert a full row (with body_text, body_html) to MailEntry.
fn row_to_mail_entry_full(row: &rusqlite::Row) -> rusqlite::Result<MailEntry> {
    let to_json: String = row.get(4)?;
    let to: Vec<String> = serde_json::from_str(&to_json).unwrap_or_default();
    let date_str: String = row.get(6)?;
    let date = chrono::DateTime::parse_from_rfc3339(&date_str)
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    let extra = parse_extra_columns(row, 14);

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
        uid: row.get::<_, u32>(13).unwrap_or(0),
        categories: extra.categories,
        attachment_meta: Vec::new(),
        phishing_warnings: Vec::new(),
        auth_status: None,
        sender_warnings: Vec::new(),
        message_id: extra.message_id,
        in_reply_to: extra.in_reply_to,
        references: extra.references,
        thread_id: extra.thread_id,
        spam_score: extra.spam_score,
        is_pinned: extra.is_pinned,
        snoozed_until: extra.snoozed_until,
        importance: extra.importance,
        read_receipt_requested: extra.read_receipt_requested,
        read_receipt_to: extra.read_receipt_to,
        flag_due_date: extra.flag_due_date,
    })
}

/// Convert a header-only row (no body_text/body_html) to MailEntry.
fn row_to_mail_entry_headers(row: &rusqlite::Row) -> rusqlite::Result<MailEntry> {
    let to_json: String = row.get(4)?;
    let to: Vec<String> = serde_json::from_str(&to_json).unwrap_or_default();
    let date_str: String = row.get(6)?;
    let date = chrono::DateTime::parse_from_rfc3339(&date_str)
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    let extra = parse_extra_columns(row, 12);

    Ok(MailEntry {
        id: row.get(0)?,
        account_id: row.get(1)?,
        folder: row.get(2)?,
        from: row.get(3)?,
        to,
        subject: row.get(5)?,
        date,
        preview: row.get(7)?,
        body_text: String::new(),
        body_html: None,
        is_read: row.get::<_, i32>(8)? != 0,
        is_starred: row.get::<_, i32>(9)? != 0,
        has_attachments: row.get::<_, i32>(10)? != 0,
        uid: row.get::<_, u32>(11).unwrap_or(0),
        categories: extra.categories,
        attachment_meta: Vec::new(),
        phishing_warnings: Vec::new(),
        auth_status: None,
        sender_warnings: Vec::new(),
        message_id: extra.message_id,
        in_reply_to: extra.in_reply_to,
        references: extra.references,
        thread_id: extra.thread_id,
        spam_score: extra.spam_score,
        is_pinned: extra.is_pinned,
        snoozed_until: extra.snoozed_until,
        importance: extra.importance,
        read_receipt_requested: extra.read_receipt_requested,
        read_receipt_to: extra.read_receipt_to,
        flag_due_date: extra.flag_due_date,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::{Account, MailEntry};

    fn test_db() -> Database {
        Database::open_in_memory().expect("Failed to open in-memory DB")
    }

    fn make_account() -> Account {
        Account {
            id: "test-acc-1".to_string(),
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            imap_host: "imap.example.com".to_string(),
            imap_port: 993,
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 465,
            username: "test@example.com".to_string(),
            use_tls: true,
            ..Default::default()
        }
    }

    fn make_mail(id: &str, subject: &str, folder: &str) -> MailEntry {
        MailEntry {
            id: id.to_string(),
            uid: 1,
            from: "sender@example.com".to_string(),
            to: vec!["recipient@example.com".to_string()],
            subject: subject.to_string(),
            date: chrono::Utc::now(),
            preview: "Preview text".to_string(),
            body_text: "Full body text".to_string(),
            body_html: Some("<p>Full body</p>".to_string()),
            is_read: false,
            is_starred: false,
            has_attachments: false,
            folder: folder.to_string(),
            account_id: "test-acc-1".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_save_and_load_account() {
        let db = test_db();
        let account = make_account();
        db.save_account(&account).unwrap();
        let loaded = db.load_accounts().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "test-acc-1");
        assert_eq!(loaded[0].email, "test@example.com");
    }

    #[test]
    fn test_delete_account() {
        let db = test_db();
        let account = make_account();
        db.save_account(&account).unwrap();
        db.delete_account("test-acc-1").unwrap();
        let loaded = db.load_accounts().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_save_and_load_messages() {
        let db = test_db();
        let mail = make_mail("msg-1", "Hello World", "INBOX");
        db.save_messages(&[mail]).unwrap();
        let loaded = db.load_messages("test-acc-1", "INBOX").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "msg-1");
        assert_eq!(loaded[0].subject, "Hello World");
        assert_eq!(loaded[0].body_text, "Full body text");
    }

    #[test]
    fn test_update_read_status() {
        let db = test_db();
        let mail = make_mail("msg-1", "Test", "INBOX");
        db.save_messages(&[mail]).unwrap();
        db.update_read_status("msg-1", true).unwrap();
        let loaded = db.load_messages("test-acc-1", "INBOX").unwrap();
        assert!(loaded[0].is_read);
        db.update_read_status("msg-1", false).unwrap();
        let loaded = db.load_messages("test-acc-1", "INBOX").unwrap();
        assert!(!loaded[0].is_read);
    }

    #[test]
    fn test_update_star_status() {
        let db = test_db();
        let mail = make_mail("msg-1", "Test", "INBOX");
        db.save_messages(&[mail]).unwrap();
        db.update_star_status("msg-1", true).unwrap();
        let loaded = db.load_messages("test-acc-1", "INBOX").unwrap();
        assert!(loaded[0].is_starred);
    }

    #[test]
    fn test_delete_message() {
        let db = test_db();
        let mail = make_mail("msg-1", "Test", "INBOX");
        db.save_messages(&[mail]).unwrap();
        db.delete_message("msg-1").unwrap();
        let loaded = db.load_messages("test-acc-1", "INBOX").unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_search_messages() {
        let db = test_db();
        let mail1 = make_mail("msg-1", "Rust Programming", "INBOX");
        let mail2 = make_mail("msg-2", "Python Tutorial", "INBOX");
        db.save_messages(&[mail1, mail2]).unwrap();
        let results = db.search_messages("Rust").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].subject, "Rust Programming");
    }

    #[test]
    fn test_message_count() {
        let db = test_db();
        let mail1 = make_mail("msg-1", "Test 1", "INBOX");
        let mail2 = make_mail("msg-2", "Test 2", "INBOX");
        let mail3 = make_mail("msg-3", "Test 3", "Sent");
        db.save_messages(&[mail1, mail2, mail3]).unwrap();
        assert_eq!(db.message_count("test-acc-1", "INBOX").unwrap(), 2);
        assert_eq!(db.message_count("test-acc-1", "Sent").unwrap(), 1);
    }

    #[test]
    fn test_categories() {
        let db = test_db();
        let mail = make_mail("msg-1", "Test", "INBOX");
        db.save_messages(&[mail]).unwrap();
        db.add_category("msg-1", "Work").unwrap();
        db.add_category("msg-1", "Important").unwrap();
        let loaded = db.load_messages("test-acc-1", "INBOX").unwrap();
        assert!(loaded[0].categories.contains(&"Work".to_string()));
        assert!(loaded[0].categories.contains(&"Important".to_string()));
        db.remove_category("msg-1", "Work").unwrap();
        let loaded = db.load_messages("test-acc-1", "INBOX").unwrap();
        assert!(!loaded[0].categories.contains(&"Work".to_string()));
        assert!(loaded[0].categories.contains(&"Important".to_string()));
    }

    #[test]
    fn test_contacts() {
        let db = test_db();
        db.upsert_contact("alice@example.com", "Alice").unwrap();
        db.upsert_contact("bob@example.com", "Bob").unwrap();
        db.upsert_contact("alice@example.com", "Alice").unwrap();
        let results = db.search_contacts("alice").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "alice@example.com");
        assert_eq!(results[0].2, 2);
    }

    #[test]
    fn test_pin_and_snooze() {
        let db = test_db();
        let mail = make_mail("msg-1", "Test", "INBOX");
        db.save_messages(&[mail]).unwrap();
        db.update_pinned_status("msg-1", true).unwrap();
        let loaded = db.load_messages("test-acc-1", "INBOX").unwrap();
        assert!(loaded[0].is_pinned);
        db.update_snoozed_until("msg-1", Some("2099-01-01T00:00:00Z")).unwrap();
        let loaded = db.load_messages("test-acc-1", "INBOX").unwrap();
        assert_eq!(loaded[0].snoozed_until.as_deref(), Some("2099-01-01T00:00:00Z"));
    }

    #[test]
    fn test_draft_versioning() {
        let db = test_db();
        for i in 1..=7 {
            db.save_draft_version(
                "draft-1",
                &format!("Subject v{}", i),
                &format!("Body v{}", i),
                Some(&format!("<p>Body v{}</p>", i)),
            )
            .unwrap();
        }
        let versions = db.load_draft_versions("draft-1").unwrap();
        assert_eq!(versions.len(), 5);
        assert_eq!(versions[0].version, 7);
        assert_eq!(versions[0].subject, "Subject v7");
        assert_eq!(versions[4].version, 3);
    }

    #[test]
    fn test_draft_versioning_delete() {
        let db = test_db();
        db.save_draft_version("draft-1", "Subject", "Body", None).unwrap();
        db.delete_draft_versions("draft-1").unwrap();
        let versions = db.load_draft_versions("draft-1").unwrap();
        assert!(versions.is_empty());
    }

    #[test]
    fn test_analytics() {
        let db = test_db();
        let mail1 = make_mail("msg-1", "Inbox mail 1", "INBOX");
        let mail2 = make_mail("msg-2", "Inbox mail 2", "INBOX");
        let mut mail3 = make_mail("msg-3", "Sent mail", "Sent");
        mail3.from = "me@example.com".to_string();
        db.save_messages(&[mail1, mail2, mail3]).unwrap();
        let analytics = db.get_analytics("test-acc-1").unwrap();
        assert_eq!(analytics.total_received, 2);
        assert_eq!(analytics.total_sent, 1);
        assert_eq!(analytics.by_hour.len(), 24);
        assert!(!analytics.top_senders.is_empty());
    }

    #[test]
    fn test_max_uid() {
        let db = test_db();
        let mut mail1 = make_mail("msg-1", "Test 1", "INBOX");
        mail1.uid = 100;
        let mut mail2 = make_mail("msg-2", "Test 2", "INBOX");
        mail2.uid = 200;
        db.save_messages(&[mail1, mail2]).unwrap();
        assert_eq!(db.max_uid("test-acc-1", "INBOX").unwrap(), 200);
    }

    #[test]
    fn test_load_message_body() {
        let db = test_db();
        let mail = make_mail("msg-1", "Test", "INBOX");
        db.save_messages(&[mail]).unwrap();
        let (text, html) = db.load_message_body("msg-1").unwrap();
        assert_eq!(text, "Full body text");
        assert_eq!(html.as_deref(), Some("<p>Full body</p>"));
    }
}
