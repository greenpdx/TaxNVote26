mod acl;
mod auth;
mod csv_parse;
mod extract;
mod handlers;
#[cfg(test)]
mod it_admin;
mod mailer;
mod models;
mod state;
mod validation;

use axum::{extract::DefaultBodyLimit, routing::{get, post}, Router};
use sqlx::any::AnyPoolOptions;
use std::process::ExitCode;
use tower_http::{cors::CorsLayer, services::{ServeDir, ServeFile}, trace::TraceLayer};
use tracing_subscriber::EnvFilter;
use crate::state::{AppState, DbBackend, load_node_ids};

const MIN_JWT_SECRET_LEN: usize = 32;

#[tokio::main]
async fn main() -> ExitCode {
    // Load .env first so RUST_LOG and friends are visible to the logger.
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("startup failed: {e}");
            ExitCode::from(1)
        }
    }
}

async fn run() -> Result<(), String> {
    // ─── Required env ────────────────────────────────────────────
    let jwt_secret = std::env::var("JWT_SECRET")
        .map_err(|_| "JWT_SECRET is required (set in .env). \
                     Generate one with: openssl rand -hex 32".to_string())?;
    if jwt_secret.len() < MIN_JWT_SECRET_LEN {
        return Err(format!(
            "JWT_SECRET must be at least {MIN_JWT_SECRET_LEN} chars (got {})",
            jwt_secret.len()
        ));
    }

    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL is required (set in .env)".to_string())?;
    let backend = DbBackend::from_url(&database_url)?;

    let fiscal_year = std::env::var("FISCAL_YEAR")
        .map_err(|_| "FISCAL_YEAR is required (set in .env)".to_string())?;
    if fiscal_year.len() != 4 || !fiscal_year.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("FISCAL_YEAR must be 4 digits, got '{fiscal_year}'"));
    }

    // ─── Optional env ────────────────────────────────────────────
    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "data".into());
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());

    // JWT lifetime (hours). Short-lived since there is no revocation yet.
    let jwt_ttl_hours: i64 = std::env::var("JWT_TTL_HOURS").ok()
        .and_then(|s| s.parse().ok())
        .filter(|h| *h > 0)
        .unwrap_or(2);
    let jwt_ttl_secs = jwt_ttl_hours * 3600;

    // Honor X-Forwarded-For only when explicitly told we're behind a trusted
    // proxy (e.g. Caddy). Otherwise the header is attacker-controlled.
    let trusted_proxy = env_flag("TRUSTED_PROXY", false);

    // Demo PIN identity (/api/identify). Off by default — it is an
    // impersonation oracle and must stay disabled on public deployments.
    let enable_demo_identity = env_flag("ENABLE_DEMO_IDENTITY", false);
    if enable_demo_identity {
        tracing::warn!(
            "ENABLE_DEMO_IDENTITY is ON: /api/identify (name + 4-digit PIN) is mounted. \
             This allows trivial impersonation and must NOT be enabled in production."
        );
    }

    // ─── Filesystem prep (sqlite needs parent dir to exist) ──────
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("create data_dir {data_dir}: {e}"))?;

    // ─── Validation set (fatal if CSV missing) ───────────────────
    let valid_node_ids = load_node_ids(&data_dir)?;
    tracing::info!("Loaded {} valid node IDs from {}/budauth.csv",
        valid_node_ids.len(), data_dir);

    // ─── Database ────────────────────────────────────────────────
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(10)
        .connect(&database_url).await
        // Do not interpolate database_url — it may carry a password.
        .map_err(|e| format!("database connection failed ({:?}): {e}", backend))?;

    match backend {
        DbBackend::Sqlite => {
            sqlx::migrate!("../../migrations/sqlite").run(&pool).await
                .map_err(|e| format!("sqlite migrate: {e}"))?;
            tracing::info!("SQLite migrations applied");
        }
        DbBackend::Postgres => {
            sqlx::migrate!("../../migrations/postgres").run(&pool).await
                .map_err(|e| format!("postgres migrate: {e}"))?;
            tracing::info!("Postgres migrations applied");
        }
    }

    // ─── Mailer ──────────────────────────────────────────────────
    let mailer = mailer::from_env()?;

    // ─── AppState ────────────────────────────────────────────────
    let state = AppState::new(
        pool,
        backend,
        std::path::PathBuf::from(&data_dir),
        jwt_secret,
        fiscal_year.clone(),
        jwt_ttl_secs,
        trusted_proxy,
        enable_demo_identity,
        valid_node_ids,
        mailer,
    );

    // ─── CLI subcommands (e.g. `tnv-server admin promote <email>`) ───
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("admin") {
        return run_admin_cli(&state, &args).await;
    }

    // ─── Bootstrap admin + load runtime settings ─────────────────
    bootstrap_admin(&state).await?;
    state.reload_settings().await.map_err(|e| format!("load settings: {e}"))?;

    // Spawn periodic rate limiter cleanup (every 5 min)
    {
        let rl = state.rate_limiter.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            loop {
                interval.tick().await;
                let mut limiter = rl.write().await;
                limiter.cleanup(900); // max window is 900s
            }
        });
    }

    let mut api = Router::new()
        .route("/health", get(handlers::health::health))
        .route("/auth/challenge", get(handlers::auth::challenge))
        .route("/auth/register", post(handlers::auth::register))
        .route("/auth/verify", post(handlers::auth::verify))
        .route("/auth/login", post(handlers::auth::login))
        .route("/auth/me", get(handlers::auth::me))
        .route("/aggregate", get(handlers::aggregate::aggregate))
        .route("/templates", get(handlers::templates::list_templates))
        .route("/templates", post(handlers::templates::create_template))
        .route("/templates/{receipt_no}", get(handlers::templates::get_template))
        .route("/taxdollar", post(handlers::taxdollar::submit_taxdollar))
        .route("/taxdollar/mine", get(handlers::taxdollar::my_taxdollars))
        .route("/taxdollar/{receipt_token}", get(handlers::taxdollar::get_taxdollar));

    if enable_demo_identity {
        api = api.route("/identify", post(handlers::identity::identify));
    }

    // Admin sub-router, gated to admin-tier callers by the axum-acl layer.
    let admin = Router::new()
        .route("/users", get(handlers::admin::list_users))
        .route("/users/{kind}/{id}/disable", post(handlers::admin::disable_user))
        .route("/users/{kind}/{id}/enable", post(handlers::admin::enable_user))
        .route("/users/{kind}/{id}/role", post(handlers::admin::set_role))
        .route("/templates", get(handlers::admin::list_templates))
        .route("/templates/{receipt_no}/hide", post(handlers::admin::hide_template))
        .route("/templates/{receipt_no}/unhide", post(handlers::admin::unhide_template))
        .route("/audit", get(handlers::admin::list_audit))
        .route("/config", get(handlers::admin::get_config))
        .route("/config/{key}", axum::routing::put(handlers::admin::set_config))
        .layer(acl::admin_layer(state.jwt_secret.clone()));
    api = api.nest("/admin", admin);

    let static_service = ServeDir::new("static")
        .not_found_service(ServeFile::new("static/index.html"));

    let app = Router::new()
        .nest("/api", api)
        .fallback_service(static_service)
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::max(1024 * 1024)) // 1 MB
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await
        .map_err(|e| format!("bind {bind_addr}: {e}"))?;
    tracing::info!("TNV server listening on {bind_addr} (fiscal_year={fiscal_year}, backend={:?})", backend);

    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .await
        .map_err(|e| format!("serve: {e}"))
}

