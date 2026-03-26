//! Email provider auto-detection and server configuration.

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
    OAuth2,
    AppPassword,
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

/// Build an OAuth2Config for the given account based on its email domain.
/// Returns `None` if the account is not OAuth2.
pub fn oauth2_config_for_account(
    account: &crate::app_state::Account,
    config: &crate::config::Config,
) -> Option<crate::accounts::oauth2::OAuth2Config> {
    if !matches!(account.auth_method, crate::app_state::AuthMethod::OAuth2 { .. }) {
        return None;
    }
    let provider_cfg = detect_provider(&account.email);
    let (client_id, client_secret) = match provider_cfg.provider {
        Provider::Gmail => (config.google_client_id.clone(), config.google_client_secret.clone()),
        Provider::Outlook => (config.microsoft_client_id.clone(), config.microsoft_client_secret.clone()),
        _ => (config.google_client_id.clone(), config.google_client_secret.clone()),
    };
    Some(crate::accounts::oauth2::config_for_provider(
        provider_cfg.provider,
        client_id,
        client_secret,
    ))
}

/// Detect the email provider from an email address.
///
/// Exchange Online / Office 365 accounts use the same IMAP/SMTP servers and
/// OAuth2 endpoints as consumer Outlook.com (`outlook.office365.com`), so any
/// domain hosted on Microsoft 365 is mapped to the Outlook provider config.
pub fn detect_provider(email: &str) -> ProviderConfig {
    let domain = match email.rsplit_once('@') {
        Some((_, d)) => d,
        None => return CUSTOM.clone(),
    };

    let domain_lower = domain.to_ascii_lowercase();

    match domain_lower.as_str() {
        "gmail.com" | "googlemail.com" => GMAIL.clone(),
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" => OUTLOOK.clone(),
        "yahoo.com" | "yahoo.fr" | "ymail.com" => YAHOO.clone(),
        "icloud.com" | "me.com" | "mac.com" => ICLOUD.clone(),
        "protonmail.com" | "proton.me" | "pm.me" => PROTONMAIL.clone(),
        "fastmail.com" | "fastmail.fm" => FASTMAIL.clone(),
        _ => {
            // Heuristic: domains ending with common Exchange Online / Microsoft 365
            // patterns are treated as Outlook (same IMAP host and OAuth2 endpoints).
            if domain_lower.ends_with(".onmicrosoft.com")
                || domain_lower.ends_with(".mail.onmicrosoft.com")
            {
                OUTLOOK.clone()
            } else {
                CUSTOM.clone()
            }
        }
    }
}
