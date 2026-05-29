// src/node.rs
//
// Data structures that live entirely inside Rust/WASM memory.
// Tree is built once from a CBO budget-authority CSV (embedded as static asset),
// then structure is frozen — only values and lock flags change at runtime.

use std::collections::HashMap;

// ─── Configuration ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BudgetConfig {
    /// Floor = default_value × this fraction.  Range 0.0–0.5.
    pub min_fraction_of_default: f64,
    /// Smallest displayable increment (0.01 = two decimal places).
    pub precision: f64,
    /// Force sibling sums to exactly equal parent after every adjustment.
    pub enforce_exact_sum: bool,
    /// Which fiscal year column to use as default values.
    pub fiscal_year: String,
    /// BEA category filter: "Discretionary", "Mandatory", etc. or "" for all.
    pub bea_filter: String,
    /// true = on-budget only, false = include off-budget.
    pub on_budget_only: bool,
}

impl Default for BudgetConfig {
    /// `fiscal_year` is intentionally empty — callers must set it explicitly
    /// (the CSV parser will reject an empty fiscal_year with a clear error).
    fn default() -> Self {
        Self {
            min_fraction_of_default: 0.0,
            precision: 0.01,
            enforce_exact_sum: true,
            fiscal_year: String::new(),
            bea_filter: "Discretionary".to_string(),
            on_budget_only: true,
        }
    }
}

// ─── Node ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BudgetNode {
    pub idx: usize,
    /// "root", "t:env", "a:env:010", "b:env:010:01", "c:env:010:01:1000"
    pub id: String,
    pub name: String,
    /// Current value (thousands of dollars).  ONLY mutable field in adjust.
    pub value: f64,
    /// Value from embedded CSV at build time.
    pub default_value: f64,
    /// Value from last applied template (if any). Initially == default_value.
    pub template_value: f64,
    pub locked: bool,
    /// Parent index. usize::MAX = root.
    pub parent: usize,
    pub children: Vec<usize>,
    pub level: u8,
}

impl BudgetNode {
    #[inline]
    pub fn min_value(&self, cfg: &BudgetConfig) -> f64 {
        self.default_value * cfg.min_fraction_of_default
    }
    #[inline]
    pub fn is_root(&self) -> bool { self.parent == usize::MAX }
    #[inline]
    pub fn is_leaf(&self) -> bool { self.children.is_empty() }
}

// ─── Changeset ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Change {
    pub idx: usize,
    pub new_val: f64,
}

// ─── Errors ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AdjustError {
    NodeNotFound(String),
    NodeLocked(String),
    CannotAdjustRoot,
    NoUnlockedSiblings(String),
    ValueOutOfBounds { node: String, requested: f64, min: f64, max: f64 },
}

impl std::fmt::Display for AdjustError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NodeNotFound(id) => write!(f, "node not found: {id}"),
            Self::NodeLocked(id) => write!(f, "node is locked: {id}"),
            Self::CannotAdjustRoot => write!(f, "cannot adjust root directly"),
            Self::NoUnlockedSiblings(id) => write!(f, "no unlocked siblings for: {id}"),
            Self::ValueOutOfBounds { node, requested, min, max } =>
                write!(f, "{node}: {requested} out of bounds [{min}, {max}]"),
        }
    }
}

// ─── Tree ────────────────────────────────────────────────────────

pub struct BudgetTree {
    pub nodes: Vec<BudgetNode>,
    pub id_map: HashMap<String, usize>,
    pub config: BudgetConfig,
}

// ─── CSV record (intermediate) ───────────────────────────────────

#[derive(Debug)]
pub struct CsvRecord {
    pub agency_code: String,
    pub agency_name: String,
    pub bureau_code: String,
    pub bureau_name: String,
    pub account_code: String,
    pub account_name: String,
    pub bea_category: String,
    pub on_off_budget: String,
    pub year_value: f64,
}

// ─── CSV parser ──────────────────────────────────────────────────

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => { fields.push(current.clone()); current.clear(); }
            '\r' => {}
            _ => current.push(ch),
        }
    }
    fields.push(current);
    fields
}

