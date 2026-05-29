// src/wasm.rs
//
// Thin wasm-bindgen shell.  BudgetTree is opaque to JS.
// JS only sends ids + f64s in, gets changesets out.
//
// Compile: wasm-pack build --target web --out-dir pkg

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "wasm")]
use crate::node::{BudgetTree, BudgetConfig};

/// The embedded default CSV (compiled into the WASM binary).
/// Replace this path with your actual budauth.csv location in the project.
#[cfg(feature = "wasm")]
const DEFAULT_CSV: &str = include_str!("../data/budauth.csv");

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub struct WasmBudgetTree {
    inner: BudgetTree,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl WasmBudgetTree {
    /// Create tree from embedded default CSV.
    /// min_pct: min fraction of default (0.0–0.5).
    /// fiscal_year: e.g. "2021".
    /// bea_filter: e.g. "Discretionary" or "" for all.
    /// on_budget_only: true = on-budget only.
    #[wasm_bindgen(constructor)]
    pub fn new(
        min_pct: f64,
        fiscal_year: &str,
        bea_filter: &str,
        on_budget_only: bool,
    ) -> Result<WasmBudgetTree, JsValue> {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        let config = BudgetConfig {
            min_fraction_of_default: min_pct.max(0.0).min(0.5),
            fiscal_year: fiscal_year.to_string(),
            bea_filter: bea_filter.to_string(),
            on_budget_only,
            ..Default::default()
        };

        let inner = BudgetTree::from_csv(DEFAULT_CSV, config)
            .map_err(|e| JsValue::from_str(&e))?;

        Ok(WasmBudgetTree { inner })
    }

    /// Number of nodes in the tree.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Get a node's current value by id.  Returns NaN if not found.
    pub fn get_value(&self, id: &str) -> f64 {
        self.inner.get_value(id).unwrap_or(f64::NAN)
    }

    /// Get ALL values as a flat f64 array.  Index = node arena index.
    /// Used once at init to populate the display layer.
    pub fn all_values(&self) -> Box<[f64]> {
        self.inner.all_values().into_boxed_slice()
    }

    /// Get all node metadata as JSON string (for initial display setup).
    /// Each entry: { idx, id, name, level, parent, value, locked }
    pub fn all_nodes_json(&self) -> String {
        let entries: Vec<String> = self.inner.nodes.iter().map(|n| {
            format!(
                r#"{{"idx":{},"id":"{}","name":"{}","level":{},"parent":{},"value":{:.2},"locked":{}}}"#,
                n.idx,
                n.id.replace('"', r#"\""#),
                n.name.replace('"', r#"\""#),
                n.level,
                if n.is_root() { -1i64 } else { n.parent as i64 },
                n.value,
                n.locked,
            )
        }).collect();
        format!("[{}]", entries.join(","))
    }

    /// Adjust a node to a new value.
    /// Returns changeset as packed f64 array: [idx0, val0, idx1, val1, ...]
    /// Empty array = no-op.  On error, returns array with single NaN.
    pub fn adjust(&mut self, id: &str, new_value: f64) -> Box<[f64]> {
        match self.inner.adjust(id, new_value) {
            Ok(changes) => {
                let mut out = Vec::with_capacity(changes.len() * 2);
                for c in changes {
                    out.push(c.idx as f64);
                    out.push(c.new_val);
                }
                out.into_boxed_slice()
            }
            Err(_e) => {
                // Return [NaN] to signal error. JS checks: if result[0] is NaN → error.
                vec![f64::NAN].into_boxed_slice()
            }
        }
    }

    /// Lock a node by id.
    pub fn lock(&mut self, id: &str) -> bool {
        self.inner.lock(id).is_ok()
    }

    /// Unlock a node by id.
    pub fn unlock(&mut self, id: &str) -> bool {
        self.inner.unlock(id).is_ok()
    }

    /// Reset one node to its default value.
    /// Returns changeset as packed f64 array.
    pub fn reset_node(&mut self, id: &str) -> Box<[f64]> {
        match self.inner.reset_node(id) {
            Ok(changes) => {
                let mut out = Vec::with_capacity(changes.len() * 2);
                for c in changes {
                    out.push(c.idx as f64);
                    out.push(c.new_val);
                }
                out.into_boxed_slice()
            }
            Err(_) => vec![f64::NAN].into_boxed_slice(),
        }
    }

    /// Reset all nodes to embedded default values.
    /// Returns changeset as packed f64 array.
    pub fn reset_to_default(&mut self) -> Box<[f64]> {
        let changes = self.inner.reset_to_default();
        let mut out = Vec::with_capacity(changes.len() * 2);
        for c in changes {
            out.push(c.idx as f64);
            out.push(c.new_val);
        }
        out.into_boxed_slice()
    }

    /// Reset all nodes to last applied template values.
    /// Returns changeset as packed f64 array.
    pub fn reset_to_template(&mut self) -> Box<[f64]> {
        let changes = self.inner.reset_to_template();
        let mut out = Vec::with_capacity(changes.len() * 2);
        for c in changes {
            out.push(c.idx as f64);
            out.push(c.new_val);
        }
        out.into_boxed_slice()
    }

    /// Apply a template: JSON string of [{"id":"...", "value": 123.0}, ...]
    /// Returns changeset as packed f64 array.
    pub fn apply_template(&mut self, json: &str) -> Box<[f64]> {
        // Minimal JSON parse — template is array of {id, value}
        let entries: Vec<(String, f64)> = Self::parse_template_json(json);
        let changes = self.inner.apply_template(&entries);
        let mut out = Vec::with_capacity(changes.len() * 2);
        for c in changes {
            out.push(c.idx as f64);
            out.push(c.new_val);
        }
        out.into_boxed_slice()
    }

    /// Validate tree invariants.  Returns empty string if healthy, errors otherwise.
    pub fn validate(&self) -> String {
        let errors = self.inner.validate();
        if errors.is_empty() { String::new() } else { errors.join("; ") }
    }

    // Internal: parse template JSON without pulling in a full JSON parser
    // at the WASM boundary (serde_json is available but we keep the
    // wasm API layer using primitives only).
    fn parse_template_json(json: &str) -> Vec<(String, f64)> {
        // Use serde_json since it's already a dependency
        if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(json) {
            arr.iter().filter_map(|obj| {
                let id = obj.get("id")?.as_str()?.to_string();
                let val = obj.get("value")?.as_f64()?;
                Some((id, val))
            }).collect()
        } else {
            Vec::new()
        }
    }
}
