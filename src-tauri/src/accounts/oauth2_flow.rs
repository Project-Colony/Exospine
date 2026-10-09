//! OAuth2 browser-based authorization flow.

use anyhow::{bail, Context, Result};
use std::sync::Mutex;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::accounts::oauth2::{self, TokenResponse};
use crate::accounts::provider::Provider;

// ── Rate limiting for the OAuth callback server ──────────────────────

/// Maximum requests allowed within the rate-limit window.
const RATE_LIMIT_MAX: usize = 10;
/// Rate-limit window duration in seconds.
const RATE_LIMIT_WINDOW_SECS: u64 = 60;

static RATE_LIMITER: Mutex<Option<RateLimiter>> = Mutex::new(None);

struct RateLimiter {
    timestamps: Vec<Instant>,
}

impl RateLimiter {
    fn new() -> Self {
        Self {
            timestamps: Vec::new(),
        }
    }

    /// Record a request and return true if the request is allowed.
    fn check_and_record(&mut self) -> bool {
        let now = Instant::now();
        let window = std::time::Duration::from_secs(RATE_LIMIT_WINDOW_SECS);

        // Remove timestamps outside the window
        self.timestamps.retain(|t| now.duration_since(*t) < window);

        if self.timestamps.len() >= RATE_LIMIT_MAX {
            false
        } else {
            self.timestamps.push(now);
            true
        }
    }
}

fn check_rate_limit() -> bool {
    let mut guard = crate::app_state::lock_or_recover(&RATE_LIMITER);
    let limiter = guard.get_or_insert_with(RateLimiter::new);
    limiter.check_and_record()
}

/// Run the full OAuth2 authorization-code flow for the given provider.
pub async fn start_oauth_flow(
    provider: Provider,
    client_id: String,
    client_secret: String,
) -> Result<TokenResponse> {
    let config = oauth2::config_for_provider(provider, client_id, client_secret);

    let (auth_url, expected_state) = oauth2::generate_auth_url(&config);

    open_browser(&auth_url);

    let code = listen_for_callback(&expected_state).await?;

    let token = oauth2::exchange_code(&config, &code).await?;

    Ok(token)
}

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    tracing::info!("Opened browser for OAuth2 consent");
}

async fn listen_for_callback(expected_state: &str) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:8844")
        .await
        .context("Failed to bind local OAuth callback server on 127.0.0.1:8844")?;

    tracing::info!("OAuth callback server listening on 127.0.0.1:8844");

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(300);

    // Accept connections in a loop, applying rate limiting.
    // Only the first valid OAuth callback is processed; excess requests get 429.
    let (mut stream, request) = loop {
        let accept_result = tokio::time::timeout_at(deadline, listener.accept()).await;

        let (mut s, _addr) = accept_result
            .context("Timed out waiting for OAuth callback")?
            .context("Failed to accept OAuth callback connection")?;

        // Rate-limit check
        if !check_rate_limit() {
            tracing::warn!("OAuth callback rate limit exceeded, rejecting request");
            let body = "429 Too Many Requests";
            let response = format!(
                "HTTP/1.1 429 Too Many Requests\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nRetry-After: 60\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = s.write_all(response.as_bytes()).await;
            continue;
        }

        let mut buf = vec![0u8; 4096];
        let n = s
            .read(&mut buf)
            .await
            .context("Failed to read callback request")?;
        let req = String::from_utf8_lossy(&buf[..n]).to_string();

        // If this looks like a valid HTTP request with a query string, process it
        if req.starts_with("GET ") {
            break (s, req);
        }

        // Not a GET request, ignore and wait for the next connection
        let _ = s.write_all(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n").await;
    };

    let first_line = request.lines().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("");

    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
    let params = parse_query_string(query);

    let code = params
        .iter()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.clone());

    let state = params
        .iter()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.clone());

    let error = params
        .iter()
        .find(|(k, _)| k == "error")
        .map(|(_, v)| v.clone());

    if let Some(err) = error {
        let description = params
            .iter()
            .find(|(k, _)| k == "error_description")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        let html = format!(
            "<html><body><h2>Authentication Failed</h2><p>{}: {}</p>\
             <p>You can close this tab.</p></body></html>",
            ammonia::clean_text(&err),
            ammonia::clean_text(&description)
        );
        let response = format!(
            "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        );
        let _ = stream.write_all(response.as_bytes()).await;
        bail!("OAuth error: {} -- {}", err, description);
    }

    match state {
        Some(ref s) if s == expected_state => { /* OK */ }
        Some(ref s) => {
            let html = "<html><body><h2>State Mismatch</h2>\
                        <p>Security check failed. Please try again.</p></body></html>";
            let response = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html.len(),
                html
            );
            let _ = stream.write_all(response.as_bytes()).await;
            bail!(
                "OAuth state mismatch: expected {}, got {}",
                expected_state,
                s
            );
        }
        None => {
            let html = "<html><body><h2>Missing State Parameter</h2>\
                        <p>Security check failed. Please try again.</p></body></html>";
            let response = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html.len(),
                html
            );
            let _ = stream.write_all(response.as_bytes()).await;
            bail!("OAuth callback missing state parameter (possible CSRF)");
        }
    }

    let code = code.context("No authorization code in OAuth callback")?;

    let html = "<html><body><h2>Authentication Successful!</h2>\
                <p>You can close this tab and return to Exospine.</p></body></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    let _ = stream.write_all(response.as_bytes()).await;

    Ok(code)
}

fn parse_query_string(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((k.to_string(), percent_decode(v)))
        })
        .collect()
}

fn percent_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}
