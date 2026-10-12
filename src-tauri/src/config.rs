use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::app_state::Account;

// ── Portable mode detection ──────────────────────────────────────────

/// Returns `true` if a `portable.txt` file exists next to the executable.
/// When portable mode is active, all config and data are stored in a `data/`
/// folder next to the exe instead of `%APPDATA%` / `~/.config`.
pub fn is_portable() -> bool {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.join("portable.txt").exists();
        }
    }
    false
}

/// Returns the root data directory for Exospine.
/// - Portable mode: `<exe_dir>/data/`
/// - Normal mode: `<config_dir>/exospine/` (e.g. `%APPDATA%/exospine/`)
pub fn data_dir() -> PathBuf {
    if is_portable() {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("data")))
            .unwrap_or_else(|| PathBuf::from("data"))
    } else {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("exospine")
    }
}

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
pub fn default_language() -> String {
    "en".to_string()
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
            language: "en".to_string(),
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
        data_dir().join("config.toml")
    }

    /// Delete accounts.json, any copy set aside as unreadable and the key
    /// salt of older versions (used by secure wipe).
    pub fn delete_accounts_file(&self) -> anyhow::Result<()> {
        ACCOUNTS_READ_ONLY.store(false, Ordering::Relaxed);
        let Ok(entries) = std::fs::read_dir(data_dir()) else {
            return Ok(());
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(ACCOUNTS_FILE) || name == LEGACY_SALT_FILE {
                std::fs::remove_file(entry.path())?;
                tracing::info!("Deleted {}", entry.path().display());
            }
        }
        Ok(())
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

const ACCOUNTS_FILE: &str = "accounts.json";
const LEGACY_SALT_FILE: &str = "encryption.salt";

/// Set when accounts.json must not be overwritten this session: it could not
/// be set aside after a failed read, or it still holds refresh tokens that
/// could not be moved to the keyring.
static ACCOUNTS_READ_ONLY: AtomicBool = AtomicBool::new(false);

/// The account list read at startup.
pub struct LoadedAccounts {
    pub accounts: Vec<Account>,
    /// Whether saving may replace accounts.json.
    writable: bool,
    /// A problem the user has to hear about, shown once by the UI.
    pub problem: Option<String>,
}

/// Read accounts.json, migrating the formats of older versions.
pub fn load_accounts() -> LoadedAccounts {
    let loaded = load_accounts_in(&data_dir(), &mut |account_id, token| {
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
            problem: Some(format!(
                "Some sign-in tokens could not be moved to the system keyring. {} was left \
                 as it is and changes to your accounts will not be saved until Exospine \
                 can reach the keyring.",
                path.display()
            )),
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
            problem: Some(format!(
                "Exospine could not read your account list ({reason}). It was kept as {} \
                 and Exospine started without accounts.",
                kept.display()
            )),
        },
        Err(e) => {
            tracing::error!("Failed to rename {}: {}", path.display(), e);
            LoadedAccounts {
                accounts: Vec::new(),
                writable: false,
                problem: Some(format!(
                    "Exospine could not read your account list at {} ({reason}). It was \
                     left untouched, and changes to your accounts will not be saved until \
                     it is moved away.",
                    path.display()
                )),
            }
        }
    }
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
    let path = data_dir().join(ACCOUNTS_FILE);
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
        assert!(loaded.problem.is_some());
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
    fn token_stays_on_disk_when_keyring_fails() {
        let dir = temp_dir();
        let path = dir.join(ACCOUNTS_FILE);
        std::fs::write(&path, LEGACY_JSON).unwrap();

        let (loaded, stored) = load(&dir, false);
        assert_eq!(stored, moved_token());
        assert_eq!(loaded.accounts.len(), 1);
        assert!(!loaded.writable && loaded.problem.is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), LEGACY_JSON);
        std::fs::remove_dir_all(&dir).unwrap();
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
        assert_eq!(config.language, "en");
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
