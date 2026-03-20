//! OAuth2 authentication for Google and Microsoft accounts.
//!
//! Provides configuration, token exchange, token refresh, and XOAUTH2
//! SASL string formatting. Uses a minimal HTTPS client built on
//! `tokio::net::TcpStream` + `tokio_rustls` for token endpoint requests.

use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use crate::accounts::provider::Provider;

// ── Base64 encoder (encode-only, standard alphabet + padding) ────────

const B64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Minimal base64 encoder — standard alphabet with `=` padding.
pub fn base64_encode(input: &[u8]) -> String {
    let mut out = Vec::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(B64_ALPHABET[((triple >> 18) & 0x3F) as usize]);
        out.push(B64_ALPHABET[((triple >> 12) & 0x3F) as usize]);

        if chunk.len() > 1 {
            out.push(B64_ALPHABET[((triple >> 6) & 0x3F) as usize]);
        } else {
            out.push(b'=');
        }

        if chunk.len() > 2 {
            out.push(B64_ALPHABET[(triple & 0x3F) as usize]);
        } else {
            out.push(b'=');
        }
    }
    // SAFETY: B64_ALPHABET and '=' are all valid ASCII/UTF-8.
    unsafe { String::from_utf8_unchecked(out) }
}

// ── OAuth2 configuration ─────────────────────────────────────────────

/// OAuth2 endpoint configuration for a provider.
#[derive(Debug, Clone)]
pub struct OAuth2Config {
    pub client_id: String,
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub scopes: &'static str,
    pub redirect_uri: &'static str,
}

/// Pre-configured OAuth2 settings for Google (Gmail).
///
/// The `client_id` must be set by the user or compiled-in from build config.
/// Here we use a placeholder that should be replaced with a real Desktop
/// OAuth client ID.
pub const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const GOOGLE_SCOPES: &str = "https://mail.google.com/";

pub const MICROSOFT_AUTH_URL: &str =
    "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
pub const MICROSOFT_TOKEN_URL: &str =
    "https://login.microsoftonline.com/common/oauth2/v2.0/token";
pub const MICROSOFT_SCOPES: &str =
    "https://outlook.office365.com/IMAP.AccessAsUser.All https://outlook.office365.com/SMTP.Send offline_access";

pub const REDIRECT_URI: &str = "http://127.0.0.1:8844/oauth/callback";

/// Build an [`OAuth2Config`] for the given provider.
///
/// `client_id` must be supplied by the caller (loaded from config / env).
pub fn config_for_provider(provider: Provider, client_id: String) -> OAuth2Config {
    match provider {
        Provider::Gmail => OAuth2Config {
            client_id,
            auth_url: GOOGLE_AUTH_URL,
            token_url: GOOGLE_TOKEN_URL,
            scopes: GOOGLE_SCOPES,
            redirect_uri: REDIRECT_URI,
        },
        Provider::Outlook => OAuth2Config {
            client_id,
            auth_url: MICROSOFT_AUTH_URL,
            token_url: MICROSOFT_TOKEN_URL,
            scopes: MICROSOFT_SCOPES,
            redirect_uri: REDIRECT_URI,
        },
        _ => OAuth2Config {
            client_id,
            auth_url: GOOGLE_AUTH_URL,
            token_url: GOOGLE_TOKEN_URL,
            scopes: GOOGLE_SCOPES,
            redirect_uri: REDIRECT_URI,
        },
    }
}

// ── Token response ───────────────────────────────────────────────────

/// Response from an OAuth2 token endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: u64,
    #[serde(default)]
    pub token_type: String,
}

// ── URL helpers ──────────────────────────────────────────────────────

/// Percent-encode a string for use in URL query parameters.
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

/// Generate the browser authorization URL and a random state nonce.
///
/// Returns `(full_auth_url, state_nonce)`.
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

// ── Minimal HTTPS POST client ────────────────────────────────────────

/// Extract `(host, path)` from a token URL like `https://host/path`.
fn parse_https_url(url: &str) -> Result<(&str, &str)> {
    let rest = url
        .strip_prefix("https://")
        .context("Token URL must be HTTPS")?;
    let (host, path) = rest
        .split_once('/')
        .map(|(h, p)| (h, p))
        .unwrap_or((rest, ""));
    Ok((host, path))
}

/// Perform an HTTPS POST with `application/x-www-form-urlencoded` body
/// and return the raw response body as a `String`.
async fn https_post(url: &str, body: &str) -> Result<String> {
    let (host, path) = parse_https_url(url)?;

    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store())
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(tls_config));

    let addr = format!("{}:443", host);
    let tcp = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("Failed to connect to {}", addr))?;

    let server_name: rustls::pki_types::ServerName<'static> = host
        .to_owned()
        .try_into()
        .context("Invalid server name for token endpoint")?;

    let mut tls = connector
        .connect(server_name, tcp)
        .await
        .context("TLS handshake failed for token endpoint")?;

    // Build HTTP/1.1 request.
    let request = format!(
        "POST /{path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Content-Type: application/x-www-form-urlencoded\r\n\
         Content-Length: {len}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        path = path,
        host = host,
        len = body.len(),
        body = body,
    );

    tls.write_all(request.as_bytes()).await?;
    tls.flush().await?;

    let mut response = Vec::new();
    tls.read_to_end(&mut response).await?;

    let response_str = String::from_utf8_lossy(&response).to_string();

    // Split headers from body on the first \r\n\r\n.
    let body_start = response_str
        .find("\r\n\r\n")
        .map(|i| i + 4)
        .unwrap_or(0);
    let response_body = &response_str[body_start..];

    // Handle chunked transfer encoding: if the body starts with a hex
    // chunk size, strip it and trailing markers.
    let response_body = if response_str.contains("Transfer-Encoding: chunked") {
        decode_chunked(response_body)
    } else {
        response_body.to_string()
    };

    Ok(response_body)
}

