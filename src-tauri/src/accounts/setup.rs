//! Account setup: connection testing and account creation.

use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use crate::accounts::keyring_store;
use crate::accounts::provider::detect_provider;
use crate::app_state::Account;

/// Validate an IMAP connection by logging in over TLS and immediately logging out.
pub async fn test_imap_connection(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
) -> Result<()> {
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store())
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(tls_config));

    let addr = format!("{}:{}", host, port);
    let tcp = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("Cannot reach IMAP server at {}", addr))?;

    let server_name: rustls::pki_types::ServerName<'static> = host
        .to_owned()
        .try_into()
        .context("Invalid IMAP server name")?;

    let tls_stream = connector
        .connect(server_name, tcp)
        .await
        .context("IMAP TLS handshake failed")?;

    let client = async_imap::Client::new(tls_stream);

    let mut session = client
        .login(username, password)
        .await
        .map_err(|e| anyhow::anyhow!("IMAP login failed: {:?}", e.0))?;

    session.logout().await.context("IMAP logout failed")?;

    tracing::info!("IMAP connection test passed for {}", host);
    Ok(())
}

/// Validate an SMTP connection.
pub async fn test_smtp_connection(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
) -> Result<()> {
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, Tokio1Executor};

    let creds = Credentials::new(username.to_owned(), password.to_owned());

    let mailer: AsyncSmtpTransport<Tokio1Executor> =
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
            .with_context(|| format!("Cannot create SMTP transport for {}", host))?
            .credentials(creds)
            .port(port)
            .build();

    mailer
        .test_connection()
        .await
        .with_context(|| format!("SMTP connection test failed for {}:{}", host, port))?;

    tracing::info!("SMTP connection test passed for {}", host);
    Ok(())
}

/// Create a fully validated [`Account`].
pub async fn create_account(
    name: &str,
    email: &str,
    password: &str,
    imap_host: &str,
    imap_port: u16,
    smtp_host: &str,
    smtp_port: u16,
) -> Result<Account> {
    let (imap_result, smtp_result) = tokio::join!(
        test_imap_connection(imap_host, imap_port, email, password),
        test_smtp_connection(smtp_host, smtp_port, email, password),
    );

    imap_result.context("IMAP connection test failed")?;
    smtp_result.context("SMTP connection test failed")?;

    let id = uuid::Uuid::new_v4().to_string();

    keyring_store::store_password(&id, password)
        .context("Failed to store password securely")?;

    let _provider_config = detect_provider(email);

    let account = Account {
        id,
        name: name.to_owned(),
        email: email.to_owned(),
        imap_host: imap_host.to_owned(),
        imap_port,
        smtp_host: smtp_host.to_owned(),
        smtp_port,
        username: email.to_owned(),
        use_tls: true,
        auth_method: crate::app_state::AuthMethod::Basic,
        folders: Vec::new(),
        signature: String::new(),
        signature_html: None,
    };

    tracing::info!("Account created: {} <{}>", account.name, account.email);
    Ok(account)
}

/// Create an [`Account`] for an OAuth2-authenticated provider.
pub async fn create_oauth_account(
    name: &str,
    email: &str,
    access_token: &str,
    refresh_token: &str,
    imap_host: &str,
    imap_port: u16,
    smtp_host: &str,
    smtp_port: u16,
) -> Result<Account> {
    let id = uuid::Uuid::new_v4().to_string();

    keyring_store::store_password(&id, access_token)
        .context("Failed to store OAuth access token securely")?;

    let account = Account {
        id,
        name: name.to_owned(),
        email: email.to_owned(),
        imap_host: imap_host.to_owned(),
        imap_port,
        smtp_host: smtp_host.to_owned(),
        smtp_port,
        username: email.to_owned(),
        use_tls: true,
        auth_method: crate::app_state::AuthMethod::OAuth2 {
            refresh_token: refresh_token.to_owned(),
        },
        folders: Vec::new(),
        signature: String::new(),
        signature_html: None,
    };

    tracing::info!("OAuth account created: {} <{}>", account.name, account.email);
    Ok(account)
}

fn root_store() -> rustls::RootCertStore {
    let mut store = rustls::RootCertStore::empty();
    store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    store
}
