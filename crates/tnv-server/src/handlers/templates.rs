use axum::{extract::{State, Path}, http::StatusCode, Json};
use serde_json::{json, Value};
use sqlx::Row;
use crate::csv_parse::parse_template_csv;
use crate::models::*;
use crate::state::*;
use crate::validation::validate_template;

pub async fn list_templates(
    State(state): State<AppState>,
) -> Result<Json<Vec<TemplateSummary>>, (StatusCode, Json<Value>)> {
    let rows = sqlx::query(
        "SELECT t.receipt_no, t.name, t.entity_name, t.description, t.fiscal_year, t.created_at, \
                (SELECT COUNT(*) FROM template_entries WHERE template_id = t.id) AS entry_count \
         FROM templates t ORDER BY t.id DESC"
    )
        .fetch_all(&state.db).await
        .map_err(|e| internal(e.to_string()))?;

    let summaries: Vec<TemplateSummary> = rows.iter().map(|r| {
        let description: Option<String> = r.try_get("description").ok();
        let entity_name: Option<String> = r.try_get("entity_name").ok();
        TemplateSummary {
            receipt_no: r.try_get("receipt_no").unwrap_or_default(),
            name: r.try_get("name").unwrap_or_default(),
            entity_name: entity_name.filter(|s| !s.is_empty()),
            description: description.filter(|s| !s.is_empty()),
            fiscal_year: r.try_get("fiscal_year").unwrap_or_default(),
            entry_count: r.try_get("entry_count").unwrap_or(0),
            created_at: r.try_get("created_at").unwrap_or_default(),
        }
    }).collect();
    Ok(Json(summaries))
}

pub async fn get_template(
    State(state): State<AppState>,
    Path(receipt_no): Path<String>,
) -> Result<String, (StatusCode, Json<Value>)> {
    let row = sqlx::query(&state.q(
        "SELECT raw_csv FROM templates WHERE receipt_no = ? LIMIT 1"
    ))
        .bind(&receipt_no)
        .fetch_optional(&state.db).await
        .map_err(|e| internal(e.to_string()))?
        .ok_or_else(|| not_found("template not found"))?;
    row.try_get::<String, _>("raw_csv").map_err(|e| internal(e.to_string()))
}

pub async fn create_template(
    State(state): State<AppState>,
    claims: Claims,
    body: String,
) -> Result<Json<TemplateReceipt>, (StatusCode, Json<Value>)> {
    let parsed = parse_template_csv(&body).map_err(bad)?;
    validate_template(&parsed, &state.valid_node_ids).map_err(bad)?;

    let receipt_no = state.next_template_receipt().await
        .map_err(|e| internal(e.to_string()))?;
    let now = chrono::Utc::now().to_rfc3339();
    let description: Option<&str> = if parsed.description.is_empty() {
        None
    } else {
        Some(parsed.description.as_str())
    };
    let entity: Option<&str> = if parsed.entity_name.is_empty() {
        None
    } else {
        Some(parsed.entity_name.as_str())
    };

    let mut tx = state.db.begin().await.map_err(|e| internal(e.to_string()))?;

    let row = sqlx::query(&state.q(
        "INSERT INTO templates (receipt_no, person_id, entity_name, name, description, fiscal_year, raw_csv, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id"
    ))
        .bind(&receipt_no)
        .bind(claims.sub)
        .bind(entity)
        .bind(&parsed.name)
        .bind(description)
        .bind(&parsed.fiscal_year)
        .bind(&parsed.raw_csv)
        .bind(&now)
        .fetch_one(&mut *tx).await
        .map_err(|e| internal(e.to_string()))?;
    let tpl_id: i64 = row.try_get("id").map_err(|e| internal(e.to_string()))?;

    for entry in &parsed.entries {
        sqlx::query(&state.q(
            "INSERT INTO template_entries (template_id, node_id, value) VALUES (?, ?, ?)"
        ))
            .bind(tpl_id)
            .bind(&entry.node_id)
            .bind(entry.value)
            .execute(&mut *tx).await
            .map_err(|e| internal(e.to_string()))?;
    }

    tx.commit().await.map_err(|e| internal(e.to_string()))?;

    tracing::info!("Template created: {} ({} entries)", receipt_no, parsed.entries.len());
    Ok(Json(TemplateReceipt { receipt_no, name: parsed.name, created_at: now }))
}

fn bad(msg: String) -> (StatusCode, Json<Value>) {
    (StatusCode::BAD_REQUEST, Json(json!({"error": msg})))
}
fn internal(msg: String) -> (StatusCode, Json<Value>) {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg})))
}
fn not_found(msg: &str) -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_FOUND, Json(json!({"error": msg})))
}