/// Simple chunked transfer-encoding decoder.
fn decode_chunked(raw: &str) -> String {
    let mut result = String::new();
    let mut remaining = raw;
    loop {
        let remaining_trimmed = remaining.trim_start();
        if remaining_trimmed.is_empty() {
            break;
        }
        let line_end = remaining_trimmed.find("\r\n").unwrap_or(remaining_trimmed.len());
        let size_str = &remaining_trimmed[..line_end];
        let size = usize::from_str_radix(size_str.trim(), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        let data_start = line_end + 2; // skip \r\n after size
        if data_start + size <= remaining_trimmed.len() {
            result.push_str(&remaining_trimmed[data_start..data_start + size]);
            remaining = &remaining_trimmed[data_start + size..];
            // skip trailing \r\n of chunk
            if remaining.starts_with("\r\n") {
                remaining = &remaining[2..];
            }
        } else {
            // Incomplete chunk — take what we have
            result.push_str(&remaining_trimmed[data_start..]);
            break;
        }
    }
    result
}

fn root_store() -> rustls::RootCertStore {
    let mut store = rustls::RootCertStore::empty();
    store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    store
}

// ── Token exchange & refresh ─────────────────────────────────────────

/// Exchange an authorization code for tokens.
pub async fn exchange_code(config: &OAuth2Config, code: &str) -> Result<TokenResponse> {
    let body = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}",
        url_encode(code),
        url_encode(config.redirect_uri),
        url_encode(&config.client_id),
    );

    let response_body = https_post(config.token_url, &body)
        .await
        .context("Token exchange HTTP request failed")?;

    let token: TokenResponse = serde_json::from_str(&response_body)
        .with_context(|| format!("Failed to parse token response: {}", response_body))?;

    Ok(token)
}

/// Refresh an expired access token using a refresh token.
pub async fn refresh_token(config: &OAuth2Config, refresh_token: &str) -> Result<TokenResponse> {
    let body = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}",
        url_encode(refresh_token),
        url_encode(&config.client_id),
    );

    let response_body = https_post(config.token_url, &body)
        .await
        .context("Token refresh HTTP request failed")?;

    let token: TokenResponse = serde_json::from_str(&response_body)
        .with_context(|| format!("Failed to parse refresh response: {}", response_body))?;

    Ok(token)
}

// ── XOAUTH2 SASL string ─────────────────────────────────────────────

/// Format the XOAUTH2 SASL authentication string.
///
/// The format is: `base64("user=" + user + "\x01auth=Bearer " + token + "\x01\x01")`
pub fn xoauth2_token(user: &str, access_token: &str) -> String {
    let raw = format!("user={}\x01auth=Bearer {}\x01\x01", user, access_token);
    base64_encode(raw.as_bytes())
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(b"Hello, World!"), "SGVsbG8sIFdvcmxkIQ==");
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("foo@bar.com"), "foo%40bar.com");
        assert_eq!(url_encode("a+b=c"), "a%2Bb%3Dc");
    }

    #[test]
    fn test_xoauth2_token() {
        let token = xoauth2_token("user@example.com", "ya29.token123");
        let decoded_bytes = {
            // Verify the base64 decodes to expected format
            let raw = format!(
                "user=user@example.com\x01auth=Bearer ya29.token123\x01\x01"
            );
            base64_encode(raw.as_bytes())
        };
        assert_eq!(token, decoded_bytes);
    }

    #[test]
    fn test_generate_auth_url_google() {
        let config = config_for_provider(Provider::Gmail, "test-client-id".to_string());
        let (url, state) = generate_auth_url(&config);
        assert!(url.starts_with("https://accounts.google.com/"));
        assert!(url.contains("client_id=test-client-id"));
        assert!(url.contains(&state));
        assert!(!state.is_empty());
    }

    #[test]
    fn test_generate_auth_url_microsoft() {
        let config = config_for_provider(Provider::Outlook, "ms-client-id".to_string());
        let (url, state) = generate_auth_url(&config);
        assert!(url.starts_with("https://login.microsoftonline.com/"));
        assert!(url.contains("client_id=ms-client-id"));
        assert!(url.contains(&state));
    }

    #[test]
    fn test_parse_https_url() {
        let (host, path) = parse_https_url("https://oauth2.googleapis.com/token").unwrap();
        assert_eq!(host, "oauth2.googleapis.com");
        assert_eq!(path, "token");
    }

    #[test]
    fn test_decode_chunked() {
        let raw = "a\r\n0123456789\r\n5\r\nhello\r\n0\r\n\r\n";
        assert_eq!(decode_chunked(raw), "0123456789hello");
    }
}
