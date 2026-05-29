use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};
use sqlx::Row;
use crate::auth::{create_jwt, hash_secret};
use crate::models::*;
use crate::state::*;

// ─── POST /api/identify ──────────────────────────────────────────
// Demo identity: a name + 4-digit secret. The (name, secret) pair is the
// identity — find-or-create the person, then issue a JWT (sub = person_id)
// so the existing Claims extractor protects submit / mine.
pub async fn identify(
    State(state): State<AppState>,
    Json(req): Json<IdentifyRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<Value>)> {
    let name = req.name.trim().to_string();
    if name.len() < PERSON_NAME_MIN || name.len() > PERSON_NAME_MAX {
        return Err(bad(format!("name must be {}-{} chars", PERSON_NAME_MIN, PERSON_NAME_MAX)));
    }
    if req.secret.len() != PIN_LEN || !req.secret.chars().all(|c| c.is_ascii_digit()) {
        return Err(bad(format!("secret must be {} digits", PIN_LEN)));
    }

    let secret_hash = hash_secret(&name, &req.secret);

    // Find existing (name, secret) person.
    let existing = sqlx::query(&state.q(
        "SELECT id FROM persons WHERE name = ? AND secret_hash = ? LIMIT 1"
    ))
        .bind(&name)
        .bind(&secret_hash)
        .fetch_optional(&state.db).await
        .map_err(|e| internal(e.to_string()))?;

    let person_id: i64 = if let Some(row) = existing {
        row.try_get("id").map_err(|e| internal(e.to_string()))?
    } else {
        let now = chrono::Utc::now().to_rfc3339();
        let row = sqlx::query(&state.q(
            "INSERT INTO persons (name, secret_hash, created_at) VALUES (?, ?, ?) RETURNING id"
        ))
            .bind(&name)
            .bind(&secret_hash)
            .bind(&now)
            .fetch_one(&state.db).await
            .map_err(|e| internal(e.to_string()))?;
        row.try_get("id").map_err(|e| internal(e.to_string()))?
    };

    let token = create_jwt(person_id, &name, 0, &state.jwt_secret).map_err(internal)?;
    Ok(Json(AuthResponse { token, username: name }))
}

fn bad(msg: String) -> (StatusCode, Json<Value>) {
    (StatusCode::BAD_REQUEST, Json(json!({"error": msg})))
}
fn internal(msg: String) -> (StatusCode, Json<Value>) {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg})))
}
