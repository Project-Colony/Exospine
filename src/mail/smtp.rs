use anyhow::{Context, Result};
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::state::{Account, ComposeDraft};

/// Send an email using SMTP with TLS.
pub async fn send_mail(account: &Account, password: &str, draft: &ComposeDraft) -> Result<()> {
    let email = Message::builder()
        .from(
            account
                .email
                .parse()
                .context("Invalid sender address")?,
        )
        .to(draft.to.parse().context("Invalid recipient address")?)
        .subject(&draft.subject)
        .header(ContentType::TEXT_PLAIN)
        .body(draft.body.clone())
        .context("Failed to build email message")?;

    let creds = Credentials::new(account.username.clone(), password.to_string());

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&account.smtp_host)
        .context("Failed to create SMTP transport")?
        .credentials(creds)
        .port(account.smtp_port)
        .build();

    mailer
        .send(email)
        .await
        .context("Failed to send email")?;

    tracing::info!("Email sent to {}", draft.to);
    Ok(())
}
