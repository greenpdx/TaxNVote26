use axum::{extract::{State, Path, Query}, http::{StatusCode, HeaderMap}, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use crate::auth::{hash_secret, ct_eq};
use crate::csv_parse::parse_taxdollar_csv;
use crate::extract::ClientIp;
use crate::models::*;
use crate::state::*;
use crate::validation::validate_taxdollar;

#[derive(Debug, Deserialize)]
pub struct ViewQuery {
    pub pin: Option<String>,
}

/// Hash a 4-digit access PIN against its submission's (unguessable) receipt
/// token as salt. Returns None unless the PIN is exactly 4 digits.
fn access_pin_hash(receipt_token: &str, pin: &str) -> Option<String> {
    let pin = pin.trim();
    if pin.len() == 4 && pin.chars().all(|c| c.is_ascii_digit()) {
        Some(hash_secret(receipt_token, pin))
    } else {
        None
    }
}

pub async fn submit_taxdollar(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    claims: Claims,
    body: String,
) -> Result<Json<TaxDollarReceipt>, (StatusCode, Json<Value>)> {
    state.rate_limit(ip, "submit", RATE_SUBMIT_MAX, RATE_SUBMIT_WINDOW_SECS)
        .await.map_err(too_many)?;

    let parsed = parse_taxdollar_csv(&body).map_err(bad)?;
    validate_taxdollar(&parsed, &state.valid_node_ids).map_err(bad)?;

    // Tax dollars are only accepted for the year this deployment is configured for.
    // Bumping FISCAL_YEAR in .env is how a new annual cycle is opened.
    if parsed.fiscal_year != state.fiscal_year {
        return Err(bad(format!(
            "submission fiscal_year '{}' does not match active year '{}'",
            parsed.fiscal_year, state.fiscal_year
        )));
    }

    // Verify template exists unless "default"
    if parsed.template_id != "default" {
        let exists = sqlx::query(&state.q(
            "SELECT 1 AS one FROM templates WHERE receipt_no = ? LIMIT 1"
        ))
            .bind(&parsed.template_id)
            .fetch_optional(&state.db).await
            .map_err(|e| internal(e.to_string()))?;
        if exists.is_none() {
            return Err(bad(format!("template '{}' not found", parsed.template_id)));
        }
    }

    let receipt_token = state.generate_td_receipt();
    let now = chrono::Utc::now().to_rfc3339();

    // Optional access PIN (X-Access-Pin: NNNN) gating the public link before release.
    let pin_hash: Option<String> = headers.get("x-access-pin")
        .and_then(|v| v.to_str().ok())
        .and_then(|p| access_pin_hash(&receipt_token, p));

    let mut tx = state.db.begin().await.map_err(|e| internal(e.to_string()))?;

    // Upsert: drop the prior TD for (subject, fiscal_year). CASCADE removes allocations.
    let del = sqlx::query(&state.q(
        "DELETE FROM tax_dollars WHERE subject_kind = ? AND subject_id = ? AND fiscal_year = ?"
    ))
        .bind(&claims.kind)
        .bind(claims.sub)
        .bind(&parsed.fiscal_year)
        .execute(&mut *tx).await
        .map_err(|e| internal(e.to_string()))?;
    let replaced = del.rows_affected() > 0;

    let row = sqlx::query(&state.q(
        "INSERT INTO tax_dollars (receipt_token, subject_kind, subject_id, fiscal_year, template_receipt_no, raw_csv, checksum, access_pin_hash, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id"
    ))
        .bind(&receipt_token)
        .bind(&claims.kind)
        .bind(claims.sub)
        .bind(&parsed.fiscal_year)
        .bind(&parsed.template_id)
        .bind(&parsed.raw_csv)
        .bind(&parsed.checksum)
        .bind(&pin_hash)
        .bind(&now)
        .fetch_one(&mut *tx).await
        .map_err(|e| internal(e.to_string()))?;
    let td_id: i64 = row.try_get("id").map_err(|e| internal(e.to_string()))?;

    for alloc in &parsed.allocations {
        sqlx::query(&state.q(
            "INSERT INTO tax_dollar_allocations (tax_dollar_id, node_id, pct) VALUES (?, ?, ?)"
        ))
            .bind(td_id)
            .bind(&alloc.node_id)
            .bind(alloc.pct)
            .execute(&mut *tx).await
            .map_err(|e| internal(e.to_string()))?;
    }

    tx.commit().await.map_err(|e| internal(e.to_string()))?;

    // Data changed → drop cached aggregate for this year so it recomputes.
    state.aggregate_cache.write().await.remove(&parsed.fiscal_year);

    tracing::info!("TD submitted: {} (person={}, fy={}, replaced={})",
        receipt_token, claims.sub, parsed.fiscal_year, replaced);

    Ok(Json(TaxDollarReceipt {
        receipt_token, fiscal_year: parsed.fiscal_year,
        created_at: now, replaced,
    }))
}

pub async fn get_taxdollar(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Path(receipt_token): Path<String>,
    Query(q): Query<ViewQuery>,
) -> Result<String, (StatusCode, Json<Value>)> {
    // Throttle so the 4-digit access PIN can't be brute-forced (the token is
    // already unguessable; this guards the PIN).
    state.rate_limit(ip, "view", RATE_VIEW_MAX, RATE_VIEW_WINDOW_SECS)
        .await.map_err(too_many)?;

    let row = sqlx::query(&state.q(
        "SELECT raw_csv, access_pin_hash FROM tax_dollars WHERE receipt_token = ? AND hidden = 0 LIMIT 1"
    ))
        .bind(&receipt_token)
        .fetch_optional(&state.db).await
        .map_err(|e| internal(e.to_string()))?
        .ok_or_else(|| not_found("tax dollar not found"))?;

    let csv: String = row.try_get("raw_csv").map_err(|e| internal(e.to_string()))?;
    let pin_hash: Option<String> = row.try_get("access_pin_hash").ok().flatten();

    // Once the data is released, links are public — no PIN needed.
    if state.setting_or("data_public", false).await {
        return Ok(csv);
    }

    // Before release: gate on the per-submission access PIN.
    let Some(hash) = pin_hash else {
        return Err((StatusCode::FORBIDDEN,
            Json(json!({"error": "this submission is private until the data is released"}))));
    };
    match q.pin {
        None => Err((StatusCode::UNAUTHORIZED, Json(json!({"error": "pin required", "pin_required": true})))),
        Some(pin) if ct_eq(hash_secret(&receipt_token, pin.trim()).as_bytes(), hash.as_bytes()) => Ok(csv),
        Some(_) => Err((StatusCode::FORBIDDEN, Json(json!({"error": "invalid pin"})))),
    }
}

pub async fn my_taxdollars(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<Vec<TaxDollarSummary>>, (StatusCode, Json<Value>)> {
    let rows = sqlx::query(&state.q(
        "SELECT receipt_token, fiscal_year, template_receipt_no, raw_csv, created_at \
         FROM tax_dollars WHERE subject_kind = ? AND subject_id = ? ORDER BY id DESC"
    ))
        .bind(&claims.kind)
        .bind(claims.sub)
        .fetch_all(&state.db).await
        .map_err(|e| internal(e.to_string()))?;

    let summaries: Vec<TaxDollarSummary> = rows.iter().map(|r| TaxDollarSummary {
        receipt_token: r.try_get("receipt_token").unwrap_or_default(),
        fiscal_year: r.try_get("fiscal_year").unwrap_or_default(),
        template_receipt_no: r.try_get("template_receipt_no").unwrap_or_default(),
        created_at: r.try_get("created_at").unwrap_or_default(),
        raw_csv: r.try_get("raw_csv").unwrap_or_default(),
    }).collect();
    Ok(Json(summaries))
}

fn bad(msg: String) -> (StatusCode, Json<Value>) {
    (StatusCode::BAD_REQUEST, Json(json!({"error": msg})))
}
fn internal(msg: String) -> (StatusCode, Json<Value>) {
    tracing::error!("internal error: {msg}");
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "internal server error"})))
}
fn not_found(msg: &str) -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_FOUND, Json(json!({"error": msg})))
}
fn too_many(retry_after: u64) -> (StatusCode, Json<Value>) {
    (StatusCode::TOO_MANY_REQUESTS, Json(json!({
        "error": "too many requests", "retry_after_secs": retry_after
    })))
}