/// Handle `tnv-server admin <subcommand>` CLI invocations, then exit.
async fn run_admin_cli(state: &AppState, args: &[String]) -> Result<(), String> {
    match args.get(2).map(String::as_str) {
        Some("promote") => {
            let email = args.get(3)
                .ok_or("usage: tnv-server admin promote <email>".to_string())?;
            let email_h = crate::auth::hash_email(email);
            let n = sqlx::query(&state.q("UPDATE accounts SET tier = ? WHERE email_hash = ?"))
                .bind(models::ADMIN_TIER)
                .bind(&email_h)
                .execute(&state.db).await
                .map_err(|e| format!("promote failed: {e}"))?
                .rows_affected();
            if n == 0 {
                return Err(format!("no account found for email {email}"));
            }
            println!("Promoted {email} to admin (tier {}).", models::ADMIN_TIER);
            Ok(())
        }
        other => Err(format!("unknown admin command: {other:?} (try: promote <email>)")),
    }
}

/// Promote the BOOTSTRAP_ADMIN_EMAIL account to admin at startup, if present.
async fn bootstrap_admin(state: &AppState) -> Result<(), String> {
    let email = match std::env::var("BOOTSTRAP_ADMIN_EMAIL") {
        Ok(e) if !e.trim().is_empty() => e,
        _ => return Ok(()),
    };
    let email_h = crate::auth::hash_email(&email);
    let n = sqlx::query(&state.q("UPDATE accounts SET tier = ? WHERE email_hash = ? AND tier < ?"))
        .bind(models::ADMIN_TIER)
        .bind(&email_h)
        .bind(models::ADMIN_TIER)
        .execute(&state.db).await
        .map_err(|e| format!("bootstrap admin: {e}"))?
        .rows_affected();
    if n > 0 {
        tracing::info!("Bootstrapped admin account for {email}");
    } else {
        tracing::info!("BOOTSTRAP_ADMIN_EMAIL set; no matching non-admin account yet for {email}");
    }
    Ok(())
}

/// Read a boolean env var. Accepts 1/true/yes/on (case-insensitive) as true.
fn env_flag(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => default,
    }
}
