use axum::{extract::State, Json};
use serde_json::{json, Value};
use crate::state::AppState;

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    let node_count = state.valid_node_ids.len();
    Json(json!({
        "status": "ok",
        "nodes": node_count,
    }))
}
