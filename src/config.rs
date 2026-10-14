use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::app_state::Account;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub window_width: u32,
    pub window_height: u32,
    pub theme: String,
    pub font_size: u16,
    pub check_interval_secs: u64,
    pub show_notifications: bool,
    #[serde(default = "default_reading_pane")]
    pub reading_pane: String,
    #[serde(default = "default_density")]
    pub density: String,
    #[serde(default = "default_language")]
    pub language: String,

    #[serde(default)]
    pub google_client_id: String,
    #[serde(default)]
    pub google_client_secret: String,
    #[serde(default)]
    pub microsoft_client_id: String,
    #[serde(default)]
    pub microsoft_client_secret: String,

    /// config.toml values that an `EXOSPINE_*` variable replaced at load.
    /// `save` writes these back, so a variable stays a runtime override and
    /// never lands in the file.
    #[serde(skip)]
    file_values: Vec<(&'static str, String)>,
}

pub fn default_reading_pane() -> String {
    "right".to_string()
}
pub fn default_density() -> String {
    "normal".to_string()
}
/// "auto" follows the system language; "en" and "fr" pick one.
pub fn default_language() -> String {
    "auto".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window_width: 1200,
            window_height: 800,
            theme: "dark".to_string(),
            font_size: 14,
            check_interval_secs: 300,
            show_notifications: true,
            reading_pane: "right".to_string(),
            density: "normal".to_string(),
            language: default_language(),
            google_client_id: String::new(),
            google_client_secret: String::new(),
            microsoft_client_id: String::new(),
            microsoft_client_secret: String::new(),
            file_values: Vec::new(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = Self::config_path();
        let mut config = match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str::<Config>(&contents) {
                Ok(config) => {
                    tracing::info!("Loaded config from {}", path.display());
                    config
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse config at {}: {}. Using defaults.",
                        path.display(),
                        e
                    );
                    Self::default()
                }
            },
            Err(_) => {
                tracing::info!(
                    "No config file found at {}. Using defaults.",
                    path.display()
                );
                Self::default()
            }
        };

        config.apply_env_overrides(|name| std::env::var(name).ok());
        config
    }

    /// The OAuth client fields that an environment variable can override.
    fn env_overridable(&mut self) -> [(&'static str, &mut String); 4] {
        [
            ("EXOSPINE_GOOGLE_CLIENT_ID", &mut self.google_client_id),
            ("EXOSPINE_GOOGLE_CLIENT_SECRET", &mut self.google_client_secret),
            ("EXOSPINE_MICROSOFT_CLIENT_ID", &mut self.microsoft_client_id),
            ("EXOSPINE_MICROSOFT_CLIENT_SECRET", &mut self.microsoft_client_secret),
        ]
    }

    /// Let environment variables override the OAuth client settings of
    /// config.toml for this run, remembering the file values for `save`.
    fn apply_env_overrides(&mut self, var: impl Fn(&str) -> Option<String>) {
        let mut replaced = Vec::new();
        for (name, field) in self.env_overridable() {
            if let Some(value) = var(name) {
                replaced.push((name, std::mem::replace(field, value)));
            }
        }
        self.file_values = replaced;
    }

    /// The config.toml text: current settings, with the file's own values in
    /// place of environment overrides.
    fn to_file_toml(&self) -> Result<String, toml::ser::Error> {
        let mut on_disk = self.clone();
        for (name, field) in on_disk.env_overridable() {
            if let Some((_, value)) = self.file_values.iter().find(|(n, _)| *n == name) {
                *field = value.clone();
            }
        }
        toml::to_string_pretty(&on_disk)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, self.to_file_toml()?)?;
        tracing::info!("Saved config to {}", path.display());
        Ok(())
    }

    pub fn config_path() -> PathBuf {
        crate::paths::layout().preferences.join(CONFIG_FILE)
    }

    /// Delete accounts.json, any copy set aside as unreadable and the key
    /// salt of older versions (used by secure wipe), both where they are used
    /// and where versions up to 0.2 kept them, which hold a copy after the
    /// move to the Colony layout. The keyring entries of the accounts these
    /// files name go too, as a copy set aside holds accounts that are not in
    /// memory. An encrypted copy of an older version cannot be read, so the
    /// entries of its accounts remain.
    pub fn delete_accounts_file(&self) -> anyhow::Result<()> {
        ACCOUNTS_READ_ONLY.store(false, Ordering::Relaxed);
        // Keep going after a failure so every other copy and its keyring
        // entries are still removed, then report the first failure.
        let mut first_error = None;
        for dir in crate::paths::account_dirs() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with(ACCOUNTS_FILE) {
                    for id in account_ids_in(&entry.path()) {
                        crate::accounts::keyring_store::delete_account_secrets(&id);
                    }
                }
                if name.starts_with(ACCOUNTS_FILE) || name == LEGACY_SALT_FILE {
                    match std::fs::remove_file(entry.path()) {
                        Ok(()) => tracing::info!("Deleted {}", entry.path().display()),
                        Err(e) => {
                            tracing::error!("Cannot delete {}: {}", entry.path().display(), e);
                            first_error.get_or_insert_with(|| {
                                anyhow::anyhow!("Cannot delete {}: {}", entry.path().display(), e)
                            });
                        }
                    }
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

// ── Account persistence ──────────────────────────────────────────────
//
// accounts.json is plain JSON and holds no secrets: passwords and OAuth
// refresh tokens live in the OS keyring. Older versions encrypted the file
// with a key derived from the host name, the user name and encryption.salt,
// which broke whenever one of them changed (always, for a portable copy on
// another machine), and kept refresh tokens inside it. Both are migrated
// once on load.

pub(crate) const CONFIG_FILE: &str = "config.toml";
pub(crate) const ACCOUNTS_FILE: &str = "accounts.json";
pub(crate) const LEGACY_SALT_FILE: &str = "encryption.salt";

/// Set when accounts.json must not be overwritten this session: it could not
/// be set aside after a failed read, or it still holds refresh tokens that
/// could not be moved to the keyring.
static ACCOUNTS_READ_ONLY: AtomicBool = AtomicBool::new(false);

/// Refuse to add or remove an account while accounts.json cannot be saved:
/// the change would be lost at the next start, and a removed account would
/// come back without its keyring entries. The error is the key of the
/// message the UI shows in the user's language.
pub fn ensure_accounts_writable() -> Result<(), String> {
    if ACCOUNTS_READ_ONLY.load(Ordering::Relaxed) {
        return Err("accounts_read_only".to_string());
    }
    Ok(())
}

/// What went wrong with accounts.json at startup. The UI words it in the
/// user's language from `kind` (accounts_problem_<kind>) and the fields.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AccountsProblem {
    /// Refresh tokens could not move to the keyring: the file at `path` is
    /// left as it is and account changes are not saved this session.
    KeyringUnreachable { path: String },
    /// The file could not be read (`reason`) and was renamed to `kept`.
    SetAside { reason: String, kept: String },
    /// The file at `path` could not be read (`reason`) nor renamed.
    Unreadable { reason: String, path: String },
}

/// The account list read at startup.
pub struct LoadedAccounts {
    pub accounts: Vec<Account>,
    /// Whether saving may replace accounts.json.
    writable: bool,
    /// A problem the user has to hear about, shown once by the UI.
    pub problem: Option<AccountsProblem>,
}

/// Read accounts.json, migrating the formats of older versions.
pub fn load_accounts() -> LoadedAccounts {
    let loaded = load_accounts_in(&crate::paths::layout().preferences, &mut |account_id, token| {
        use crate::accounts::keyring_store;
        // A token already in the keyring came from an earlier migration of
        // this file, possibly rotated since: it is the newer one.
        if keyring_store::get_refresh_token(account_id).is_ok() {
            return true;
        }
        keyring_store::store_refresh_token(account_id, token)
            .map_err(|e| {
                tracing::error!(
                    "Failed to move the refresh token of account {} to the keyring: {:#}",
                    account_id,
                    e
                )
            })
            .is_ok()
    });
    ACCOUNTS_READ_ONLY.store(!loaded.writable, Ordering::Relaxed);
    loaded
}

/// `load_accounts` for the accounts.json in `dir`. `store_refresh_token`
/// moves one legacy token to the keyring and reports whether it succeeded.
fn load_accounts_in(
    dir: &Path,
    store_refresh_token: &mut dyn FnMut(&str, &str) -> bool,
) -> LoadedAccounts {
    let path = dir.join(ACCOUNTS_FILE);
    let data = match std::fs::read(&path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!("No accounts file at {}. Starting fresh.", path.display());
            return LoadedAccounts { accounts: Vec::new(), writable: true, problem: None };
        }
        Err(e) => return set_aside(&path, &e.to_string()),
    };

    let (mut json, encrypted) = match serde_json::from_slice::<serde_json::Value>(&data) {
        Ok(json) => (json, false),
        Err(_) => match legacy_decrypt(&data, &dir.join(LEGACY_SALT_FILE))
            .and_then(|plain| serde_json::from_slice(&plain).ok())
        {
            Some(json) => (json, true),
            None => return set_aside(&path, "it is neither JSON nor decryptable on this machine"),
        },
    };

    let tokens = take_refresh_tokens(&mut json);
    let accounts: Vec<Account> = match serde_json::from_value(json) {
        Ok(accounts) => accounts,
        Err(e) => return set_aside(&path, &e.to_string()),
    };

    let mut moved = true;
    for (account_id, token) in &tokens {
        moved &= store_refresh_token(account_id, token);
    }
    if !moved {
        return LoadedAccounts {
            accounts,
            writable: false,
            problem: Some(AccountsProblem::KeyringUnreachable {
                path: path.display().to_string(),
            }),
        };
    }

    if encrypted || !tokens.is_empty() {
        match write_accounts(&path, &accounts) {
            Ok(()) => {
                tracing::info!("Migrated {} to plain JSON without secrets", path.display());
                if encrypted {
                    let _ = std::fs::remove_file(dir.join(LEGACY_SALT_FILE));
                }
            }
            Err(e) => tracing::error!("Failed to rewrite {}: {}", path.display(), e),
        }
    }

    tracing::info!("Loaded {} account(s) from {}", accounts.len(), path.display());
    LoadedAccounts { accounts, writable: true, problem: None }
}

