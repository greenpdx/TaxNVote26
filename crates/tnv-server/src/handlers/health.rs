use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};
use crate::state::AppState;

/// Liveness + readiness. Pings the database so orchestrators (and Caddy/LB
/// health checks) can detect a dead or unreachable DB: returns 503 when the
/// query fails, 200 otherwise.
pub async fn health(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let db_ok = sqlx::query("SELECT 1").fetch_one(&state.db).await.is_ok();
    let nodes = state.valid_node_ids.len();
    if db_ok {
        (StatusCode::OK, Json(json!({ "status": "ok", "nodes": nodes, "db": "ok" })))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "status": "degraded", "nodes": nodes, "db": "down" })))
    }
}
