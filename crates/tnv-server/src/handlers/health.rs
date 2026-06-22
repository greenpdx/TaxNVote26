use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};
use crate::models::*;
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

/// Public, unauthenticated runtime config the SPA needs before sign-in:
/// the admin-editable header subtitles and landing-page copy (with defaults).
pub async fn public_config(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "subtitle_1":   state.setting_str("subtitle_1", DEFAULT_SUBTITLE_1).await,
        "subtitle_2":   state.setting_str("subtitle_2", DEFAULT_SUBTITLE_2).await,
        "lp_kicker":    state.setting_str("lp_kicker", DEFAULT_LP_KICKER).await,
        "lp_headline":  state.setting_str("lp_headline", DEFAULT_LP_HEADLINE).await,
        "lp_pitch":     state.setting_str("lp_pitch", DEFAULT_LP_PITCH).await,
        "lp_why":       state.setting_str("lp_why", DEFAULT_LP_WHY).await,
        "lp_pb_intro":  state.setting_str("lp_pb_intro", DEFAULT_LP_PB_INTRO).await,
        "lp_pillars":   state.setting_str("lp_pillars", DEFAULT_LP_PILLARS).await,
        "lp_pb_footer": state.setting_str("lp_pb_footer", DEFAULT_LP_PB_FOOTER).await,
    }))
}