// ─── Tree construction ───────────────────────────────────────────

impl BudgetTree {
    /// Build from CSV text.  Called once at init.
    pub fn from_csv(csv_text: &str, config: BudgetConfig) -> Result<Self, String> {
        let records = Self::parse_csv(csv_text, &config)?;
        Self::build_tree(records, config)
    }

    fn parse_csv(csv_text: &str, config: &BudgetConfig) -> Result<Vec<CsvRecord>, String> {
        let mut lines = csv_text.lines();
        let header_line = lines.next().ok_or("empty CSV")?;
        let headers = parse_csv_line(header_line);

        let year_col = headers.iter().position(|h| h.trim() == config.fiscal_year)
            .ok_or_else(|| format!("fiscal year '{}' not found in headers", config.fiscal_year))?;

        // Detect format: if CGAC Agency Code column exists, BEA/OnOff shift by 1
        let has_cgac = headers.iter().any(|h| h.trim() == "CGAC Agency Code");
        let col_bea = if has_cgac { 10 } else { 9 };
        let col_on_off = if has_cgac { 11 } else { 10 };

        let mut records = Vec::new();
        for line in lines {
            let line = line.trim();
            if line.is_empty() { continue; }
            let fields = parse_csv_line(line);
            if fields.len() <= year_col { continue; }

            // Skip parent rows (root, agency, bureau totals) — they have empty account code
            let account_code = fields.get(4).map(|s| s.trim()).unwrap_or("");
            if account_code.is_empty() { continue; }

            let bea = fields.get(col_bea).map(|s| s.trim()).unwrap_or("");
            if !config.bea_filter.is_empty() && bea != config.bea_filter { continue; }

            let on_off = fields.get(col_on_off).map(|s| s.trim()).unwrap_or("");
            if config.on_budget_only && on_off != "On-budget" { continue; }

            let raw_val = fields[year_col].trim().replace(',', "");
            let val: f64 = raw_val.parse().unwrap_or(0.0);
            // Drop empty and offsetting-collection / receivable accounts: the
            // budget is spending only, and the allocation algorithm assumes
            // non-negative values.
            if val <= 0.0 { continue; }

            records.push(CsvRecord {
                agency_code:   fields.get(0).map(|s| s.trim().to_string()).unwrap_or_default(),
                agency_name:   fields.get(1).map(|s| s.trim().to_string()).unwrap_or_default(),
                bureau_code:   fields.get(2).map(|s| s.trim().to_string()).unwrap_or_default(),
                bureau_name:   fields.get(3).map(|s| s.trim().to_string()).unwrap_or_default(),
                account_code:  account_code.to_string(),
                account_name:  fields.get(5).map(|s| s.trim().to_string()).unwrap_or_default(),
                bea_category:  bea.to_string(),
                on_off_budget: on_off.to_string(),
                year_value:    val,
            });
        }
        if records.is_empty() {
            return Err("no matching records after filtering".to_string());
        }
        Ok(records)
    }

