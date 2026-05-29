// src/mailer.rs — pluggable mailer for verification codes.
//
// LogMailer (dev): emits a one-time startup warning, then writes codes
// at debug level. Never at info — production-grade log aggregators
// frequently route info to permanent storage.
//
// SmtpMailer: stub. Real SMTP wiring is intentionally deferred; the
// constructor logs a warning and falls through to debug-level logging
// so the dev flow keeps working until real credentials are configured.

use async_trait::async_trait;

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
    pub host: String,
    pub port: u16,
    pub user: String,
    pub pass: String,
    pub from: String,
}

impl SmtpMailer {
    pub fn new(host: String, port: u16, user: String, pass: String, from: String) -> Self {
        tracing::warn!(
            "SmtpMailer configured for {host}:{port} but SMTP delivery is not yet implemented; \
             falling back to debug logging until wired up."
        );
        Self { host, port, user, pass, from }
    }
}

#[async_trait]
impl Mailer for SmtpMailer {
    async fn send_verification(&self, email: &str, code: &str) -> Result<(), String> {
        // TODO: integrate lettre or similar.
        tracing::debug!(
            target: "tnv_server::mailer",
            "(smtp-stub) would send code {code} to {email} via {}:{}",
            self.host, self.port
        );
        Ok(())
    }
}

/// Build a mailer from environment variables. SMTP_HOST empty → LogMailer.
pub fn from_env() -> std::sync::Arc<dyn Mailer> {
    let host = std::env::var("SMTP_HOST").unwrap_or_default();
    if host.is_empty() {
        return std::sync::Arc::new(LogMailer::new());
    }
    let port: u16 = std::env::var("SMTP_PORT").ok()
        .and_then(|s| s.parse().ok()).unwrap_or(587);
    let user = std::env::var("SMTP_USER").unwrap_or_default();
    let pass = std::env::var("SMTP_PASS").unwrap_or_default();
    let from = std::env::var("SMTP_FROM").unwrap_or_else(|_| "noreply@taxnvote.org".into());
    std::sync::Arc::new(SmtpMailer::new(host, port, user, pass, from))
}