/// Rename an accounts.json that cannot be used, so the next save starts a new
/// file instead of destroying the old one.
fn set_aside(path: &Path, reason: &str) -> LoadedAccounts {
    tracing::error!("Cannot read {}: {}", path.display(), reason);
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let kept = path.with_file_name(format!("{ACCOUNTS_FILE}.unreadable-{stamp}"));
    match std::fs::rename(path, &kept) {
        Ok(()) => LoadedAccounts {
            accounts: Vec::new(),
            writable: true,
            problem: Some(AccountsProblem::SetAside {
                reason: reason.to_owned(),
                kept: kept.display().to_string(),
            }),
        },
        Err(e) => {
            tracing::error!("Failed to rename {}: {}", path.display(), e);
            LoadedAccounts {
                accounts: Vec::new(),
                writable: false,
                problem: Some(AccountsProblem::Unreadable {
                    reason: reason.to_owned(),
                    path: path.display().to_string(),
                }),
            }
        }
    }
}

/// The account ids in an accounts.json, or none when it is not plain JSON.
fn account_ids_in(path: &Path) -> Vec<String> {
    let json: serde_json::Value = std::fs::read(path)
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default();
    json.as_array()
        .into_iter()
        .flatten()
        .filter_map(|account| account["id"].as_str().map(str::to_owned))
        .collect()
}