    fn build_tree(records: Vec<CsvRecord>, config: BudgetConfig) -> Result<Self, String> {
        let mut nodes: Vec<BudgetNode> = Vec::new();
        let mut id_map: HashMap<String, usize> = HashMap::new();

        // Root = index 0
        nodes.push(BudgetNode {
            idx: 0, id: "root".into(), name: "Federal Budget".into(),
            value: 0.0, default_value: 0.0, template_value: 0.0,
            locked: false, parent: usize::MAX, children: Vec::new(), level: 0,
        });
        id_map.insert("root".into(), 0);

        // Topic layer (level 1) — the 9 simple-form categories, fixed order.
        for t in crate::topics::TOPICS {
            let idx = nodes.len();
            let id = format!("t:{}", t.id);
            nodes.push(BudgetNode {
                idx, id: id.clone(), name: t.label.into(),
                value: 0.0, default_value: 0.0, template_value: 0.0,
                locked: false, parent: 0, children: Vec::new(), level: 1,
            });
            id_map.insert(id, idx);
            nodes[0].children.push(idx);
        }

        for rec in &records {
            // Topic (level 1) — already created above.
            let topic = crate::topics::topic_for(&rec.agency_code, &rec.bureau_code);
            let topic_idx = id_map[&format!("t:{}", topic)];

            // Agency (level 2) — topic-scoped, since one agency may split across
            // topics by bureau (e.g. Interior -> Science/Education/Environment).
            let agency_id = format!("a:{}:{}", topic, rec.agency_code);
            let agency_idx = if let Some(&idx) = id_map.get(&agency_id) {
                idx
            } else {
                let idx = nodes.len();
                nodes.push(BudgetNode {
                    idx, id: agency_id.clone(), name: rec.agency_name.clone(),
                    value: 0.0, default_value: 0.0, template_value: 0.0,
                    locked: false, parent: topic_idx, children: Vec::new(), level: 2,
                });
                id_map.insert(agency_id, idx);
                nodes[topic_idx].children.push(idx);
                idx
            };

            // Bureau (level 3)
            let bureau_id = format!("b:{}:{}:{}", topic, rec.agency_code, rec.bureau_code);
            let bureau_idx = if let Some(&idx) = id_map.get(&bureau_id) {
                idx
            } else {
                let idx = nodes.len();
                nodes.push(BudgetNode {
                    idx, id: bureau_id.clone(), name: rec.bureau_name.clone(),
                    value: 0.0, default_value: 0.0, template_value: 0.0,
                    locked: false, parent: agency_idx, children: Vec::new(), level: 3,
                });
                id_map.insert(bureau_id, idx);
                nodes[agency_idx].children.push(idx);
                idx
            };

            // Account (leaf, level 4). Rows sharing (topic, agency, bureau,
            // account) but differing only by subfunction are merged into one
            // leaf (summed) so every node id is unique and CSV-derivable — no
            // synthetic "#N" ids that the server's validator couldn't know.
            let acct_id = format!("c:{}:{}:{}:{}", topic, rec.agency_code, rec.bureau_code, rec.account_code);
            if let Some(&existing) = id_map.get(&acct_id) {
                nodes[existing].value += rec.year_value;
                nodes[existing].default_value += rec.year_value;
                nodes[existing].template_value += rec.year_value;
            } else {
                let leaf_idx = nodes.len();
                nodes.push(BudgetNode {
                    idx: leaf_idx, id: acct_id.clone(), name: rec.account_name.clone(),
                    value: rec.year_value, default_value: rec.year_value, template_value: rec.year_value,
                    locked: false, parent: bureau_idx, children: Vec::new(), level: 4,
                });
                id_map.insert(acct_id, leaf_idx);
                nodes[bureau_idx].children.push(leaf_idx);
            }
        }

        // Sum bottom-up: level 3, 2, 1, 0
        for level in (0u8..=3).rev() {
            let indices: Vec<usize> = nodes.iter()
                .filter(|n| n.level == level).map(|n| n.idx).collect();
            for pidx in indices {
                let s: f64 = nodes[pidx].children.iter().map(|&c| nodes[c].value).sum();
                nodes[pidx].value = s;
                nodes[pidx].default_value = s;
                nodes[pidx].template_value = s;
            }
        }

        Ok(BudgetTree { nodes, id_map, config })
    }

    // ─── Accessors ───────────────────────────────────────────────

    pub fn lookup(&self, id: &str) -> Option<usize> {
        self.id_map.get(id).copied()
    }

    pub fn get_value(&self, id: &str) -> Option<f64> {
        self.lookup(id).map(|i| self.nodes[i].value)
    }

