use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};
use sqlx::Row;
use crate::auth::*;
use crate::extract::ClientIp;
use crate::models::*;
use crate::state::*;
use crate::validation::{validate_registration, verify_pow};

// ─── GET /api/auth/challenge ─────────────────────────────────────

pub async fn challenge(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
) -> Result<Json<ChallengeResponse>, (StatusCode, Json<Value>)> {
    state.rate_limit(ip, "challenge", RATE_CHALLENGE_MAX, RATE_CHALLENGE_WINDOW_SECS)
        .await.map_err(too_many)?;
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
    ClientIp(ip): ClientIp,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state.rate_limit(ip, "register", RATE_REGISTER_MAX, RATE_REGISTER_WINDOW_SECS)
        .await.map_err(too_many)?;

    validate_registration(&req).map_err(bad)?;

    // Verify the PoW math FIRST so a wrong nonce can't burn a valid challenge,
    // then consume the challenge to enforce single-use + issued + not-expired.
    if !verify_pow(&req.challenge, &req.nonce, POW_DIFFICULTY) {
        return Err(bad("proof-of-work verification failed".into()));
    }
    {
        let mut challenges = state.challenges.write().await;
        if !challenges.consume(&req.challenge) {
            return Err(bad("invalid or expired challenge".into()));
        }
    }

    // Usernames are public, so a clash is safe to report.
    let uname_taken = sqlx::query(&state.q(
        "SELECT 1 AS one FROM accounts WHERE username = ? LIMIT 1"
    ))
        .bind(&req.username)
        .fetch_optional(&state.db).await
        .map_err(|e| internal(e.to_string()))?;
    if uname_taken.is_some() {
        return Err(bad("username already taken".into()));
    }

    let email_h = hash_email(&req.email);
    let password_h = hash_password(&req.password).map_err(internal)?;

    // Email-enumeration guard: if this email is already registered, respond
    // exactly as the happy path but send nothing — never reveal that it exists.
    let email_taken = sqlx::query(&state.q(
        "SELECT 1 AS one FROM accounts WHERE email_hash = ? LIMIT 1"
    ))
        .bind(&email_h)
        .fetch_optional(&state.db).await
        .map_err(|e| internal(e.to_string()))?;
    if email_taken.is_some() {
        return Ok(Json(json!({"message": "verification code sent", "email": req.email})));
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

    // Fail closed: if the code can't be delivered, roll back the pending row
    // so we don't leave an unusable verification record (and don't mislead the
    // user into waiting for an email that will never arrive).
    if let Err(e) = state.mailer.send_verification(&req.email, &code).await {
        tracing::warn!("mailer failed for {}: {e}", req.email);
        let _ = sqlx::query(&state.q("DELETE FROM email_verifications WHERE email_hash = ?"))
            .bind(&email_h)
            .execute(&state.db).await;
        return Err(internal("could not send verification email".into()));
    }

    Ok(Json(json!({"message": "verification code sent", "email": req.email})))
}

// ─── POST /api/auth/verify ───────────────────────────────────────

pub async fn verify(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<Value>)> {
    state.rate_limit(ip, "verify", RATE_VERIFY_MAX, RATE_VERIFY_WINDOW_SECS)
        .await.map_err(too_many)?;

    let email_h = hash_email(&req.email);
    let now = chrono::Utc::now().to_rfc3339();

    // Look up the pending row by email only (not by code) so we can count
    // failed guesses and burn the code after too many — otherwise a 6-digit
    // code is brute-forceable within the rate-limit window.
    let row = sqlx::query(&state.q(
        "SELECT id, username, password_hash, code, attempts FROM email_verifications \
         WHERE email_hash = ? AND expires_at > ? ORDER BY id DESC LIMIT 1"
    ))
        .bind(&email_h)
        .bind(&now)
        .fetch_optional(&state.db).await
        .map_err(|e| internal(e.to_string()))?
        .ok_or_else(|| bad("invalid or expired verification code".into()))?;

    let ver_id: i64 = row.try_get("id").map_err(|e| internal(e.to_string()))?;
    let username: String = row.try_get("username").map_err(|e| internal(e.to_string()))?;
    let password_hash: String = row.try_get("password_hash").map_err(|e| internal(e.to_string()))?;
    let stored_code: String = row.try_get("code").map_err(|e| internal(e.to_string()))?;
    let attempts: i64 = row.try_get("attempts").map_err(|e| internal(e.to_string()))?;

    if !ct_eq(stored_code.trim().as_bytes(), req.code.trim().as_bytes()) {
        // Wrong code: count the attempt, and burn the pending row once the
        // limit is reached so the code can't be brute-forced.
        if attempts + 1 >= MAX_VERIFY_ATTEMPTS {
            let _ = sqlx::query(&state.q("DELETE FROM email_verifications WHERE id = ?"))
                .bind(ver_id).execute(&state.db).await;
        } else {
            let _ = sqlx::query(&state.q(
                "UPDATE email_verifications SET attempts = attempts + 1 WHERE id = ?"
            )).bind(ver_id).execute(&state.db).await;
        }
        return Err(bad("invalid or expired verification code".into()));
    }

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

    let token = create_jwt(id, &username, 0, state.jwt_ttl_secs, &state.jwt_secret).map_err(internal)?;
    Ok(Json(AuthResponse { token, username }))
}

// ─── POST /api/auth/login ────────────────────────────────────────

pub async fn login(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<Value>)> {
    state.rate_limit(ip, "login", RATE_LOGIN_MAX, RATE_LOGIN_WINDOW_SECS)
        .await.map_err(too_many)?;

    let email_h = hash_email(&req.email);

    let row = sqlx::query(&state.q(
        "SELECT id, username, password_hash, tier FROM accounts WHERE email_hash = ? LIMIT 1"
    ))
        .bind(&email_h)
        .fetch_optional(&state.db).await
        .map_err(|e| internal(e.to_string()))?;

    // Constant-time-ish: when the email is unknown, still spend an Argon2
    // verification so response timing doesn't reveal whether the account exists.
    let row = match row {
        Some(r) => r,
        None => {
            dummy_password_verify(&req.password);
            return Err(bad("invalid email or password".into()));
        }
    };

    let id: i64 = row.try_get("id").map_err(|e| internal(e.to_string()))?;
    let username: String = row.try_get("username").map_err(|e| internal(e.to_string()))?;
    let password_hash: String = row.try_get("password_hash").map_err(|e| internal(e.to_string()))?;
    let tier: i32 = row.try_get("tier").map_err(|e| internal(e.to_string()))?;

    if !verify_password(&req.password, &password_hash) {
        return Err(bad("invalid email or password".into()));
    }

    let token = create_jwt(id, &username, tier, state.jwt_ttl_secs, &state.jwt_secret).map_err(internal)?;
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
    tracing::error!("internal error: {msg}");
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "internal server error"})))
}
fn too_many(retry_after: u64) -> (StatusCode, Json<Value>) {
    (StatusCode::TOO_MANY_REQUESTS, Json(json!({
        "error": "too many requests",
        "retry_after_secs": retry_after
    })))
}
