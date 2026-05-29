use axum::{extract::{State, Query}, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use std::collections::HashMap;
use crate::models::*;
use crate::state::*;

#[derive(Debug, Deserialize)]
pub struct AggQuery {
    pub fiscal_year: Option<String>,
}

// ─── GET /api/aggregate ──────────────────────────────────────────
// Public "People's Budget": per-node statistics across all submitted Tax
// Dollars for a fiscal year. Each submission's leaf allocations are rolled up
// to every ancestor (topic/agency/bureau) by parsing the node id. Cached per
// fiscal_year; recomputed only after a submission changes the data.
pub async fn aggregate(
    State(state): State<AppState>,
    Query(q): Query<AggQuery>,
) -> Result<Json<AggregateResponse>, (StatusCode, Json<Value>)> {
    let fy = q.fiscal_year.unwrap_or_else(|| state.fiscal_year.clone());

    // Serve cached result if present (no change since last compute).
    if let Some(cached) = state.aggregate_cache.read().await.get(&fy) {
        return Ok(Json(cached.clone()));
    }

    let result = compute_aggregate(&state, &fy).await
        .map_err(|e| internal(e.to_string()))?;
    state.aggregate_cache.write().await.insert(fy, result.clone());
    Ok(Json(result))
}

async fn compute_aggregate(state: &AppState, fy: &str) -> Result<AggregateResponse, sqlx::Error> {
    // All submissions for the year (one per person).
    let td_rows = sqlx::query(&state.q(
        "SELECT id FROM tax_dollars WHERE fiscal_year = ?"
    ))
        .bind(fy)
        .fetch_all(&state.db).await?;
    let td_ids: Vec<i64> = td_rows.iter().filter_map(|r| r.try_get("id").ok()).collect();
    let n = td_ids.len();

    if n == 0 {
        return Ok(AggregateResponse { fiscal_year: fy.to_string(), submission_count: 0, nodes: Vec::new() });
    }

    // Per submission: roll leaf allocations up to every ancestor node.
    let mut per_person: Vec<HashMap<String, f64>> = Vec::with_capacity(n);
    let mut all_nodes: std::collections::HashSet<String> = std::collections::HashSet::new();

    for td_id in &td_ids {
        let alloc_rows = sqlx::query(&state.q(
            "SELECT node_id, pct FROM tax_dollar_allocations WHERE tax_dollar_id = ?"
        ))
            .bind(td_id)
            .fetch_all(&state.db).await?;

        let mut map: HashMap<String, f64> = HashMap::new();
        for r in &alloc_rows {
            let node_id: String = r.try_get("node_id").unwrap_or_default();
            let pct: f64 = r.try_get("pct").unwrap_or(0.0);
            for ancestor in ancestors_of(&node_id) {
                *map.entry(ancestor.clone()).or_insert(0.0) += pct;
                all_nodes.insert(ancestor);
            }
        }
        per_person.push(map);
    }

    // Per node: build the sample (0 for submitters who didn't allocate there).
    let mut nodes: Vec<NodeStat> = Vec::with_capacity(all_nodes.len());
    for node_id in all_nodes {
        let values: Vec<f64> = per_person.iter()
            .map(|m| *m.get(&node_id).unwrap_or(&0.0))
            .collect();
        nodes.push(node_stat(node_id, values));
    }
    nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));

    Ok(AggregateResponse { fiscal_year: fy.to_string(), submission_count: n, nodes })
}

/// A leaf id "c:topic:agency:bureau:account" rolls up to itself plus
/// b:topic:agency:bureau, a:topic:agency, t:topic. Non-leaf ids return self.
fn ancestors_of(node_id: &str) -> Vec<String> {
    let parts: Vec<&str> = node_id.split(':').collect();
    match parts.as_slice() {
        ["c", t, a, b, _acct] => vec![
            format!("t:{t}"),
            format!("a:{t}:{a}"),
            format!("b:{t}:{a}:{b}"),
            node_id.to_string(),
        ],
        ["b", t, a, _b] => vec![format!("t:{t}"), format!("a:{t}:{a}"), node_id.to_string()],
        ["a", t, _a] => vec![format!("t:{t}"), node_id.to_string()],
        _ => vec![node_id.to_string()],
    }
}

fn node_stat(node_id: String, mut v: Vec<f64>) -> NodeStat {
    let count = v.len();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let sum: f64 = v.iter().sum();
    let mean = sum / count as f64;
    let min = *v.first().unwrap();
    let max = *v.last().unwrap();
    let median = if count % 2 == 1 {
        v[count / 2]
    } else {
        (v[count / 2 - 1] + v[count / 2]) / 2.0
    };
    let variance = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / count as f64;
    let std_dev = variance.sqrt();
    // Trimmed mean: drop k from each end.
    let k = (TRIM_FRACTION * count as f64).floor() as usize;
    let trimmed_mean = if 2 * k < count {
        let slice = &v[k..count - k];
        slice.iter().sum::<f64>() / slice.len() as f64
    } else {
        mean
    };
    NodeStat { node_id, count, mean, median, trimmed_mean, std_dev, min, max }
}

fn internal(msg: String) -> (StatusCode, Json<Value>) {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg})))
}
