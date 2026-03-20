//! OAuth2 browser-based authorization flow.
//!
//! Opens the user's default browser to the provider's consent page,
//! then listens on a local HTTP server (`127.0.0.1:8844`) for the
//! redirect callback carrying the authorization code.

use anyhow::{bail, Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::accounts::oauth2::{self, TokenResponse};
use crate::accounts::provider::Provider;

/// Run the full OAuth2 authorization-code flow for the given provider.
///
/// 1. Builds the authorization URL with a random state nonce.
/// 2. Opens the user's browser to the consent page.
/// 3. Starts a single-shot HTTP listener on `127.0.0.1:8844`.
/// 4. Receives the redirect with `?code=...&state=...`.
/// 5. Exchanges the code for tokens at the provider's token endpoint.
///
/// Returns the [`TokenResponse`] containing access and refresh tokens.
pub async fn start_oauth_flow(
    provider: Provider,
    client_id: String,
) -> Result<TokenResponse> {
    let config = oauth2::config_for_provider(provider, client_id);

    let (auth_url, expected_state) = oauth2::generate_auth_url(&config);

    // Open the browser.
    open_browser(&auth_url);

    // Wait for the callback.
    let code = listen_for_callback(&expected_state).await?;

    // Exchange the authorization code for tokens.
    let token = oauth2::exchange_code(&config, &code).await?;

    Ok(token)
}

/// Open a URL in the user's default browser.
fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
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

/// Listen on `127.0.0.1:8844` for a single HTTP GET from the OAuth
/// redirect, extract the `code` query parameter, send a success page
/// back to the browser, and return the code.
async fn listen_for_callback(expected_state: &str) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:8844")
        .await
        .context("Failed to bind local OAuth callback server on 127.0.0.1:8844")?;

    tracing::info!("OAuth callback server listening on 127.0.0.1:8844");

    // Accept one connection (with a generous timeout).
    let (mut stream, _addr) = tokio::time::timeout(
        std::time::Duration::from_secs(300), // 5 minutes for user to consent
        listener.accept(),
    )
    .await
    .context("Timed out waiting for OAuth callback")?
    .context("Failed to accept OAuth callback connection")?;

    // Read the HTTP request.
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await.context("Failed to read callback request")?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Parse the first line: "GET /oauth/callback?code=...&state=... HTTP/1.1"
    let first_line = request.lines().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("");

    // Extract query string.
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

    // Check for errors from the provider.
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
        // Send error page.
        let html = format!(
            "<html><body><h2>Authentication Failed</h2><p>{}: {}</p>\
             <p>You can close this tab.</p></body></html>",
            err, description
        );
        let response = format!(
            "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        );
        let _ = stream.write_all(response.as_bytes()).await;
        bail!("OAuth error: {} — {}", err, description);
    }

    // Validate state.
    if let Some(ref s) = state {
        if s != expected_state {
            let html = "<html><body><h2>State Mismatch</h2>\
                        <p>Security check failed. Please try again.</p></body></html>";
            let response = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html.len(),
                html
            );
            let _ = stream.write_all(response.as_bytes()).await;
            bail!("OAuth state mismatch: expected {}, got {}", expected_state, s);
        }
    }

    let code = code.context("No authorization code in OAuth callback")?;

    // Send a friendly success page.
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

/// Parse a URL query string like `code=abc&state=xyz` into key-value pairs.
/// Handles percent-decoding of values.
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

/// Simple percent-decoding for URL query values.
fn percent_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                &s[i + 1..i + 3],
                16,
            ) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_string() {
        let params = parse_query_string("code=abc123&state=xyz789&extra=hello%20world");
        assert_eq!(params.len(), 3);
        assert_eq!(params[0], ("code".into(), "abc123".into()));
        assert_eq!(params[1], ("state".into(), "xyz789".into()));
        assert_eq!(params[2], ("extra".into(), "hello world".into()));
    }

    #[test]
    fn test_parse_query_string_empty() {
        let params = parse_query_string("");
        assert!(params.is_empty());
    }

    #[test]
    fn test_percent_decode() {
        assert_eq!(percent_decode("hello%20world"), "hello world");
        assert_eq!(percent_decode("foo+bar"), "foo bar");
        assert_eq!(percent_decode("100%25"), "100%");
    }
}