    pub fn all_values(&self) -> Vec<f64> {
        self.nodes.iter().map(|n| n.value).collect()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Apply a template: sparse map of id → new_value.
    /// Only changes values; structure is untouched.
    /// Stores values as template_value for reset-to-template.
    pub fn apply_template(&mut self, entries: &[(String, f64)]) -> Vec<Change> {
        let mut changes = Vec::new();
        // Apply leaf/explicit values first
        for (id, val) in entries {
            if let Some(idx) = self.lookup(id) {
                self.nodes[idx].value = *val;
                self.nodes[idx].template_value = *val;
                changes.push(Change { idx, new_val: *val });
            }
        }
        // Recompute parent sums bottom-up
        for level in (0u8..=3).rev() {
            let indices: Vec<usize> = self.nodes.iter()
                .filter(|n| n.level == level).map(|n| n.idx).collect();
            for pidx in indices {
                let s: f64 = self.nodes[pidx].children.iter().map(|&c| self.nodes[c].value).sum();
                if (self.nodes[pidx].value - s).abs() > 0.001 {
                    self.nodes[pidx].value = s;
                    self.nodes[pidx].template_value = s;
                    changes.push(Change { idx: pidx, new_val: s });
                }
            }
        }
        changes
    }

    /// Reset all nodes to embedded default values.
    pub fn reset_to_default(&mut self) -> Vec<Change> {
        let mut changes = Vec::new();
        for i in 0..self.nodes.len() {
            if (self.nodes[i].value - self.nodes[i].default_value).abs() > 0.001 {
                self.nodes[i].value = self.nodes[i].default_value;
                changes.push(Change { idx: i, new_val: self.nodes[i].default_value });
            }
            self.nodes[i].locked = false;
        }
        changes
    }

    /// Reset all nodes to last applied template values.
    pub fn reset_to_template(&mut self) -> Vec<Change> {
        let mut changes = Vec::new();
        for i in 0..self.nodes.len() {
            if (self.nodes[i].value - self.nodes[i].template_value).abs() > 0.001 {
                self.nodes[i].value = self.nodes[i].template_value;
                changes.push(Change { idx: i, new_val: self.nodes[i].template_value });
            }
            self.nodes[i].locked = false;
        }
        changes
    }
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn test_csv() -> &'static str {
        "Agency Code,Agency Name,Bureau Code,Bureau Name,Account Code,Account Name,Treasury Agency Code,Subfunction Code,Subfunction Title,BEA Category,On- or Off- Budget,2021\n\
         010,Defense,01,Army,1000,Operations,10,051,Dept of Defense,Discretionary,On-budget,\"300,000\"\n\
         010,Defense,01,Army,2000,Personnel,10,051,Dept of Defense,Discretionary,On-budget,\"200,000\"\n\
         010,Defense,02,Navy,1000,Ship Ops,10,051,Dept of Defense,Discretionary,On-budget,\"250,000\"\n\
         020,Education,01,K-12,1000,Grants,20,501,Education,Discretionary,On-budget,\"100,000\"\n\
         020,Education,01,K-12,2000,Programs,20,501,Education,Discretionary,On-budget,\"50,000\""
    }

    pub fn test_config() -> BudgetConfig {
        BudgetConfig {
            fiscal_year: "2021".into(),
            bea_filter: "Discretionary".into(),
            on_budget_only: true,
            ..Default::default()
        }
    }

    pub fn make_tree() -> BudgetTree {
        BudgetTree::from_csv(test_csv(), test_config()).unwrap()
    }

    #[test]
    fn test_build_tree_structure() {
        let tree = make_tree();
        assert_eq!(tree.len(), 20); // root + 9 topics + 2 agencies + 3 bureaus + 5 accounts
        assert_eq!(tree.nodes[0].value, 900_000.0);
    }

    #[test]
    fn test_parent_child_bidirectional() {
        let tree = make_tree();
        for node in &tree.nodes {
            if !node.is_root() {
                assert!(tree.nodes[node.parent].children.contains(&node.idx));
            }
            for &c in &node.children {
                assert_eq!(tree.nodes[c].parent, node.idx);
            }
        }
    }

