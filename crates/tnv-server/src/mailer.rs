// src/mailer.rs — pluggable mailer for verification codes.
//
// LogMailer (dev): emits a one-time startup warning, then writes codes
// at debug level. Never at info — production-grade log aggregators
// frequently route info to permanent storage. Selected when SMTP_HOST
// is empty.
//
// SmtpMailer: real delivery via lettre over a rustls-secured connection
// (implicit TLS on 465, STARTTLS otherwise). Selected when SMTP_HOST is set.
// A send failure returns Err so callers can fail closed.

use async_trait::async_trait;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

#[async_trait]
pub trait Mailer: Send + Sync {
    async fn send_verification(&self, email: &str, code: &str) -> Result<(), String>;
}

pub struct LogMailer;

impl LogMailer {
    pub fn new() -> Self {
        tracing::warn!(
            "LogMailer active: verification codes will be written to debug logs only. \
             Configure SMTP_HOST in .env to enable real email delivery."
        );
        Self
    }
}

#[async_trait]
impl Mailer for LogMailer {
    async fn send_verification(&self, email: &str, code: &str) -> Result<(), String> {
        tracing::debug!(target: "tnv_server::mailer", "verification code for {email}: {code}");
        Ok(())
    }
}

pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: String,
}

impl SmtpMailer {
    /// Build a TLS SMTP transport. Returns Err if host/from are unusable so
    /// startup can fail rather than silently dropping mail.
    pub fn new(
        host: String,
        port: u16,
        user: String,
        pass: String,
        from: String,
    ) -> Result<Self, String> {
        // Validate the From address up front.
        from.parse::<lettre::message::Mailbox>()
            .map_err(|e| format!("invalid SMTP_FROM '{from}': {e}"))?;

        // Port 465 → implicit TLS; everything else → STARTTLS upgrade.
        let mut builder = if port == 465 {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&host)
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)
        }
        .map_err(|e| format!("SMTP transport for {host}:{port}: {e}"))?
        .port(port);

        if !user.is_empty() {
            builder = builder.credentials(Credentials::new(user, pass));
        }

        tracing::info!("SMTP mailer configured for {host}:{port} (from {from})");
        Ok(Self { transport: builder.build(), from })
    }
}

#[async_trait]
impl Mailer for SmtpMailer {
    async fn send_verification(&self, email: &str, code: &str) -> Result<(), String> {
        let to = email.parse::<lettre::message::Mailbox>()
            .map_err(|e| format!("invalid recipient: {e}"))?;
        let from = self.from.parse::<lettre::message::Mailbox>()
            .map_err(|e| format!("invalid from: {e}"))?;

        let message = Message::builder()
            .from(from)
            .to(to)
            .subject("Your TNV verification code")
            .header(ContentType::TEXT_PLAIN)
            .body(format!(
                "Your Tax N Vote verification code is: {code}\n\n\
                 It expires in 15 minutes. If you did not request this, ignore this email."
            ))
            .map_err(|e| format!("build email: {e}"))?;

        self.transport.send(message).await
            .map(|_| ())
            // Don't echo the full SMTP error (may contain server banners); log it.
            .map_err(|e| {
                tracing::error!("SMTP send failed: {e}");
                "failed to send verification email".to_string()
            })
    }
}

/// Build a mailer from environment variables. SMTP_HOST empty → LogMailer.
/// Returns Err if SMTP is configured but the transport can't be built, so the
/// server fails to start rather than silently dropping verification email.
pub fn from_env() -> Result<std::sync::Arc<dyn Mailer>, String> {
    let host = std::env::var("SMTP_HOST").unwrap_or_default();
    if host.is_empty() {
        return Ok(std::sync::Arc::new(LogMailer::new()));
    }
    let port: u16 = std::env::var("SMTP_PORT").ok()
        .and_then(|s| s.parse().ok()).unwrap_or(587);
    let user = std::env::var("SMTP_USER").unwrap_or_default();
    let pass = std::env::var("SMTP_PASS").unwrap_or_default();
    let from = std::env::var("SMTP_FROM").unwrap_or_else(|_| "noreply@taxnvote.org".into());
    Ok(std::sync::Arc::new(SmtpMailer::new(host, port, user, pass, from)?))
}
