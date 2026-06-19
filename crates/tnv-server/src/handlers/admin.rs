// src/handlers/admin.rs — admin API under /api/admin/** (gated by the ACL layer
// to admin-tier callers; each handler also re-checks is_admin defensively).
//
// Moderation is soft: accounts/persons are *disabled*, templates/submissions are
// *hidden* — reversible, and the audit trail is preserved.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;

use crate::extract::ClientIp;
use crate::models::*;
use crate::state::*;

type Resp<T> = Result<Json<T>, (StatusCode, Json<Value>)>;

fn err(status: StatusCode, msg: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": msg })))
}
fn internal(msg: String) -> (StatusCode, Json<Value>) {
    tracing::error!("internal error: {msg}");
    err(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
}
fn require_admin(claims: &Claims) -> Result<(), (StatusCode, Json<Value>)> {
    if claims.is_admin() {
        Ok(())
    } else {
        Err(err(StatusCode::FORBIDDEN, "admin access required"))
    }
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub q: Option<String>,
    pub limit: Option<i64>,
    pub action: Option<String>,
}

fn limit_of(q: &ListQuery) -> i64 {
    q.limit.unwrap_or(200).clamp(1, 1000)
}

// ─── Users ───────────────────────────────────────────────────────

pub async fn list_users(
    State(state): State<AppState>,
    claims: Claims,
    Query(q): Query<ListQuery>,
) -> Resp<Vec<AdminUser>> {
    require_admin(&claims)?;
    let limit = limit_of(&q);
    let like = q.q.as_ref().map(|s| format!("%{s}%"));

    let mut users = Vec::new();

    // Accounts
    let acct_sql = state.q(if like.is_some() {
        "SELECT id, username, tier, disabled, created_at FROM accounts WHERE username LIKE ? ORDER BY id DESC LIMIT ?"
    } else {
        "SELECT id, username, tier, disabled, created_at FROM accounts ORDER BY id DESC LIMIT ?"
    });
    let mut aq = sqlx::query(&acct_sql);
    if let Some(l) = &like { aq = aq.bind(l); }
    aq = aq.bind(limit);
    for r in aq.fetch_all(&state.db).await.map_err(|e| internal(e.to_string()))? {
        users.push(AdminUser {
            kind: SUBJECT_ACCOUNT.to_string(),
            id: r.try_get("id").unwrap_or(0),
            name: r.try_get("username").unwrap_or_default(),
            tier: r.try_get("tier").unwrap_or(0),
            disabled: r.try_get::<i64, _>("disabled").unwrap_or(0) != 0,
            created_at: r.try_get("created_at").unwrap_or_default(),
        });
    }

    // Persons
    let person_sql = state.q(if like.is_some() {
        "SELECT id, name, disabled, created_at FROM persons WHERE name LIKE ? ORDER BY id DESC LIMIT ?"
    } else {
        "SELECT id, name, disabled, created_at FROM persons ORDER BY id DESC LIMIT ?"
    });
    let mut pq = sqlx::query(&person_sql);
    if let Some(l) = &like { pq = pq.bind(l); }
    pq = pq.bind(limit);
    for r in pq.fetch_all(&state.db).await.map_err(|e| internal(e.to_string()))? {
        users.push(AdminUser {
            kind: SUBJECT_PERSON.to_string(),
            id: r.try_get("id").unwrap_or(0),
            name: r.try_get("name").unwrap_or_default(),
            tier: 0,
            disabled: r.try_get::<i64, _>("disabled").unwrap_or(0) != 0,
            created_at: r.try_get("created_at").unwrap_or_default(),
        });
    }

    Ok(Json(users))
}

fn user_table(kind: &str) -> Option<&'static str> {
    match kind {
        SUBJECT_ACCOUNT => Some("accounts"),
        SUBJECT_PERSON => Some("persons"),
        _ => None,
    }
}

