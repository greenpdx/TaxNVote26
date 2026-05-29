use axum::{extract::State, http::StatusCode, Json};
use axum::extract::ConnectInfo;
use serde_json::{json, Value};
use sqlx::Row;
use std::net::SocketAddr;
use crate::auth::*;
use crate::models::*;
use crate::state::*;
use crate::validation::{validate_registration, verify_pow};

// ─── GET /api/auth/challenge ─────────────────────────────────────

pub async fn challenge(
    State(state): State<AppState>,
) -> Result<Json<ChallengeResponse>, (StatusCode, Json<Value>)> {
    let mut challenges = state.challenges.write().await;
    let challenge = challenges.issue();
    Ok(Json(ChallengeResponse {
        challenge,
        difficulty: POW_DIFFICULTY,
        expires_in_secs: CHALLENGE_TTL_SECS,
    }))
}

// ─── POST /api/auth/register ─────────────────────────────────────

pub async fn register(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    {
        let mut rl = state.rate_limiter.write().await;
        rl.check(addr.ip(), "register", RATE_REGISTER_MAX, RATE_REGISTER_WINDOW_SECS)
            .map_err(too_many)?;
    }

    validate_registration(&req).map_err(bad)?;

    {
        let mut challenges = state.challenges.write().await;
        if !challenges.consume(&req.challenge) {
            return Err(bad("invalid or expired challenge".into()));
        }
    }

    if !verify_pow(&req.challenge, &req.nonce, POW_DIFFICULTY) {
        return Err(bad("proof-of-work verification failed".into()));
    }

    let email_h = hash_email(&req.email);
    let password_h = hash_password(&req.password).map_err(internal)?;

    // Duplicate check
    let dup = sqlx::query(&state.q(
        "SELECT 1 AS one FROM accounts WHERE email_hash = ? OR username = ? LIMIT 1"
    ))
        .bind(&email_h)
        .bind(&req.username)
        .fetch_optional(&state.db).await
        .map_err(|e| internal(e.to_string()))?;
    if dup.is_some() {
        return Err(bad("username or email already registered".into()));
    }

    // Single-flight: clear any prior verification rows for this email
    sqlx::query(&state.q("DELETE FROM email_verifications WHERE email_hash = ?"))
        .bind(&email_h)
        .execute(&state.db).await
        .map_err(|e| internal(e.to_string()))?;

    let code = generate_verification_code();
    let expires = (chrono::Utc::now() + chrono::Duration::minutes(15)).to_rfc3339();

    sqlx::query(&state.q(
        "INSERT INTO email_verifications (email_hash, username, password_hash, code, expires_at) \
         VALUES (?, ?, ?, ?, ?)"
    ))
        .bind(&email_h)
        .bind(&req.username)
        .bind(&password_h)
        .bind(&code)
        .bind(&expires)
        .execute(&state.db).await
        .map_err(|e| internal(e.to_string()))?;

    if let Err(e) = state.mailer.send_verification(&req.email, &code).await {
        tracing::warn!("mailer failed for {}: {e}", req.email);
    }

    Ok(Json(json!({"message": "verification code sent", "email": req.email})))
}

// ─── POST /api/auth/verify ───────────────────────────────────────

pub async fn verify(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<Value>)> {
    {
        let mut rl = state.rate_limiter.write().await;
        rl.check(addr.ip(), "verify", RATE_VERIFY_MAX, RATE_VERIFY_WINDOW_SECS)
            .map_err(too_many)?;
    }

    let email_h = hash_email(&req.email);
    let now = chrono::Utc::now().to_rfc3339();

    let row = sqlx::query(&state.q(
        "SELECT id, username, password_hash FROM email_verifications \
         WHERE email_hash = ? AND code = ? AND expires_at > ? LIMIT 1"
    ))
        .bind(&email_h)
        .bind(&req.code)
        .bind(&now)
        .fetch_optional(&state.db).await
        .map_err(|e| internal(e.to_string()))?
        .ok_or_else(|| bad("invalid or expired verification code".into()))?;

    let ver_id: i64 = row.try_get("id").map_err(|e| internal(e.to_string()))?;
    let username: String = row.try_get("username").map_err(|e| internal(e.to_string()))?;
    let password_hash: String = row.try_get("password_hash").map_err(|e| internal(e.to_string()))?;

    // Burn the verification row before account creation so a unique-violation
    // on accounts doesn't leave a dangling code redeemable later.
    sqlx::query(&state.q("DELETE FROM email_verifications WHERE id = ?"))
        .bind(ver_id)
        .execute(&state.db).await
        .map_err(|e| internal(e.to_string()))?;

    let created_at = chrono::Utc::now().to_rfc3339();
    let row = sqlx::query(&state.q(
        "INSERT INTO accounts (username, email_hash, password_hash, tier, created_at) \
         VALUES (?, ?, ?, 0, ?) RETURNING id"
    ))
        .bind(&username)
        .bind(&email_h)
        .bind(&password_hash)
        .bind(&created_at)
        .fetch_one(&state.db).await
        .map_err(|e| internal(e.to_string()))?;
    let id: i64 = row.try_get("id").map_err(|e| internal(e.to_string()))?;

    let token = create_jwt(id, &username, 0, &state.jwt_secret).map_err(internal)?;
    Ok(Json(AuthResponse { token, username }))
}

