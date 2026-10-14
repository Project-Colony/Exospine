//! OAuth2 authentication for Google and Microsoft accounts.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::accounts::provider::Provider;

// ── OAuth2 configuration ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct OAuth2Config {
    pub client_id: String,
    pub client_secret: String,
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub scopes: &'static str,
    pub redirect_uri: &'static str,
}

pub const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const GOOGLE_SCOPES: &str = "https://mail.google.com/ https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile";

pub const MICROSOFT_AUTH_URL: &str =
    "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
pub const MICROSOFT_TOKEN_URL: &str =
    "https://login.microsoftonline.com/common/oauth2/v2.0/token";
pub const MICROSOFT_SCOPES: &str =
    "https://outlook.office365.com/IMAP.AccessAsUser.All https://outlook.office365.com/SMTP.Send openid profile email offline_access";

pub const REDIRECT_URI: &str = "http://127.0.0.1:8844/oauth/callback";

pub fn config_for_provider(
    provider: Provider,
    client_id: String,
    client_secret: String,
) -> OAuth2Config {
    match provider {
        Provider::Gmail => OAuth2Config {
            client_id,
            client_secret,
            auth_url: GOOGLE_AUTH_URL,
            token_url: GOOGLE_TOKEN_URL,
            scopes: GOOGLE_SCOPES,
            redirect_uri: REDIRECT_URI,
        },
        Provider::Outlook => OAuth2Config {
            client_id,
            client_secret,
            auth_url: MICROSOFT_AUTH_URL,
            token_url: MICROSOFT_TOKEN_URL,
            scopes: MICROSOFT_SCOPES,
            redirect_uri: REDIRECT_URI,
        },
        _ => OAuth2Config {
            client_id,
            client_secret,
            auth_url: GOOGLE_AUTH_URL,
            token_url: GOOGLE_TOKEN_URL,
            scopes: GOOGLE_SCOPES,
            redirect_uri: REDIRECT_URI,
        },
    }
}

// ── Token response ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: u64,
    #[serde(default)]
    pub token_type: String,
    /// OpenID Connect ID token, sent with the `openid` scope.
    #[serde(default)]
    pub id_token: Option<String>,
}

// ── URL helpers ──────────────────────────────────────────────────────

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(char::from(b"0123456789ABCDEF"[(b >> 4) as usize]));
                out.push(char::from(b"0123456789ABCDEF"[(b & 0x0F) as usize]));
            }
        }
    }
    out
}

// ── Authorization URL generation ─────────────────────────────────────

pub fn generate_auth_url(config: &OAuth2Config) -> (String, String) {
    let state = uuid::Uuid::new_v4().to_string();
    let url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&access_type=offline&prompt=consent",
        config.auth_url,
        url_encode(&config.client_id),
        url_encode(config.redirect_uri),
        url_encode(config.scopes),
        url_encode(&state),
    );
    (url, state)
}

// ── HTTPS POST via reqwest ────────────────────────────────────────────
//
// Security note — TLS certificate validation:
// reqwest with the `native-tls` feature validates server certificates against
// the OS certificate store by default. `danger_accept_invalid_certs(false)` is
// the default behavior — we do NOT opt out of certificate validation.
// All OAuth requests use HTTPS (no HTTP fallback).
//
// We intentionally do NOT pin specific certificate hashes because Google and
// Microsoft rotate their leaf certificates frequently. Pinning would cause
// hard-to-diagnose breakage. The native-tls chain-of-trust validation is
// sufficient for a desktop email client.

async fn https_post(url: &str, body: &str) -> Result<String> {
    tracing::info!("HTTPS POST to {}", url);

    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body.to_owned())
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .context("Token endpoint HTTP request failed")?;

    let status = response.status();
    tracing::info!("Response status: {}", status);

    let response_body = response
        .text()
        .await
        .context("Failed to read token endpoint response body")?;

    tracing::info!("Response body ({} bytes): [redacted]", response_body.len());

    Ok(response_body)
}

// ── Token exchange & refresh ─────────────────────────────────────────

pub async fn exchange_code(config: &OAuth2Config, code: &str) -> Result<TokenResponse> {
    tracing::info!("Exchanging authorization code for tokens...");

    let body = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&client_secret={}",
        url_encode(code),
        url_encode(config.redirect_uri),
        url_encode(&config.client_id),
        url_encode(&config.client_secret),
    );

    let response_body = https_post(config.token_url, &body)
        .await
        .context("Token exchange HTTP request failed")?;

    let token: TokenResponse = serde_json::from_str(&response_body)
        .context("Failed to parse token response (body redacted for security)")?;

    tracing::info!(
        "Token exchange successful, access_token len={}",
        token.access_token.len()
    );
    Ok(token)
}

pub async fn refresh_token(config: &OAuth2Config, refresh_token: &str) -> Result<TokenResponse> {
    let body = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}&client_secret={}",
        url_encode(refresh_token),
        url_encode(&config.client_id),
        url_encode(&config.client_secret),
    );

    let response_body = https_post(config.token_url, &body)
        .await
        .context("Token refresh HTTP request failed")?;

    let token: TokenResponse = serde_json::from_str(&response_body)
        .context("Failed to parse refresh response (body redacted for security)")?;

    Ok(token)
}

// ── User info ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct OAuthUserInfo {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub name: String,
}

/// Read the account address and name from the ID token that Microsoft sends
/// with the `openid profile email` scopes. The access token is for
/// outlook.office365.com, so Microsoft Graph would reject it. The ID token
/// comes straight from the token endpoint over TLS, which OpenID Connect
/// accepts in place of checking its signature.
pub fn microsoft_userinfo_from_id_token(id_token: &str) -> Result<OAuthUserInfo> {
    use base64::Engine as _;

    #[derive(Deserialize)]
    struct Claims {
        #[serde(default)]
        email: String,
        #[serde(default)]
        preferred_username: String,
        #[serde(default)]
        name: String,
    }

    let payload = id_token.split('.').nth(1).context("The ID token is not a JWT")?;
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .context("Failed to decode the ID token")?;
    let claims: Claims =
        serde_json::from_slice(&payload).context("Failed to parse the ID token claims")?;

    // preferred_username can be a phone number on consumer accounts: only an
    // address is usable, otherwise the caller reports the missing email.
    let email = if claims.email.is_empty() && claims.preferred_username.contains('@') {
        claims.preferred_username
    } else {
        claims.email
    };
    Ok(OAuthUserInfo { email, name: claims.name })
}

pub async fn fetch_google_userinfo(access_token: &str) -> Result<OAuthUserInfo> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .header("Authorization", format!("Bearer {}", access_token))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .context("Failed to fetch Google userinfo")?;

    let status = response.status();
    let body = response
        .text()
        .await
        .context("Failed to read userinfo response body")?;

    if !status.is_success() {
        anyhow::bail!("Google userinfo endpoint returned {}: {}", status, body);
    }

    let info: OAuthUserInfo = serde_json::from_str(&body)
        .with_context(|| format!("Failed to parse userinfo response: {}", body))?;

    tracing::info!("Fetched userinfo: email={}, name={}", info.email, info.name);
    Ok(info)
}
