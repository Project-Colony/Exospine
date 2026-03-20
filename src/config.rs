use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub window_width: u32,
    pub window_height: u32,
    pub theme: String,
    pub font_size: u16,
    pub check_interval_secs: u64,
    pub show_notifications: bool,
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
        }
    }
}

impl Config {
    /// Load configuration from disk, falling back to defaults if the file
    /// is missing or cannot be parsed.
    pub fn load() -> Self {
        let path = Self::config_path();
        match std::fs::read_to_string(&path) {
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
        }
    }

    /// Persist the current configuration to disk, creating parent
    /// directories if they do not already exist.
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

    /// Returns the canonical path for the configuration file:
    /// `<config_dir>/exospine/config.toml`
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("exospine")
            .join("config.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_round_trips() {
        let config = Config::default();
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.window_width, config.window_width);
        assert_eq!(deserialized.theme, config.theme);
        assert_eq!(deserialized.check_interval_secs, config.check_interval_secs);
    }
}