// ─── POST /api/auth/login ────────────────────────────────────────

pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<Value>)> {
    {
        let mut rl = state.rate_limiter.write().await;
        rl.check(addr.ip(), "login", RATE_LOGIN_MAX, RATE_LOGIN_WINDOW_SECS)
            .map_err(too_many)?;
    }

    let email_h = hash_email(&req.email);

    let row = sqlx::query(&state.q(
        "SELECT id, username, password_hash, tier FROM accounts WHERE email_hash = ? LIMIT 1"
    ))
        .bind(&email_h)
        .fetch_optional(&state.db).await
        .map_err(|e| internal(e.to_string()))?
        .ok_or_else(|| bad("invalid email or password".into()))?;

    let id: i64 = row.try_get("id").map_err(|e| internal(e.to_string()))?;
    let username: String = row.try_get("username").map_err(|e| internal(e.to_string()))?;
    let password_hash: String = row.try_get("password_hash").map_err(|e| internal(e.to_string()))?;
    let tier: i32 = row.try_get("tier").map_err(|e| internal(e.to_string()))?;

    if !verify_password(&req.password, &password_hash) {
        return Err(bad("invalid email or password".into()));
    }

    let token = create_jwt(id, &username, tier, &state.jwt_secret).map_err(internal)?;
    Ok(Json(AuthResponse { token, username }))
}

// ─── GET /api/auth/me ────────────────────────────────────────────

pub async fn me(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<MeResponse>, (StatusCode, Json<Value>)> {
    let row = sqlx::query(&state.q(
        "SELECT id, username, tier, created_at FROM accounts WHERE id = ? LIMIT 1"
    ))
        .bind(claims.sub)
        .fetch_optional(&state.db).await
        .map_err(|e| internal(e.to_string()))?
        .ok_or_else(|| bad("account not found".into()))?;

    Ok(Json(MeResponse {
        id: row.try_get("id").map_err(|e| internal(e.to_string()))?,
        username: row.try_get("username").map_err(|e| internal(e.to_string()))?,
        tier: row.try_get("tier").map_err(|e| internal(e.to_string()))?,
        created_at: row.try_get("created_at").map_err(|e| internal(e.to_string()))?,
    }))
}

/// JWT extractor for protected routes
impl axum::extract::FromRequestParts<AppState> for Claims {
    type Rejection = (StatusCode, Json<Value>);

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts, state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = parts.headers.get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, Json(json!({"error": "missing authorization"}))))?;

        let token = auth.strip_prefix("Bearer ")
            .ok_or_else(|| (StatusCode::UNAUTHORIZED, Json(json!({"error": "invalid auth format"}))))?;

        verify_jwt(token, &state.jwt_secret)
            .map_err(|e| (StatusCode::UNAUTHORIZED, Json(json!({"error": e}))))
    }
}

fn bad(msg: String) -> (StatusCode, Json<Value>) {
    (StatusCode::BAD_REQUEST, Json(json!({"error": msg})))
}
fn internal(msg: String) -> (StatusCode, Json<Value>) {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg})))
}
fn too_many(retry_after: u64) -> (StatusCode, Json<Value>) {
    (StatusCode::TOO_MANY_REQUESTS, Json(json!({
        "error": "too many requests",
        "retry_after_secs": retry_after
    })))
}