    #[test]
    fn test_sums_consistent() {
        let tree = make_tree();
        for node in &tree.nodes {
            if !node.is_leaf() {
                let s: f64 = node.children.iter().map(|&c| tree.nodes[c].value).sum();
                assert!((node.value - s).abs() < 0.01,
                    "{} value {} != sum {}", node.id, node.value, s);
            }
        }
    }

    #[test]
    fn test_csv_quoted_commas() {
        let line = r#"001,Leg,05,Sen,0110,"Salaries, Officers",00,801,Leg,Discretionary,On-budget,"126,428""#;
        let f = parse_csv_line(line);
        assert_eq!(f[5], "Salaries, Officers");
        assert_eq!(f[11], "126,428");
    }

    #[test]
    fn test_filter_off_budget_zero_and_negative() {
        // Off-budget, zero, and negative (offsetting-collection) accounts are all
        // excluded; only the single positive on-budget account remains.
        let csv = "Agency Code,Agency Name,Bureau Code,Bureau Name,Account Code,Account Name,Treasury Agency Code,Subfunction Code,Subfunction Title,BEA Category,On- or Off- Budget,2021\n\
                    010,Def,01,Army,1000,Ops,10,051,DoD,Discretionary,On-budget,\"100,000\"\n\
                    010,Def,01,Army,2000,Secret,10,051,DoD,Discretionary,Off-budget,\"999,999\"\n\
                    010,Def,01,Army,3000,Empty,10,051,DoD,Discretionary,On-budget,0\n\
                    010,Def,01,Army,4000,Receivable,10,051,DoD,Discretionary,On-budget,\"-50,000\"";
        let tree = BudgetTree::from_csv(csv, test_config()).unwrap();
        assert_eq!(tree.len(), 13); // root + 9 topics + 1 agency + 1 bureau + 1 account
        assert_eq!(tree.nodes[0].value, 100_000.0);
        assert!(tree.lookup("c:env:010:01:4000").is_none(), "negative account must be dropped");
    }

    #[test]
    fn test_apply_template() {
        let mut tree = make_tree();
        let leaf_idx = tree.lookup("c:env:010:01:1000").unwrap();
        assert_eq!(tree.nodes[leaf_idx].value, 300_000.0);

        let changes = tree.apply_template(&[
            ("c:env:010:01:1000".into(), 350_000.0),
        ]);
        assert!(changes.len() >= 1);
        assert_eq!(tree.nodes[leaf_idx].value, 350_000.0);
        assert_eq!(tree.nodes[leaf_idx].template_value, 350_000.0);

        // Parent sums should have updated
        let army_idx = tree.lookup("b:env:010:01").unwrap();
        assert_eq!(tree.nodes[army_idx].value, 550_000.0); // 350+200
    }

    #[test]
    fn test_reset_to_default() {
        let mut tree = make_tree();
        tree.apply_template(&[("c:env:010:01:1000".into(), 999.0)]);
        let changes = tree.reset_to_default();
        assert!(!changes.is_empty());
        let leaf_idx = tree.lookup("c:env:010:01:1000").unwrap();
        assert_eq!(tree.nodes[leaf_idx].value, 300_000.0);
    }

    #[test]
    fn test_reset_to_template() {
        let mut tree = make_tree();
        tree.apply_template(&[("c:env:010:01:1000".into(), 400_000.0)]);
        // Simulate user adjustment
        let leaf_idx = tree.lookup("c:env:010:01:1000").unwrap();
        tree.nodes[leaf_idx].value = 123.0;
        let _changes = tree.reset_to_template();
        assert_eq!(tree.nodes[leaf_idx].value, 400_000.0);
    }

    #[test]
    fn test_load_real_csv() {
        if let Ok(text) = std::fs::read_to_string("/tmp/taxnvote/budget/budauth.csv") {
            let tree = BudgetTree::from_csv(&text, test_config()).unwrap();
            assert!(tree.len() > 100);
            for node in &tree.nodes {
                if !node.is_leaf() {
                    let s: f64 = node.children.iter().map(|&c| tree.nodes[c].value).sum();
                    assert!((node.value - s).abs() < 0.01);
                }
            }
        }
    }
}
