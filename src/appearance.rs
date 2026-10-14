//! Appearance and accessibility preferences.
//!
//! The field names and values are those of
//! `colony_ui::preferences::StandardPreferences`, which Exospine cannot link
//! (its interface is a webview), so the Preferences page reads and writes the
//! same settings as every other Colony program. Like the original, reading
//! never fails: a missing or malformed field falls back to its default and
//! leaves the others alone.
//!
//! Versions up to 0.2 stored `theme = "dark" | "light" | "auto" |
//! "high-contrast" | "custom"` and `font_size` in pixels. Both are read once
//! and carried over to the Colony fields.

use serde::de::{Deserializer, IgnoredAny};
use serde::{Deserialize, Serialize};

/// The theme of a fresh install, and the fallback for an unknown one.
pub const DEFAULT_THEME: (&str, &str) = ("gruvbox", "dark");

/// Typography size (Preferences > Appearance): 0.85, 1.0 or 1.2.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontSize {
    Small,
    #[default]
    Default,
    Large,
}

/// Text size (Preferences > Accessibility): 0.85, 1.0, 1.2 or 1.4.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextSize {
    Small,
    #[default]
    Default,
    Large,
    Xlarge,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Appearance {
    pub selected_theme: String,
    pub selected_variant: String,
    /// The picked accent's key, or none for the theme's own accent. Never
    /// "auto": that is `auto_accent`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_accent: Option<String>,
    pub auto_accent: bool,
    pub font_size: FontSize,
    pub animations: bool,
    pub high_contrast: bool,
    pub dyslexia_font: bool,
    pub reduce_motion: bool,
    pub text_size_a11y: TextSize,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            selected_theme: DEFAULT_THEME.0.to_string(),
            selected_variant: DEFAULT_THEME.1.to_string(),
            selected_accent: None,
            auto_accent: false,
            font_size: FontSize::Default,
            animations: true,
            high_contrast: false,
            dyslexia_font: false,
            reduce_motion: false,
            text_size_a11y: TextSize::Default,
        }
    }
}

/// A field that is `None` when missing or malformed, instead of an error.
fn lenient<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OrAnything<T> {
        Value(T),
        Other(IgnoredAny),
    }
    Ok(match OrAnything::<T>::deserialize(deserializer)? {
        OrAnything::Value(value) => Some(value),
        OrAnything::Other(_) => None,
    })
}

/// `font_size` as Colony writes it, or as a pixel size from 0.2.
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredFontSize {
    Step(FontSize),
    Pixels(i64),
}

impl StoredFontSize {
    /// The step nearest a pixel size, against the old 14 px default.
    fn step(self) -> FontSize {
        match self {
            StoredFontSize::Step(step) => step,
            StoredFontSize::Pixels(px) if px < 13 => FontSize::Small,
            StoredFontSize::Pixels(px) if px < 16 => FontSize::Default,
            StoredFontSize::Pixels(_) => FontSize::Large,
        }
    }
}

