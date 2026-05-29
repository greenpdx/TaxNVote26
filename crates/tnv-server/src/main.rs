mod auth;
mod csv_parse;
mod handlers;
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
        .map_err(|e| format!("connect {database_url}: {e}"))?;

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
    let mailer = mailer::from_env();

    // ─── AppState ────────────────────────────────────────────────
    let state = AppState::new(
        pool,
        backend,
        std::path::PathBuf::from(&data_dir),
        jwt_secret,
        fiscal_year.clone(),
        valid_node_ids,
        mailer,
    );

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

    let api = Router::new()
        .route("/health", get(handlers::health::health))
        .route("/auth/challenge", get(handlers::auth::challenge))
        .route("/auth/register", post(handlers::auth::register))
        .route("/auth/verify", post(handlers::auth::verify))
        .route("/auth/login", post(handlers::auth::login))
        .route("/auth/me", get(handlers::auth::me))
        .route("/identify", post(handlers::identity::identify))
        .route("/aggregate", get(handlers::aggregate::aggregate))
        .route("/templates", get(handlers::templates::list_templates))
        .route("/templates", post(handlers::templates::create_template))
        .route("/templates/{receipt_no}", get(handlers::templates::get_template))
        .route("/taxdollar", post(handlers::taxdollar::submit_taxdollar))
        .route("/taxdollar/mine", get(handlers::taxdollar::my_taxdollars))
        .route("/taxdollar/{receipt_token}", get(handlers::taxdollar::get_taxdollar));

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
