use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::app_state::MailEntry;

// Re-export types from sub-modules so external code using `crate::storage::db::TypeName` still works.
pub use super::analytics_db::EmailAnalytics;
pub use super::drafts_db::DraftVersion;
pub use super::followup_db::Followup;
pub use super::schedule_db::ScheduledEmail;
pub use super::tasks_db::Task;

/// A contact row: (email, name, frequency, phone, company, notes).
pub type ContactRow = (String, String, u32, String, String, String);

/// SQLite-backed local storage for accounts and cached messages.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Expose the inner connection to sub-modules in this crate.
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Current schema version. Bump this when adding new migrations.
    pub(crate) const SCHEMA_VERSION: u32 = 7;
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
                ON draft_versions(draft_id, version DESC);

            CREATE TABLE IF NOT EXISTS followups (
                mail_id TEXT PRIMARY KEY,
                expected_from TEXT NOT NULL,
                created_at TEXT NOT NULL,
                due_date TEXT NOT NULL DEFAULT '',
                resolved INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                mail_id TEXT NOT NULL DEFAULT '',
                due_date TEXT NOT NULL DEFAULT '',
                completed INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );",
        )?;
        Ok(())
    }

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

        // Migration 7: add notes table and unsubscribe_url column
        if current_version < 7 {
            self.conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS notes (
                    mail_id TEXT PRIMARY KEY,
                    note TEXT NOT NULL DEFAULT '',
                    updated_at TEXT NOT NULL DEFAULT ''
                );

                ALTER TABLE messages ADD COLUMN unsubscribe_url TEXT;",
            ).unwrap_or_else(|e| {
                tracing::warn!("Migration 7 partial: {}", e);
                // Try each separately in case one already exists
                let _ = self.conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS notes (
                        mail_id TEXT PRIMARY KEY,
                        note TEXT NOT NULL DEFAULT '',
                        updated_at TEXT NOT NULL DEFAULT ''
                    );"
                );
                let _ = self.conn.execute_batch(
                    "ALTER TABLE messages ADD COLUMN unsubscribe_url TEXT;"
                );
            });
        }

        // Update schema version to current
        self.conn.execute_batch(&format!(
            "PRAGMA user_version = {};",
            Self::SCHEMA_VERSION
        ))?;

        Ok(())
    }
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
    unsubscribe_url: Option<String>,
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
    let unsubscribe_url: Option<String> = row.get::<_, Option<String>>(base + 12).unwrap_or(None);
    ExtraCols { message_id, in_reply_to, references, thread_id, spam_score, categories, is_pinned, snoozed_until, importance, read_receipt_requested, read_receipt_to, flag_due_date, unsubscribe_url }
}

/// Convert a full row (with body_text, body_html) to MailEntry.
pub(crate) fn row_to_mail_entry_full(row: &rusqlite::Row) -> rusqlite::Result<MailEntry> {
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
        unsubscribe_url: extra.unsubscribe_url,
    })
}

/// Convert a header-only row (no body_text/body_html) to MailEntry.
pub(crate) fn row_to_mail_entry_headers(row: &rusqlite::Row) -> rusqlite::Result<MailEntry> {
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
        unsubscribe_url: extra.unsubscribe_url,
    })
}

/// Parse an email address string like "Name <email@example.com>" into (name, email).
pub(crate) fn parse_address(addr: &str) -> (String, String) {
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