async fn set_disabled(
    state: &AppState,
    claims: &Claims,
    ip: &str,
    kind: &str,
    id: i64,
    disabled: bool,
) -> Resp<Value> {
    let table = user_table(kind).ok_or_else(|| err(StatusCode::BAD_REQUEST, "invalid subject kind"))?;
    let n = sqlx::query(&state.q(&format!("UPDATE {table} SET disabled = ? WHERE id = ?")))
        .bind(if disabled { 1 } else { 0 })
        .bind(id)
        .execute(&state.db).await
        .map_err(|e| internal(e.to_string()))?
        .rows_affected();
    if n == 0 {
        return Err(err(StatusCode::NOT_FOUND, "user not found"));
    }
    let action = if disabled { "admin.user.disable" } else { "admin.user.enable" };
    state.audit(SUBJECT_ACCOUNT, Some(claims.sub), action, Some(kind), Some(&id.to_string()), None, Some(ip)).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn disable_user(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    claims: Claims,
    Path((kind, id)): Path<(String, i64)>,
) -> Resp<Value> {
    require_admin(&claims)?;
    set_disabled(&state, &claims, &ip.to_string(), &kind, id, true).await
}

pub async fn enable_user(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    claims: Claims,
    Path((kind, id)): Path<(String, i64)>,
) -> Resp<Value> {
    require_admin(&claims)?;
    set_disabled(&state, &claims, &ip.to_string(), &kind, id, false).await
}

pub async fn set_role(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    claims: Claims,
    Path((kind, id)): Path<(String, i64)>,
    Json(req): Json<SetRoleRequest>,
) -> Resp<Value> {
    require_admin(&claims)?;
    if kind != SUBJECT_ACCOUNT {
        return Err(err(StatusCode::BAD_REQUEST, "roles apply to accounts only"));
    }
    if req.tier < 0 {
        return Err(err(StatusCode::BAD_REQUEST, "tier must be >= 0"));
    }
    let n = sqlx::query(&state.q("UPDATE accounts SET tier = ? WHERE id = ?"))
        .bind(req.tier)
        .bind(id)
        .execute(&state.db).await
        .map_err(|e| internal(e.to_string()))?
        .rows_affected();
    if n == 0 {
        return Err(err(StatusCode::NOT_FOUND, "account not found"));
    }
    state.audit(SUBJECT_ACCOUNT, Some(claims.sub), "admin.user.set_role",
        Some(SUBJECT_ACCOUNT), Some(&id.to_string()), Some(&json!({"tier": req.tier}).to_string()), Some(&ip.to_string())).await;
    Ok(Json(json!({ "ok": true })))
}

// ─── Templates ───────────────────────────────────────────────────

pub async fn list_templates(
    State(state): State<AppState>,
    claims: Claims,
    Query(q): Query<ListQuery>,
) -> Resp<Vec<AdminTemplate>> {
    require_admin(&claims)?;
    let limit = limit_of(&q);
    let rows = sqlx::query(&state.q(
        "SELECT receipt_no, name, subject_kind, subject_id, fiscal_year, hidden, created_at \
         FROM templates ORDER BY id DESC LIMIT ?"
    ))
        .bind(limit)
        .fetch_all(&state.db).await
        .map_err(|e| internal(e.to_string()))?;
    let out = rows.iter().map(|r| AdminTemplate {
        receipt_no: r.try_get("receipt_no").unwrap_or_default(),
        name: r.try_get("name").unwrap_or_default(),
        subject_kind: r.try_get("subject_kind").unwrap_or_default(),
        subject_id: r.try_get("subject_id").unwrap_or(0),
        fiscal_year: r.try_get("fiscal_year").unwrap_or_default(),
        hidden: r.try_get::<i64, _>("hidden").unwrap_or(0) != 0,
        created_at: r.try_get("created_at").unwrap_or_default(),
    }).collect();
    Ok(Json(out))
}

async fn set_template_hidden(
    state: &AppState,
    claims: &Claims,
    ip: &str,
    receipt_no: &str,
    hidden: bool,
) -> Resp<Value> {
    let n = sqlx::query(&state.q("UPDATE templates SET hidden = ? WHERE receipt_no = ?"))
        .bind(if hidden { 1 } else { 0 })
        .bind(receipt_no)
        .execute(&state.db).await
        .map_err(|e| internal(e.to_string()))?
        .rows_affected();
    if n == 0 {
        return Err(err(StatusCode::NOT_FOUND, "template not found"));
    }
    let action = if hidden { "admin.template.hide" } else { "admin.template.unhide" };
    state.audit(SUBJECT_ACCOUNT, Some(claims.sub), action, Some("template"), Some(receipt_no), None, Some(ip)).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn hide_template(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    claims: Claims,
    Path(receipt_no): Path<String>,
) -> Resp<Value> {
    require_admin(&claims)?;
    set_template_hidden(&state, &claims, &ip.to_string(), &receipt_no, true).await
}

pub async fn unhide_template(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    claims: Claims,
    Path(receipt_no): Path<String>,
) -> Resp<Value> {
    require_admin(&claims)?;
    set_template_hidden(&state, &claims, &ip.to_string(), &receipt_no, false).await
}

// ─── Submissions (Tax Dollars) ───────────────────────────────────

pub async fn list_taxdollars(
    State(state): State<AppState>,
    claims: Claims,
    Query(q): Query<ListQuery>,
) -> Resp<Vec<AdminTaxDollar>> {
    require_admin(&claims)?;
    let limit = limit_of(&q);
    let rows = sqlx::query(&state.q(
        "SELECT receipt_token, subject_kind, subject_id, fiscal_year, template_receipt_no, hidden, created_at \
         FROM tax_dollars ORDER BY id DESC LIMIT ?"
    ))
        .bind(limit)
        .fetch_all(&state.db).await
        .map_err(|e| internal(e.to_string()))?;
    let out = rows.iter().map(|r| AdminTaxDollar {
        receipt_token: r.try_get("receipt_token").unwrap_or_default(),
        subject_kind: r.try_get("subject_kind").unwrap_or_default(),
        subject_id: r.try_get("subject_id").unwrap_or(0),
        fiscal_year: r.try_get("fiscal_year").unwrap_or_default(),
        template_receipt_no: r.try_get("template_receipt_no").unwrap_or_default(),
        hidden: r.try_get::<i64, _>("hidden").unwrap_or(0) != 0,
        created_at: r.try_get("created_at").unwrap_or_default(),
    }).collect();
    Ok(Json(out))
}

pub async fn taxdollar_allocations(
    State(state): State<AppState>,
    claims: Claims,
    Path(receipt_token): Path<String>,
) -> Resp<Vec<NodeAmount>> {
    require_admin(&claims)?;
    let rows = sqlx::query(&state.q(
        "SELECT a.node_id, a.pct FROM tax_dollar_allocations a \
         JOIN tax_dollars t ON a.tax_dollar_id = t.id WHERE t.receipt_token = ?"
    ))
        .bind(&receipt_token)
        .fetch_all(&state.db).await
        .map_err(|e| internal(e.to_string()))?;
    let out = rows.iter().map(|r| NodeAmount {
        node_id: r.try_get("node_id").unwrap_or_default(),
        amount: r.try_get("pct").unwrap_or(0.0),
    }).collect();
    Ok(Json(out))
}

async fn set_taxdollar_hidden(
    state: &AppState,
    claims: &Claims,
    ip: &str,
    receipt_token: &str,
    hidden: bool,
) -> Resp<Value> {
    let n = sqlx::query(&state.q("UPDATE tax_dollars SET hidden = ? WHERE receipt_token = ?"))
        .bind(if hidden { 1 } else { 0 })
        .bind(receipt_token)
        .execute(&state.db).await
        .map_err(|e| internal(e.to_string()))?
        .rows_affected();
    if n == 0 {
        return Err(err(StatusCode::NOT_FOUND, "submission not found"));
    }
    // Visibility changed → drop cached aggregates so the People's Budget recomputes.
    state.aggregate_cache.write().await.clear();
    let action = if hidden { "admin.taxdollar.hide" } else { "admin.taxdollar.unhide" };
    state.audit(SUBJECT_ACCOUNT, Some(claims.sub), action, Some("taxdollar"), Some(receipt_token), None, Some(ip)).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn hide_taxdollar(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    claims: Claims,
    Path(receipt_token): Path<String>,
) -> Resp<Value> {
    require_admin(&claims)?;
    set_taxdollar_hidden(&state, &claims, &ip.to_string(), &receipt_token, true).await
}

pub async fn unhide_taxdollar(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    claims: Claims,
    Path(receipt_token): Path<String>,
) -> Resp<Value> {
    require_admin(&claims)?;
    set_taxdollar_hidden(&state, &claims, &ip.to_string(), &receipt_token, false).await
}

pub async fn template_entries(
    State(state): State<AppState>,
    claims: Claims,
    Path(receipt_no): Path<String>,
) -> Resp<Vec<NodeAmount>> {
    require_admin(&claims)?;
    let rows = sqlx::query(&state.q(
        "SELECT e.node_id, e.value FROM template_entries e \
         JOIN templates t ON e.template_id = t.id WHERE t.receipt_no = ?"
    ))
        .bind(&receipt_no)
        .fetch_all(&state.db).await
        .map_err(|e| internal(e.to_string()))?;
    let out = rows.iter().map(|r| NodeAmount {
        node_id: r.try_get("node_id").unwrap_or_default(),
        amount: r.try_get("value").unwrap_or(0.0),
    }).collect();
    Ok(Json(out))
}

// ─── Audit ───────────────────────────────────────────────────────

pub async fn list_audit(
    State(state): State<AppState>,
    claims: Claims,
    Query(q): Query<ListQuery>,
) -> Resp<Vec<AuditEntry>> {
    require_admin(&claims)?;
    let limit = limit_of(&q);
    let rows = if let Some(action) = &q.action {
        sqlx::query(&state.q(
            "SELECT id, ts, actor_kind, actor_id, action, target_kind, target_id, detail, ip \
             FROM audit_log WHERE action = ? ORDER BY id DESC LIMIT ?"
        )).bind(action).bind(limit)
            .fetch_all(&state.db).await
    } else {
        sqlx::query(&state.q(
            "SELECT id, ts, actor_kind, actor_id, action, target_kind, target_id, detail, ip \
             FROM audit_log ORDER BY id DESC LIMIT ?"
        )).bind(limit)
            .fetch_all(&state.db).await
    }.map_err(|e| internal(e.to_string()))?;

    let out = rows.iter().map(|r| AuditEntry {
        id: r.try_get("id").unwrap_or(0),
        ts: r.try_get("ts").unwrap_or_default(),
        actor_kind: r.try_get("actor_kind").unwrap_or_default(),
        actor_id: r.try_get("actor_id").ok(),
        action: r.try_get("action").unwrap_or_default(),
        target_kind: r.try_get("target_kind").ok(),
        target_id: r.try_get("target_id").ok(),
        detail: r.try_get("detail").ok(),
        ip: r.try_get("ip").ok(),
    }).collect();
    Ok(Json(out))
}

// ─── Config ──────────────────────────────────────────────────────

pub async fn get_config(
    State(state): State<AppState>,
    claims: Claims,
) -> Resp<Vec<SettingItem>> {
    require_admin(&claims)?;
    // Pull stored rows for updated_at; show every allowlisted key (default "").
    let rows = sqlx::query("SELECT key, value, updated_at FROM settings")
        .fetch_all(&state.db).await
        .map_err(|e| internal(e.to_string()))?;
    let mut stored = std::collections::HashMap::new();
    for r in &rows {
        let k: String = r.try_get("key").unwrap_or_default();
        let v: String = r.try_get("value").unwrap_or_default();
        let u: String = r.try_get("updated_at").unwrap_or_default();
        stored.insert(k, (v, u));
    }
    let out = SETTING_KEYS.iter().map(|k| {
        let (value, updated_at) = stored.get(*k).cloned().unwrap_or_default();
        SettingItem { key: k.to_string(), value, updated_at }
    }).collect();
    Ok(Json(out))
}

pub async fn set_config(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    claims: Claims,
    Path(key): Path<String>,
    Json(req): Json<SetSettingRequest>,
) -> Resp<Value> {
    require_admin(&claims)?;
    if !SETTING_KEYS.contains(&key.as_str()) {
        return Err(err(StatusCode::BAD_REQUEST, "unknown setting key"));
    }
    let max = if is_lp_key(&key) { LP_VALUE_MAX } else { SETTING_VALUE_MAX };
    if req.value.len() > max {
        return Err(err(StatusCode::BAD_REQUEST, "value too long"));
    }
    state.set_setting(&key, &req.value, &claims.username).await
        .map_err(|e| internal(e.to_string()))?;
    state.audit(SUBJECT_ACCOUNT, Some(claims.sub), "admin.config.set",
        Some("setting"), Some(&key), Some(&req.value), Some(&ip.to_string())).await;
    Ok(Json(json!({ "ok": true })))
}
