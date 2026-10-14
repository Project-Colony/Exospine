//! The few strings the backend shows itself (desktop notifications and the
//! window title), in the interface language the web UI reports through
//! `set_ui_language`. Every string is written in both languages at its call
//! site, so neither can be missing.

use std::sync::atomic::{AtomicBool, Ordering};

static FRENCH: AtomicBool = AtomicBool::new(false);

/// Follow the interface language: "fr", or English for anything else.
pub fn set_language(lang: &str) {
    FRENCH.store(lang.starts_with("fr"), Ordering::Relaxed);
}

/// `en` or `fr`, whichever the interface uses.
pub fn tr(en: &'static str, fr: &'static str) -> &'static str {
    if FRENCH.load(Ordering::Relaxed) {
        fr
    } else {
        en
    }
}

/// The interface language the web UI resolved (from its preference or the
/// system), for the strings the backend shows itself.
#[tauri::command]
pub fn set_ui_language(language: String) {
    set_language(&language);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_follow_the_interface_language() {
        set_language("fr");
        assert_eq!(tr("New email", "Nouveau message"), "Nouveau message");
        set_language("en");
        assert_eq!(tr("New email", "Nouveau message"), "New email");
        set_language("de");
        assert_eq!(tr("New email", "Nouveau message"), "New email");
    }
}
