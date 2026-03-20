//! Email provider auto-detection and server configuration.
//!
//! Given an email address, [`detect_provider`] extracts the domain and
//! returns a [`ProviderConfig`] with pre-filled IMAP/SMTP settings.
//! All host strings are `&'static str` — zero heap allocations.

/// Known email providers with built-in server configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Gmail,
    Outlook,
    Yahoo,
    ICloud,
    ProtonMail,
    Fastmail,
    Custom,
}

impl Provider {
    /// Human-readable label for the provider.
    pub fn label(self) -> &'static str {
        match self {
            Self::Gmail => "Gmail",
            Self::Outlook => "Outlook",
            Self::Yahoo => "Yahoo",
            Self::ICloud => "iCloud",
            Self::ProtonMail => "ProtonMail",
            Self::Fastmail => "Fastmail",
            Self::Custom => "Custom",
        }
    }
}

/// Authentication method used by a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    /// OAuth 2.0 (Gmail, Outlook).
    OAuth2,
    /// App-specific password (iCloud, Yahoo, ProtonMail Bridge).
    AppPassword,
    /// Standard username + password.
    Basic,
}

/// Pre-filled server configuration for a provider.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub provider: Provider,
    pub imap_host: &'static str,
    pub imap_port: u16,
    pub smtp_host: &'static str,
    pub smtp_port: u16,
    pub auth_method: AuthMethod,
}

// ── Static configurations ──────────────────────────────────────────────

static GMAIL: ProviderConfig = ProviderConfig {
    provider: Provider::Gmail,
    imap_host: "imap.gmail.com",
    imap_port: 993,
    smtp_host: "smtp.gmail.com",
    smtp_port: 587,
    auth_method: AuthMethod::OAuth2,
};

static OUTLOOK: ProviderConfig = ProviderConfig {
    provider: Provider::Outlook,
    imap_host: "outlook.office365.com",
    imap_port: 993,
    smtp_host: "smtp.office365.com",
    smtp_port: 587,
    auth_method: AuthMethod::OAuth2,
};

static YAHOO: ProviderConfig = ProviderConfig {
    provider: Provider::Yahoo,
    imap_host: "imap.mail.yahoo.com",
    imap_port: 993,
    smtp_host: "smtp.mail.yahoo.com",
    smtp_port: 587,
    auth_method: AuthMethod::AppPassword,
};

static ICLOUD: ProviderConfig = ProviderConfig {
    provider: Provider::ICloud,
    imap_host: "imap.mail.me.com",
    imap_port: 993,
    smtp_host: "smtp.mail.me.com",
    smtp_port: 587,
    auth_method: AuthMethod::AppPassword,
};

static PROTONMAIL: ProviderConfig = ProviderConfig {
    provider: Provider::ProtonMail,
    imap_host: "127.0.0.1",
    imap_port: 1143,
    smtp_host: "127.0.0.1",
    smtp_port: 1025,
    auth_method: AuthMethod::AppPassword,
};

static FASTMAIL: ProviderConfig = ProviderConfig {
    provider: Provider::Fastmail,
    imap_host: "imap.fastmail.com",
    imap_port: 993,
    smtp_host: "smtp.fastmail.com",
    smtp_port: 587,
    auth_method: AuthMethod::AppPassword,
};

static CUSTOM: ProviderConfig = ProviderConfig {
    provider: Provider::Custom,
    imap_host: "",
    imap_port: 993,
    smtp_host: "",
    smtp_port: 587,
    auth_method: AuthMethod::Basic,
};

/// Detect the email provider from an email address and return
/// a [`ProviderConfig`] with pre-filled server settings.
///
/// Extracts the domain portion after `@` and matches against
/// known providers. Returns a `Custom` config for unrecognised domains.
///
/// # Zero allocations
///
/// All returned host strings are `&'static str`. The only
/// heap work is the internal `to_ascii_lowercase()` on the domain slice.
pub fn detect_provider(email: &str) -> ProviderConfig {
    let domain = match email.rsplit_once('@') {
        Some((_, d)) => d,
        None => return CUSTOM.clone(),
    };

    // Lowercase only the domain part — small bounded allocation.
    let domain_lower = domain.to_ascii_lowercase();

    match domain_lower.as_str() {
        // Gmail
        "gmail.com" | "googlemail.com" => GMAIL.clone(),

        // Outlook / Microsoft
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" => OUTLOOK.clone(),

        // Yahoo
        "yahoo.com" | "yahoo.fr" | "ymail.com" => YAHOO.clone(),

        // iCloud / Apple
        "icloud.com" | "me.com" | "mac.com" => ICLOUD.clone(),

        // ProtonMail (requires Proton Bridge running locally)
        "protonmail.com" | "proton.me" | "pm.me" => PROTONMAIL.clone(),

        // Fastmail
        "fastmail.com" | "fastmail.fm" => FASTMAIL.clone(),

        // Unknown — return sensible defaults
        _ => CUSTOM.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_gmail() {
        let cfg = detect_provider("user@gmail.com");
        assert_eq!(cfg.provider, Provider::Gmail);
        assert_eq!(cfg.imap_host, "imap.gmail.com");
    }

    #[test]
    fn detect_outlook_hotmail() {
        let cfg = detect_provider("user@hotmail.com");
        assert_eq!(cfg.provider, Provider::Outlook);
    }

    #[test]
    fn detect_yahoo_fr() {
        let cfg = detect_provider("user@yahoo.fr");
        assert_eq!(cfg.provider, Provider::Yahoo);
    }

    #[test]
    fn detect_icloud_me() {
        let cfg = detect_provider("user@me.com");
        assert_eq!(cfg.provider, Provider::ICloud);
    }

    #[test]
    fn detect_protonmail() {
        let cfg = detect_provider("user@proton.me");
        assert_eq!(cfg.provider, Provider::ProtonMail);
    }

    #[test]
    fn detect_fastmail() {
        let cfg = detect_provider("user@fastmail.fm");
        assert_eq!(cfg.provider, Provider::Fastmail);
    }

    #[test]
    fn detect_custom() {
        let cfg = detect_provider("user@mycorp.io");
        assert_eq!(cfg.provider, Provider::Custom);
        assert_eq!(cfg.imap_port, 993);
    }

    #[test]
    fn no_at_sign() {
        let cfg = detect_provider("nope");
        assert_eq!(cfg.provider, Provider::Custom);
    }

    #[test]
    fn case_insensitive() {
        let cfg = detect_provider("User@GMAIL.COM");
        assert_eq!(cfg.provider, Provider::Gmail);
    }
}
