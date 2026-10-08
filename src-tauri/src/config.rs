use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

// ── Encryption helpers for accounts.json at rest ─────────────────────

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use aes_gcm::aead::generic_array::GenericArray;
use sha2::{Digest, Sha256};

/// Path to the per-installation random salt file.
fn salt_path() -> PathBuf {
    data_dir().join("encryption.salt")
}

/// Load or create a 32-byte random salt unique to this installation.
/// The salt is stored in `<data_dir>/encryption.salt`.
///
/// Resolved once per process: concurrent first calls would otherwise each
/// generate and write their own salt (or read a file another call has just
/// truncated) and derive different keys.
fn load_or_create_salt() -> [u8; 32] {
    static SALT: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    *SALT.get_or_init(read_or_create_salt_file)
}

fn read_or_create_salt_file() -> [u8; 32] {
    let path = salt_path();
    if let Ok(data) = std::fs::read(&path) {
        if data.len() == 32 {
            let mut salt = [0u8; 32];
            salt.copy_from_slice(&data);
            return salt;
        }
    }
    // Generate a new random salt
    let mut salt = [0u8; 32];
    {
        use aes_gcm::aead::rand_core::RngCore;
        OsRng.fill_bytes(&mut salt);
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&path, salt) {
        tracing::error!("Failed to write encryption salt: {}", e);
    }
    salt
}

/// Derive a 256-bit encryption key from machine-specific data plus a
/// per-installation random salt. This prevents the same key from being
/// derived on different machines even with identical hostname/username.
fn derive_encryption_key() -> [u8; 32] {
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "exospine-host".to_string());
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "exospine-user".to_string());
    let installation_salt = load_or_create_salt();

    let mut hasher = Sha256::new();
    hasher.update(hostname.as_bytes());
    hasher.update(b":");
    hasher.update(username.as_bytes());
    hasher.update(b":");
    hasher.update(installation_salt);
    hasher.finalize().into()
}

/// Encrypt plaintext bytes with AES-256-GCM. Returns nonce (12 bytes) || ciphertext.
fn encrypt_data(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let key = derive_encryption_key();
    let cipher =
        Aes256Gcm::new(GenericArray::from_slice(&key));

    // Generate a random 96-bit nonce
    let nonce_bytes: [u8; 12] = {
        use aes_gcm::aead::rand_core::RngCore;
        let mut buf = [0u8; 12];
        OsRng.fill_bytes(&mut buf);
        buf
    };
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("Encryption failed: {}", e))?;

    // Prepend nonce so we can recover it on decrypt
    let mut out = Vec::with_capacity(12 + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt data produced by `encrypt_data` (nonce || ciphertext).
fn decrypt_data(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 13 {
        return Err("Encrypted data too short".to_string());
    }
    let key = derive_encryption_key();
    let cipher =
        Aes256Gcm::new(GenericArray::from_slice(&key));

    let nonce = Nonce::from_slice(&data[..12]);
    let ciphertext = &data[12..];

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {}", e))
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

        // Allow environment variables to override OAuth secrets from config.toml.
        if let Ok(val) = std::env::var("EXOSPINE_GOOGLE_CLIENT_ID") {
            config.google_client_id = val;
        }
        if let Ok(val) = std::env::var("EXOSPINE_GOOGLE_CLIENT_SECRET") {
            config.google_client_secret = val;
        }
        if let Ok(val) = std::env::var("EXOSPINE_MICROSOFT_CLIENT_ID") {
            config.microsoft_client_id = val;
        }
        if let Ok(val) = std::env::var("EXOSPINE_MICROSOFT_CLIENT_SECRET") {
            config.microsoft_client_secret = val;
        }

        config
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        tracing::info!("Saved config to {}", path.display());
        Ok(())
    }

    pub fn config_path() -> PathBuf {
        data_dir().join("config.toml")
    }

    /// Delete the accounts.json file from disk (used by secure wipe).
    pub fn delete_accounts_file(&self) -> anyhow::Result<()> {
        let path = accounts_path();
        if path.exists() {
            std::fs::remove_file(&path)?;
            tracing::info!("Deleted accounts file: {}", path.display());
        }
        Ok(())
    }
}