/// Remove the OAuth refresh tokens that older versions kept in accounts.json
/// (`"auth_method": {"OAuth2": {"refresh_token": "..."}}`), leaving the
/// current `"auth_method": "OAuth2"`. Returns (account id, token) pairs.
fn take_refresh_tokens(json: &mut serde_json::Value) -> Vec<(String, String)> {
    let mut tokens = Vec::new();
    for account in json.as_array_mut().into_iter().flatten() {
        let Some(legacy) = account.get("auth_method").and_then(|m| m.get("OAuth2")) else {
            continue;
        };
        let token = legacy.get("refresh_token").and_then(|t| t.as_str()).unwrap_or_default();
        let id = account.get("id").and_then(|id| id.as_str()).unwrap_or_default();
        if !token.is_empty() {
            tokens.push((id.to_owned(), token.to_owned()));
        }
        account["auth_method"] = "OAuth2".into();
    }
    tokens
}

/// Key of the encrypted accounts.json of older versions:
/// SHA-256(host name ":" user name ":" installation salt).
fn legacy_key(salt: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "exospine-host".to_string());
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "exospine-user".to_string());
    let mut hasher = Sha256::new();
    hasher.update(hostname.as_bytes());
    hasher.update(b":");
    hasher.update(username.as_bytes());
    hasher.update(b":");
    hasher.update(salt);
    hasher.finalize().into()
}

/// Decrypt an accounts.json written by an older version: AES-256-GCM,
/// 12-byte nonce followed by the ciphertext.
fn legacy_decrypt(data: &[u8], salt_path: &Path) -> Option<Vec<u8>> {
    use aes_gcm::aead::generic_array::GenericArray;
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    let salt = std::fs::read(salt_path).ok().filter(|salt| salt.len() == 32)?;
    let (nonce, ciphertext) = data.split_at_checked(12)?;
    Aes256Gcm::new(GenericArray::from_slice(&legacy_key(&salt)))
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .ok()
}