impl<'de> Deserialize<'de> for Appearance {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Default, Deserialize)]
        #[serde(default)]
        struct Stored {
            #[serde(deserialize_with = "lenient")]
            selected_theme: Option<String>,
            #[serde(deserialize_with = "lenient")]
            selected_variant: Option<String>,
            #[serde(deserialize_with = "lenient")]
            selected_accent: Option<String>,
            #[serde(deserialize_with = "lenient")]
            auto_accent: Option<bool>,
            #[serde(deserialize_with = "lenient")]
            font_size: Option<StoredFontSize>,
            #[serde(deserialize_with = "lenient")]
            animations: Option<bool>,
            #[serde(deserialize_with = "lenient")]
            high_contrast: Option<bool>,
            #[serde(deserialize_with = "lenient")]
            dyslexia_font: Option<bool>,
            #[serde(deserialize_with = "lenient")]
            reduce_motion: Option<bool>,
            #[serde(deserialize_with = "lenient")]
            text_size_a11y: Option<TextSize>,
            /// The theme of versions up to 0.2.
            #[serde(deserialize_with = "lenient")]
            theme: Option<String>,
        }

        let stored = Stored::deserialize(deserializer)?;
        let mut appearance = Appearance::default();
        if let Some(legacy) = stored.theme.filter(|_| stored.selected_theme.is_none()) {
            // "auto" switched between dark and light by the time of day; the
            // Colony themes have no such mode, so it keeps the dark default.
            // "custom" colours were hand-picked and have no Colony equivalent.
            match legacy.as_str() {
                "light" => appearance.selected_variant = "light".to_string(),
                "high-contrast" => appearance.high_contrast = true,
                _ => {}
            }
        }
        if let Some(theme) = stored.selected_theme {
            appearance.selected_theme = theme;
        }
        if let Some(variant) = stored.selected_variant {
            appearance.selected_variant = variant;
        }
        appearance.selected_accent = stored.selected_accent.filter(|key| key != "auto");
        appearance.auto_accent = stored.auto_accent.unwrap_or(appearance.auto_accent);
        appearance.font_size = stored.font_size.map_or(appearance.font_size, StoredFontSize::step);
        appearance.animations = stored.animations.unwrap_or(appearance.animations);
        appearance.high_contrast = stored.high_contrast.unwrap_or(appearance.high_contrast);
        appearance.dyslexia_font = stored.dyslexia_font.unwrap_or(appearance.dyslexia_font);
        appearance.reduce_motion = stored.reduce_motion.unwrap_or(appearance.reduce_motion);
        appearance.text_size_a11y = stored.text_size_a11y.unwrap_or(appearance.text_size_a11y);
        Ok(appearance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_toml(text: &str) -> Appearance {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn a_fresh_profile_gets_the_colony_defaults() {
        assert_eq!(from_toml(""), Appearance::default());
        let defaults = Appearance::default();
        assert_eq!((defaults.selected_theme.as_str(), defaults.selected_variant.as_str()), DEFAULT_THEME);
        assert!(defaults.animations && defaults.selected_accent.is_none());
    }

    #[test]
    fn the_themes_of_0_2_carry_over() {
        let light = from_toml(r#"theme = "light""#);
        assert_eq!((light.selected_theme.as_str(), light.selected_variant.as_str()), ("gruvbox", "light"));
        let contrast = from_toml(r#"theme = "high-contrast""#);
        assert!(contrast.high_contrast);
        assert_eq!(contrast.selected_variant, "dark");
        for dark in ["dark", "auto", "custom", "something else"] {
            let a = from_toml(&format!("theme = {dark:?}"));
            assert_eq!(a.selected_variant, "dark", "{dark}");
            assert!(!a.high_contrast, "{dark}");
        }
        // A Colony theme already chosen wins over a leftover old field.
        let chosen = from_toml("theme = \"light\"\nselected_theme = \"nord\"\nselected_variant = \"dark\"");
        assert_eq!((chosen.selected_theme.as_str(), chosen.selected_variant.as_str()), ("nord", "dark"));
    }

    #[test]
    fn pixel_font_sizes_become_the_nearest_step() {
        for (px, step) in [(10, FontSize::Small), (12, FontSize::Small), (14, FontSize::Default), (15, FontSize::Default), (16, FontSize::Large), (22, FontSize::Large)] {
            assert_eq!(from_toml(&format!("font_size = {px}")).font_size, step, "{px} px");
        }
        assert_eq!(from_toml(r#"font_size = "large""#).font_size, FontSize::Large);
    }

    #[test]
    fn a_bad_field_falls_back_alone() {
        let a = from_toml("font_size = \"huge\"\nanimations = \"maybe\"\ntext_size_a11y = \"xlarge\"\nselected_accent = \"auto\"");
        assert_eq!(a.font_size, FontSize::Default);
        assert!(a.animations);
        assert_eq!(a.text_size_a11y, TextSize::Xlarge);
        assert_eq!(a.selected_accent, None, "auto is a flag, never a colour");
    }

    #[test]
    fn colony_fields_round_trip_without_the_old_ones() {
        let a = Appearance {
            selected_theme: "catppuccin".into(),
            selected_variant: "mocha".into(),
            selected_accent: Some("violet".into()),
            font_size: FontSize::Small,
            text_size_a11y: TextSize::Large,
            dyslexia_font: true,
            ..Default::default()
        };
        let text = toml::to_string(&a).unwrap();
        assert!(text.contains("font_size = \"small\""));
        assert!(!text.lines().any(|line| line.starts_with("theme")), "{text}");
        assert_eq!(from_toml(&text), a);
        let json = serde_json::to_value(&a).unwrap();
        assert_eq!(json["text_size_a11y"], "large");
        assert_eq!(serde_json::from_value::<Appearance>(json).unwrap(), a);
    }
}
