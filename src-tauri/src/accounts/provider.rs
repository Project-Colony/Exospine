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

/// Server configuration of a known provider, whatever the address domain
/// (a Google Workspace or Microsoft 365 address can use any domain).
pub fn config_for(provider: Provider) -> ProviderConfig {
    match provider {
        Provider::Gmail => GMAIL.clone(),
        Provider::Outlook => OUTLOOK.clone(),
        Provider::Yahoo => YAHOO.clone(),
        Provider::ICloud => ICLOUD.clone(),
        Provider::ProtonMail => PROTONMAIL.clone(),
        Provider::Fastmail => FASTMAIL.clone(),
        Provider::Custom => CUSTOM.clone(),
    }
}

/// Build an OAuth2Config for the given account.
/// Returns `None` if the account is not OAuth2.
pub fn oauth2_config_for_account(
    account: &crate::app_state::Account,
    config: &crate::config::Config,
) -> Option<crate::accounts::oauth2::OAuth2Config> {
    if !matches!(account.auth_method, crate::app_state::AuthMethod::OAuth2) {
        return None;
    }
    let (provider, client_id, client_secret) = if is_microsoft_oauth(account) {
        (Provider::Outlook, &config.microsoft_client_id, &config.microsoft_client_secret)
    } else {
        (Provider::Gmail, &config.google_client_id, &config.google_client_secret)
    };
    Some(crate::accounts::oauth2::config_for_provider(
        provider,
        client_id.clone(),
        client_secret.clone(),
    ))
}

/// Whether an OAuth2 account signed in with Microsoft. The address domain
/// does not tell a Microsoft 365 address on a custom domain apart, but the
/// IMAP host set at sign-in does.
fn is_microsoft_oauth(account: &crate::app_state::Account) -> bool {
    detect_provider(&account.email).provider == Provider::Outlook
        || account.imap_host == OUTLOOK.imap_host
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::{Account, AuthMethod};

    fn oauth_account(email: &str, provider: Provider) -> Account {
        Account {
            email: email.into(),
            imap_host: config_for(provider).imap_host.into(),
            auth_method: AuthMethod::OAuth2,
            ..Default::default()
        }
    }

    /// A Microsoft 365 or Google Workspace address on a custom domain
    /// refreshes its token with the provider it signed in with.
    #[test]
    fn oauth_refresh_follows_the_sign_in_provider() {
        let config = crate::config::Config::default();
        let token_url = |account: &Account| {
            oauth2_config_for_account(account, &config).unwrap().token_url
        };
        let microsoft = token_url(&oauth_account("ada@outlook.com", Provider::Outlook));
        let google = token_url(&oauth_account("ada@gmail.com", Provider::Gmail));
        assert_ne!(microsoft, google);
        assert_eq!(token_url(&oauth_account("ada@corp.example", Provider::Outlook)), microsoft);
        assert_eq!(token_url(&oauth_account("ada@corp.example", Provider::Gmail)), google);
        assert!(oauth2_config_for_account(&Account::default(), &config).is_none());
    }
}