/// Write accounts.json through a temporary file, so a crash mid-write cannot
/// leave a truncated file behind.
fn write_accounts(path: &Path, accounts: &[Account]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(accounts)?)?;
    std::fs::rename(&tmp, path)
}

pub fn save_accounts(accounts: &[Account]) {
    let path = crate::paths::layout().preferences.join(ACCOUNTS_FILE);
    if ACCOUNTS_READ_ONLY.load(Ordering::Relaxed) {
        tracing::warn!("Not saving accounts: {} is kept as it is", path.display());
        return;
    }
    match write_accounts(&path, accounts) {
        Ok(()) => tracing::info!("Saved {} account(s) to {}", accounts.len(), path.display()),
        Err(e) => tracing::error!("Failed to write accounts to {}: {}", path.display(), e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AuthMethod;

    const LEGACY_JSON: &str = r#"[{
        "id": "acct-1", "name": "Ada", "email": "ada@example.com",
        "imap_host": "imap.example.com", "imap_port": 993,
        "smtp_host": "smtp.example.com", "smtp_port": 587,
        "username": "ada@example.com", "use_tls": true, "folders": [],
        "auth_method": {"OAuth2": {"refresh_token": "rt-secret-123"}}
    }]"#;

    /// A fresh directory under the system temp dir, never the real profile.
    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("exospine-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Load from `dir`, recording the tokens handed to the keyring.
    fn load(dir: &Path, keyring_ok: bool) -> (LoadedAccounts, Vec<(String, String)>) {
        let mut stored = Vec::new();
        let loaded = load_accounts_in(dir, &mut |id, token| {
            stored.push((id.to_owned(), token.to_owned()));
            keyring_ok
        });
        (loaded, stored)
    }

    fn moved_token() -> Vec<(String, String)> {
        vec![("acct-1".to_owned(), "rt-secret-123".to_owned())]
    }

    #[test]
    fn missing_file_starts_empty_and_writable() {
        let dir = temp_dir();
        let (loaded, stored) = load(&dir, true);
        assert!(loaded.accounts.is_empty() && loaded.writable && loaded.problem.is_none());
        assert!(stored.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refresh_token_moves_out_of_accounts_json() {
        let dir = temp_dir();
        let path = dir.join(ACCOUNTS_FILE);
        std::fs::write(&path, LEGACY_JSON).unwrap();

        let (loaded, stored) = load(&dir, true);
        assert_eq!(stored, moved_token());
        assert!(loaded.writable && loaded.problem.is_none());
        assert_eq!(loaded.accounts[0].auth_method, AuthMethod::OAuth2);

        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(!on_disk.contains("rt-secret-123") && !on_disk.contains("refresh_token"));
        // The rewritten file reads back without a second migration.
        let (again, stored) = load(&dir, true);
        assert_eq!(again.accounts.len(), 1);
        assert!(stored.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn saved_accounts_hold_no_secret() {
        let dir = temp_dir();
        let path = dir.join(ACCOUNTS_FILE);
        let account = Account {
            id: "acct-1".into(),
            auth_method: AuthMethod::OAuth2,
            ..Default::default()
        };
        write_accounts(&path, &[account]).unwrap();
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(on_disk.contains(r#""auth_method": "OAuth2""#));
        assert!(!on_disk.contains("token") && !on_disk.contains("password"));
        assert!(!dir.join("accounts.json.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn legacy_encrypted_file_migrates() {
        use aes_gcm::aead::generic_array::GenericArray;
        use aes_gcm::aead::{Aead, KeyInit};
        use aes_gcm::{Aes256Gcm, Nonce};

        let dir = temp_dir();
        let salt = [7u8; 32];
        std::fs::write(dir.join(LEGACY_SALT_FILE), salt).unwrap();
        let nonce = [3u8; 12];
        let mut encrypted = nonce.to_vec();
        encrypted.extend(
            Aes256Gcm::new(GenericArray::from_slice(&legacy_key(&salt)))
                .encrypt(Nonce::from_slice(&nonce), LEGACY_JSON.as_bytes())
                .unwrap(),
        );
        std::fs::write(dir.join(ACCOUNTS_FILE), &encrypted).unwrap();

        let (loaded, stored) = load(&dir, true);
        assert_eq!(stored, moved_token());
        assert_eq!(loaded.accounts[0].email, "ada@example.com");
        assert!(loaded.writable && loaded.problem.is_none());

        let on_disk = std::fs::read_to_string(dir.join(ACCOUNTS_FILE)).unwrap();
        let plain: Vec<Account> = serde_json::from_str(&on_disk).unwrap();
        assert_eq!(plain[0].id, "acct-1");
        assert!(!on_disk.contains("rt-secret-123"));
        assert!(!dir.join(LEGACY_SALT_FILE).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn undecryptable_file_is_kept_aside() {
        let dir = temp_dir();
        let garbage = b"\x00\x01not json and no salt to decrypt it with".to_vec();
        std::fs::write(dir.join(ACCOUNTS_FILE), &garbage).unwrap();

        let (loaded, stored) = load(&dir, true);
        assert!(loaded.accounts.is_empty() && stored.is_empty());
        assert!(loaded.writable);
        assert!(matches!(loaded.problem, Some(AccountsProblem::SetAside { .. })));
        assert!(!dir.join(ACCOUNTS_FILE).exists());

        let kept: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("accounts.json.unreadable-"))
            .collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(std::fs::read(kept[0].path()).unwrap(), garbage);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn account_ids_come_from_plain_json_only() {
        let dir = temp_dir();
        let plain = dir.join("accounts.json.unreadable-1");
        std::fs::write(&plain, LEGACY_JSON).unwrap();
        assert_eq!(account_ids_in(&plain), ["acct-1"]);
        let garbage = dir.join("accounts.json.unreadable-2");
        std::fs::write(&garbage, b"\x00\x01not json").unwrap();
        assert!(account_ids_in(&garbage).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn token_stays_on_disk_when_keyring_fails() {
        let dir = temp_dir();
        let path = dir.join(ACCOUNTS_FILE);
        std::fs::write(&path, LEGACY_JSON).unwrap();

        let (loaded, stored) = load(&dir, false);
        assert_eq!(stored, moved_token());
        assert_eq!(loaded.accounts.len(), 1);
        assert!(!loaded.writable);
        assert!(matches!(loaded.problem, Some(AccountsProblem::KeyringUnreachable { .. })));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), LEGACY_JSON);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn problems_reach_the_ui_with_their_kind() {
        let problem = AccountsProblem::SetAside { reason: "bad".into(), kept: "x".into() };
        assert_eq!(
            serde_json::to_value(problem).unwrap(),
            serde_json::json!({"kind": "set_aside", "reason": "bad", "kept": "x"})
        );
    }

    #[test]
    fn env_overrides_never_reach_config_toml() {
        let mut config = Config {
            google_client_id: "file-id".into(),
            ..Default::default()
        };
        config.apply_env_overrides(|name| match name {
            "EXOSPINE_GOOGLE_CLIENT_ID" => Some("env-id".into()),
            "EXOSPINE_MICROSOFT_CLIENT_SECRET" => Some("env-secret".into()),
            _ => None,
        });
        assert_eq!(config.google_client_id, "env-id");
        assert_eq!(config.microsoft_client_secret, "env-secret");

        let on_disk: Config = toml::from_str(&config.to_file_toml().unwrap()).unwrap();
        assert_eq!(on_disk.google_client_id, "file-id");
        assert_eq!(on_disk.microsoft_client_secret, "");
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.theme, "dark");
        assert_eq!(config.font_size, 14);
        assert_eq!(config.check_interval_secs, 300);
        assert!(config.show_notifications);
        assert_eq!(config.reading_pane, "right");
        assert_eq!(config.language, "auto");
    }

    #[test]
    fn test_config_toml_roundtrip() {
        let config = Config {
            theme: "light".to_string(),
            font_size: 16,
            check_interval_secs: 120,
            show_notifications: false,
            reading_pane: "bottom".to_string(),
            density: "compact".to_string(),
            language: "fr".to_string(),
            ..Default::default()
        };
        let toml_str = toml::to_string_pretty(&config).expect("Serialize failed");
        let parsed: Config = toml::from_str(&toml_str).expect("Deserialize failed");
        assert_eq!(parsed.theme, "light");
        assert_eq!(parsed.font_size, 16);
        assert_eq!(parsed.check_interval_secs, 120);
        assert!(!parsed.show_notifications);
        assert_eq!(parsed.reading_pane, "bottom");
        assert_eq!(parsed.language, "fr");
    }
}