// ── Account persistence ──────────────────────────────────────────────

fn accounts_path() -> PathBuf {
    data_dir().join("accounts.json")
}

pub fn save_accounts(accounts: &[Account]) {
    let path = accounts_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::error!("Failed to create config directory: {}", e);
            return;
        }
    }
    match serde_json::to_string_pretty(accounts) {
        Ok(json) => {
            match encrypt_data(json.as_bytes()) {
                Ok(encrypted) => {
                    if let Err(e) = std::fs::write(&path, &encrypted) {
                        tracing::error!("Failed to write accounts to {}: {}", path.display(), e);
                    } else {
                        tracing::info!(
                            "Saved {} account(s) to {} (encrypted)",
                            accounts.len(),
                            path.display()
                        );
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to encrypt accounts: {}", e);
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to serialize accounts: {}", e);
        }
    }
}

pub fn load_accounts() -> Vec<Account> {
    let path = accounts_path();
    match std::fs::read(&path) {
        Ok(data) => {
            // Try decrypting first (new encrypted format)
            if let Ok(plaintext) = decrypt_data(&data) {
                if let Ok(json_str) = String::from_utf8(plaintext) {
                    match serde_json::from_str::<Vec<Account>>(&json_str) {
                        Ok(accounts) => {
                            tracing::info!(
                                "Loaded {} account(s) from {} (encrypted)",
                                accounts.len(),
                                path.display()
                            );
                            return accounts;
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Decrypted accounts but failed to parse JSON: {}",
                                e
                            );
                        }
                    }
                }
            }

            // Fallback: try reading as plaintext JSON (migration from unencrypted format)
            if let Ok(json_str) = String::from_utf8(data) {
                match serde_json::from_str::<Vec<Account>>(&json_str) {
                    Ok(accounts) => {
                        tracing::info!(
                            "Loaded {} account(s) from {} (plaintext, will re-encrypt)",
                            accounts.len(),
                            path.display()
                        );
                        // Re-save to encrypt the file
                        save_accounts(&accounts);
                        return accounts;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse accounts at {}: {}. Starting with no accounts.",
                            path.display(),
                            e
                        );
                    }
                }
            }

            Vec::new()
        }
        Err(_) => {
            tracing::info!("No accounts file at {}. Starting fresh.", path.display());
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = b"Hello, this is a secret message!";
        let encrypted = encrypt_data(plaintext).expect("Encryption failed");
        // Encrypted data should be different from plaintext
        assert_ne!(encrypted, plaintext);
        // Encrypted data should be longer (nonce + ciphertext + auth tag)
        assert!(encrypted.len() > plaintext.len());

        let decrypted = decrypt_data(&encrypted).expect("Decryption failed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_empty() {
        let plaintext = b"";
        let encrypted = encrypt_data(plaintext).expect("Encryption failed");
        let decrypted = decrypt_data(&encrypted).expect("Decryption failed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_large_data() {
        let plaintext = vec![42u8; 10_000];
        let encrypted = encrypt_data(&plaintext).expect("Encryption failed");
        let decrypted = decrypt_data(&encrypted).expect("Decryption failed");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_too_short() {
        let short_data = vec![0u8; 5];
        let result = decrypt_data(&short_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_corrupted_data() {
        let plaintext = b"Test data";
        let mut encrypted = encrypt_data(plaintext).expect("Encryption failed");
        // Corrupt the ciphertext (not the nonce)
        if encrypted.len() > 15 {
            encrypted[15] ^= 0xFF;
        }
        let result = decrypt_data(&encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_derive_key_deterministic() {
        let key1 = derive_encryption_key();
        let key2 = derive_encryption_key();
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_encrypt_produces_unique_ciphertexts() {
        let plaintext = b"Same message";
        let enc1 = encrypt_data(plaintext).expect("Encryption failed");
        let enc2 = encrypt_data(plaintext).expect("Encryption failed");
        // Different nonces should produce different ciphertexts
        assert_ne!(enc1, enc2);
        // But both should decrypt to the same plaintext
        let dec1 = decrypt_data(&enc1).expect("Decryption failed");
        let dec2 = decrypt_data(&enc2).expect("Decryption failed");
        assert_eq!(dec1, dec2);
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
