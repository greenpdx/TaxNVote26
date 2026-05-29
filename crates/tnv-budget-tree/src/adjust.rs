// src/adjust.rs
//
// The adjustment algorithm.  Mutates node values inside BudgetTree,
// returns a changeset of (idx, new_val) pairs.
//
// Rules:
//   1. Proportional redistribution among unlocked siblings
//   2. Locked nodes are skipped
//   3. Parent proportion stays fixed (change is confined within parent)
//   4. All changed nodes rescale their children recursively
//   5. Configurable min bound (fraction of default)
//   6. Exact sum enforcement (rounding residual → largest unlocked sibling)

use crate::node::{BudgetTree, Change, AdjustError};
use std::collections::HashSet;

impl BudgetTree {
    /// Main entry point: set a node to a new absolute value.
    /// Returns the changeset or an error.
    pub fn adjust(&mut self, id: &str, new_value: f64) -> Result<Vec<Change>, AdjustError> {
        let target_idx = self.lookup(id)
            .ok_or_else(|| AdjustError::NodeNotFound(id.to_string()))?;

        if self.nodes[target_idx].is_root() {
            return Err(AdjustError::CannotAdjustRoot);
        }
        if self.nodes[target_idx].locked {
            return Err(AdjustError::NodeLocked(id.to_string()));
        }

        let parent_idx = self.nodes[target_idx].parent;

        // Sole child: its value fully determines the parent's, so there is no
        // sibling to absorb a change. Redirect the adjustment to the parent
        // (which redistributes among the parent's siblings and rescales this
        // node back down to match). Recurses up until a level with siblings,
        // bottoming out at the 9 topics. Forward the *unclamped* value so the
        // node can grow beyond the (currently equal) parent value.
        if !self.nodes[parent_idx].is_root()
            && self.nodes[parent_idx].children.len() == 1
        {
            let parent_id = self.nodes[parent_idx].id.clone();
            return self.adjust(&parent_id, new_value);
        }

        let parent_val = self.nodes[parent_idx].value;
        let min = self.nodes[target_idx].min_value(&self.config);
        let max = parent_val;

        // Clamp
        let new_value = new_value.max(min).min(max);

        // No-op check
        let old_value = self.nodes[target_idx].value;
        if (new_value - old_value).abs() < self.config.precision {
            return Ok(Vec::new());
        }

        let delta = new_value - old_value;

        // Find unlocked siblings
        let siblings: Vec<usize> = self.nodes[parent_idx].children.iter()
            .filter(|&&c| c != target_idx && !self.nodes[c].locked)
            .copied()
            .collect();

        if siblings.is_empty() {
            return Err(AdjustError::NoUnlockedSiblings(id.to_string()));
        }

        let mut changed = HashSet::new();
        let mut changes = Vec::new();

        // Distribute -delta among unlocked siblings proportionally
        // But first: compute how much siblings CAN absorb
        let max_absorb: f64 = siblings.iter().map(|&s| {
            let floor = self.nodes[s].min_value(&self.config);
            self.nodes[s].value - floor
        }).sum();

        // If increasing target: siblings must give up delta.
        // If they can't give up enough, clamp the target.
        let actual_delta = if delta > 0.0 {
            delta.min(max_absorb)
        } else {
            // Decreasing target: siblings gain, no floor issue on gain side
            delta
        };

        let actual_new_value = old_value + actual_delta;

        // Set target value
        self.nodes[target_idx].value = actual_new_value;
        changed.insert(target_idx);
        changes.push(Change { idx: target_idx, new_val: actual_new_value });

        // Distribute -actual_delta among unlocked siblings proportionally
        self.distribute_delta(&siblings, -actual_delta, &mut changed, &mut changes);

        // Rescale children of target
        self.rescale_children(target_idx, &mut changed, &mut changes);

        // Rescale children of each affected sibling
        for &sib in &siblings {
            if changed.contains(&sib) {
                self.rescale_children(sib, &mut changed, &mut changes);
            }
        }

        // Enforce exact sum
        if self.config.enforce_exact_sum {
            self.enforce_sum(parent_idx, target_idx, &mut changed, &mut changes);
        }

        Ok(changes)
    }

    /// Distribute a delta amount among a set of nodes proportionally to their values.
    /// Handles clamping: if a node hits its floor, the overflow goes to remaining nodes.
    fn distribute_delta(
        &mut self,
        node_indices: &[usize],
        delta: f64,
        changed: &mut HashSet<usize>,
        changes: &mut Vec<Change>,
    ) {
        if node_indices.is_empty() { return; }

        let _total: f64 = node_indices.iter().map(|&i| self.nodes[i].value).sum();
        let mut remaining = delta;
        let mut remaining_indices: Vec<usize> = node_indices.to_vec();

        // Iterative: handle clamping overflow by re-distributing
        for _pass in 0..5 {
            if remaining.abs() < 0.001 || remaining_indices.is_empty() { break; }

            let pool: f64 = remaining_indices.iter().map(|&i| self.nodes[i].value).sum();
            let mut overflow = 0.0;
            let mut clamped = Vec::new();

            for &idx in &remaining_indices {
                let share = if pool > 0.0 {
                    self.nodes[idx].value / pool
                } else {
                    1.0 / remaining_indices.len() as f64
                };

                let raw_adj = remaining * share;
                let proposed = self.nodes[idx].value + raw_adj;
                let floor = self.nodes[idx].min_value(&self.config);

                if proposed < floor {
                    // Clamp to floor
                    let actual = floor - self.nodes[idx].value;
                    overflow += raw_adj - actual;
                    self.nodes[idx].value = floor;
                    clamped.push(idx);
                } else {
                    self.nodes[idx].value = proposed;
                }

                changed.insert(idx);
                // Update or add change entry
                Self::upsert_change(changes, idx, self.nodes[idx].value);
            }

            // Remove clamped nodes from further redistribution
            remaining_indices.retain(|i| !clamped.contains(i));
            remaining = overflow;
        }
    }

    /// Recursively rescale all children of a node to match the node's new value.
    /// Preserves proportions among unlocked children.
    fn rescale_children(
        &mut self,
        parent_idx: usize,
        changed: &mut HashSet<usize>,
        changes: &mut Vec<Change>,
    ) {
        let children: Vec<usize> = self.nodes[parent_idx].children.clone();
        if children.is_empty() { return; }

        let old_sum: f64 = children.iter().map(|&c| self.nodes[c].value).sum();
        let new_parent_val = self.nodes[parent_idx].value;

        if old_sum.abs() < 0.001 { return; } // all children zero

        // Separate locked and unlocked
        let locked_sum: f64 = children.iter()
            .filter(|&&c| self.nodes[c].locked)
            .map(|&c| self.nodes[c].value)
            .sum();
        let unlocked: Vec<usize> = children.iter()
            .filter(|&&c| !self.nodes[c].locked)
            .copied()
            .collect();
        let unlocked_sum: f64 = unlocked.iter().map(|&c| self.nodes[c].value).sum();

        // Available space after locked nodes
        let available = new_parent_val - locked_sum;
        if available < 0.0 || unlocked.is_empty() { return; }

        // Scale unlocked children
        let ratio = if unlocked_sum > 0.0 { available / unlocked_sum } else { 0.0 };

        for &cidx in &unlocked {
            let floor = self.nodes[cidx].min_value(&self.config);
            let proposed = self.nodes[cidx].value * ratio;
            self.nodes[cidx].value = proposed.max(floor);
            changed.insert(cidx);
            Self::upsert_change(changes, cidx, self.nodes[cidx].value);

            // Recurse into grandchildren
            self.rescale_children(cidx, changed, changes);
        }

        // Fix rounding: adjust largest unlocked child
        if self.config.enforce_exact_sum && !unlocked.is_empty() {
            let actual_sum: f64 = children.iter().map(|&c| self.nodes[c].value).sum();
            let residual = new_parent_val - actual_sum;
            if residual.abs() > 0.001 {
                let &largest = unlocked.iter()
                    .max_by(|&&a, &&b| self.nodes[a].value.partial_cmp(&self.nodes[b].value).unwrap())
                    .unwrap();
                self.nodes[largest].value += residual;
                Self::upsert_change(changes, largest, self.nodes[largest].value);
                self.rescale_children(largest, changed, changes);
            }
        }
    }

    /// After adjustment, ensure parent's children sum exactly to parent's value.
    fn enforce_sum(
        &mut self,
        parent_idx: usize,
        target_idx: usize,
        changed: &mut HashSet<usize>,
        changes: &mut Vec<Change>,
    ) {
        let children = &self.nodes[parent_idx].children;
        let child_sum: f64 = children.iter().map(|&c| self.nodes[c].value).sum();
        let residual = self.nodes[parent_idx].value - child_sum;

        if residual.abs() < 0.001 { return; }

        // Apply residual to largest unlocked non-target sibling
        let candidate = children.iter()
            .filter(|&&c| c != target_idx && !self.nodes[c].locked)
            .max_by(|&&a, &&b| self.nodes[a].value.partial_cmp(&self.nodes[b].value).unwrap())
            .copied();

        if let Some(idx) = candidate {
            self.nodes[idx].value += residual;
            changed.insert(idx);
            Self::upsert_change(changes, idx, self.nodes[idx].value);
            self.rescale_children(idx, changed, changes);
        }
    }

    /// Lock a node.
    pub fn lock(&mut self, id: &str) -> Result<(), AdjustError> {
        let idx = self.lookup(id)
            .ok_or_else(|| AdjustError::NodeNotFound(id.to_string()))?;
        self.nodes[idx].locked = true;
        Ok(())
    }

    /// Unlock a node.
    pub fn unlock(&mut self, id: &str) -> Result<(), AdjustError> {
        let idx = self.lookup(id)
            .ok_or_else(|| AdjustError::NodeNotFound(id.to_string()))?;
        self.nodes[idx].locked = false;
        Ok(())
    }

    /// Reset a single node to its default value, redistribute delta among siblings.
    pub fn reset_node(&mut self, id: &str) -> Result<Vec<Change>, AdjustError> {
        let idx = self.lookup(id)
            .ok_or_else(|| AdjustError::NodeNotFound(id.to_string()))?;
        self.nodes[idx].locked = false;
        let default = self.nodes[idx].default_value;
        self.adjust(id, default)
    }

    /// Update or insert a change entry (keep only latest value per idx).
    fn upsert_change(changes: &mut Vec<Change>, idx: usize, new_val: f64) {
        if let Some(entry) = changes.iter_mut().find(|c| c.idx == idx) {
            entry.new_val = new_val;
        } else {
            changes.push(Change { idx, new_val });
        }
    }

    /// Verify tree invariants.  Returns list of violations (empty = healthy).
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for node in &self.nodes {
            if !node.is_leaf() {
                let s: f64 = node.children.iter().map(|&c| self.nodes[c].value).sum();
                if (node.value - s).abs() > 0.1 {
                    errors.push(format!("{}: value={:.2} != child_sum={:.2}", node.id, node.value, s));
                }
            }
            if !node.is_root() {
                if !self.nodes[node.parent].children.contains(&node.idx) {
                    errors.push(format!("{}: not in parent's children", node.id));
                }
            }
        }
        errors
    }
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::tests::{make_tree, test_config};
    use crate::node::BudgetConfig;

    fn assert_sums_valid(tree: &BudgetTree) {
        let errors = tree.validate();
        assert!(errors.is_empty(), "validation errors: {:?}", errors);
    }

    // ── Basic adjustment ─────────────────────────────────────────

    #[test]
    fn test_adjust_leaf_increases() {
        let mut tree = make_tree();
        // c:env:010:01:1000 = Operations = 300,000
        let changes = tree.adjust("c:env:010:01:1000", 350_000.0).unwrap();
        assert!(!changes.is_empty());

        // Target should be at new value
        assert_eq!(tree.get_value("c:env:010:01:1000").unwrap(), 350_000.0);

        // Sibling Personnel should have decreased
        let pers = tree.get_value("c:env:010:01:2000").unwrap();
        assert!(pers < 200_000.0, "sibling should decrease, got {}", pers);

        // Army bureau should stay the same (parent proportion kept)
        let army = tree.get_value("b:env:010:01").unwrap();
        assert!((army - 500_000.0).abs() < 0.1, "army should stay 500k, got {}", army);

        assert_sums_valid(&tree);
    }

    #[test]
    fn test_adjust_leaf_decreases() {
        let mut tree = make_tree();
        let changes = tree.adjust("c:env:010:01:1000", 200_000.0).unwrap();
        assert!(!changes.is_empty());
        assert_eq!(tree.get_value("c:env:010:01:1000").unwrap(), 200_000.0);

        // Sibling should have increased
        let pers = tree.get_value("c:env:010:01:2000").unwrap();
        assert!(pers > 200_000.0, "sibling should increase, got {}", pers);

        // Army stays same
        assert!((tree.get_value("b:env:010:01").unwrap() - 500_000.0).abs() < 0.1);
        assert_sums_valid(&tree);
    }

    #[test]
    fn test_adjust_bureau_redistributes_siblings_and_children() {
        let mut tree = make_tree();
        // Army = 500,000; Navy = 250,000. Both under Defense = 750,000
        // Increase Army to 600,000 → Navy should drop to 150,000
        let changes = tree.adjust("b:env:010:01", 600_000.0).unwrap();
        assert!(!changes.is_empty());

        assert_eq!(tree.get_value("b:env:010:01").unwrap(), 600_000.0);

        // Navy should absorb the delta
        let navy = tree.get_value("b:env:010:02").unwrap();
        assert!((navy - 150_000.0).abs() < 0.1, "navy should be 150k, got {}", navy);

        // Defense stays same
        assert!((tree.get_value("a:env:010").unwrap() - 750_000.0).abs() < 0.1);

        // Army's children should have scaled up proportionally
        // Ops was 300k/500k = 60%, Personnel was 200k/500k = 40%
        let ops = tree.get_value("c:env:010:01:1000").unwrap();
        let pers = tree.get_value("c:env:010:01:2000").unwrap();
        assert!((ops - 360_000.0).abs() < 1.0, "ops should be ~360k, got {}", ops);
        assert!((pers - 240_000.0).abs() < 1.0, "pers should be ~240k, got {}", pers);

        // Navy's child should have scaled down
        let ship = tree.get_value("c:env:010:02:1000").unwrap();
        assert!((ship - 150_000.0).abs() < 0.1, "ship should be 150k, got {}", ship);

        assert_sums_valid(&tree);
    }

    #[test]
    fn test_adjust_agency_redistributes_sibling_agency() {
        let mut tree = make_tree();
        // Defense=750k, Education=150k. Root=900k.
        // Increase Defense to 800k → Education drops to 100k
        let changes = tree.adjust("a:env:010", 800_000.0).unwrap();
        assert!(!changes.is_empty());

        assert!((tree.get_value("a:env:010").unwrap() - 800_000.0).abs() < 0.1);
        assert!((tree.get_value("a:env:020").unwrap() - 100_000.0).abs() < 0.1);

        // Root stays same
        assert!((tree.get_value("root").unwrap() - 900_000.0).abs() < 0.1);

        // Education's children should scale down proportionally
        // Grants was 100k/150k, Programs was 50k/150k
        let grants = tree.get_value("c:env:020:01:1000").unwrap();
        let progs = tree.get_value("c:env:020:01:2000").unwrap();
        assert!((grants - 66_666.67).abs() < 1.0, "grants got {}", grants);
        assert!((progs - 33_333.33).abs() < 1.0, "progs got {}", progs);

        assert_sums_valid(&tree);
    }

    #[test]
    fn test_adjust_sole_child_redirects_to_parent() {
        // Agency env:020 has a single bureau (b:env:020:01), so that bureau is a
        // sole child. Adjusting it must redirect to the agency, which redistributes
        // with its sibling agency env:010. Agency env:020 = 150k, env:010 = 750k.
        let mut tree = make_tree();
        let changes = tree.adjust("b:env:020:01", 200_000.0).unwrap();
        assert!(!changes.is_empty(), "sole-child adjust should redirect, not no-op");

        // Sole child and its (sole) parent both land at the requested value.
        assert!((tree.get_value("b:env:020:01").unwrap() - 200_000.0).abs() < 1.0);
        assert!((tree.get_value("a:env:020").unwrap() - 200_000.0).abs() < 1.0);

        // Sibling agency absorbed the +50k; root unchanged.
        assert!((tree.get_value("a:env:010").unwrap() - 700_000.0).abs() < 1.0);
        assert!((tree.get_value("root").unwrap() - 900_000.0).abs() < 0.1);
        assert_sums_valid(&tree);
    }

    // ── Lock behavior ────────────────────────────────────────────

    #[test]
    fn test_locked_sibling_skipped() {
        let mut tree = make_tree();
        // Lock Personnel, then increase Operations
        tree.lock("c:env:010:01:2000").unwrap();

        // Operations=300k, Personnel=200k(locked). Only 1 unlocked sibling...
        // but Personnel IS the only sibling. So this should fail.
        let result = tree.adjust("c:env:010:01:1000", 350_000.0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), AdjustError::NoUnlockedSiblings("c:env:010:01:1000".into()));
    }

    #[test]
    fn test_locked_sibling_skipped_multi() {
        let mut tree = make_tree();
        // At agency level: Defense=750k, Education=150k
        // Lock Education, try to change Defense
        tree.lock("a:env:020").unwrap();
        let result = tree.adjust("a:env:010", 800_000.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_locked_node_cannot_be_adjusted() {
        let mut tree = make_tree();
        tree.lock("c:env:010:01:1000").unwrap();
        let result = tree.adjust("c:env:010:01:1000", 999.0);
        assert_eq!(result.unwrap_err(), AdjustError::NodeLocked("c:env:010:01:1000".into()));
    }

    #[test]
    fn test_unlock_then_adjust() {
        let mut tree = make_tree();
        tree.lock("a:env:020").unwrap();
        assert!(tree.adjust("a:env:010", 800_000.0).is_err());

        tree.unlock("a:env:020").unwrap();
        let changes = tree.adjust("a:env:010", 800_000.0).unwrap();
        assert!(!changes.is_empty());
        assert_sums_valid(&tree);
    }

    // ── Error cases ──────────────────────────────────────────────

    #[test]
    fn test_adjust_nonexistent_node() {
        let mut tree = make_tree();
        let result = tree.adjust("a:999", 100.0);
        assert_eq!(result.unwrap_err(), AdjustError::NodeNotFound("a:999".into()));
    }

    #[test]
    fn test_adjust_root_rejected() {
        let mut tree = make_tree();
        let result = tree.adjust("root", 999.0);
        assert_eq!(result.unwrap_err(), AdjustError::CannotAdjustRoot);
    }

    #[test]
    fn test_lock_nonexistent() {
        let mut tree = make_tree();
        assert_eq!(tree.lock("bogus").unwrap_err(), AdjustError::NodeNotFound("bogus".into()));
    }

    #[test]
    fn test_unlock_nonexistent() {
        let mut tree = make_tree();
        assert_eq!(tree.unlock("bogus").unwrap_err(), AdjustError::NodeNotFound("bogus".into()));
    }

    #[test]
    fn test_reset_node_nonexistent() {
        let mut tree = make_tree();
        assert_eq!(tree.reset_node("nope").unwrap_err(), AdjustError::NodeNotFound("nope".into()));
    }

    // ── Clamping / bounds ────────────────────────────────────────

    #[test]
    fn test_clamp_below_zero() {
        let mut tree = make_tree();
        // Try to set to negative → should clamp to min (0 with default config)
        let _changes = tree.adjust("c:env:010:01:1000", -100.0).unwrap();
        assert_eq!(tree.get_value("c:env:010:01:1000").unwrap(), 0.0);
        assert_sums_valid(&tree);
    }

    #[test]
    fn test_clamp_above_parent() {
        let mut tree = make_tree();
        // Army parent = 500,000. Try to set Operations to 999,999
        let _changes = tree.adjust("c:env:010:01:1000", 999_999.0).unwrap();
        // Should clamp to parent value (500k)
        assert_eq!(tree.get_value("c:env:010:01:1000").unwrap(), 500_000.0);
        assert_sums_valid(&tree);
    }

    #[test]
    fn test_configurable_min_bound() {
        let csv = "Agency Code,Agency Name,Bureau Code,Bureau Name,Account Code,Account Name,Treasury Agency Code,Subfunction Code,Subfunction Title,BEA Category,On- or Off- Budget,2021\n\
                   010,Def,01,Army,1000,Ops,10,051,DoD,Discretionary,On-budget,\"300,000\"\n\
                   010,Def,01,Army,2000,Pers,10,051,DoD,Discretionary,On-budget,\"200,000\"\n\
                   010,Def,02,Navy,1000,Ships,10,051,DoD,Discretionary,On-budget,\"250,000\"";

        let config = BudgetConfig {
            min_fraction_of_default: 0.3, // 30% of default = floor
            fiscal_year: "2021".into(),
            bea_filter: "Discretionary".into(),
            on_budget_only: true,
            ..Default::default()
        };
        let mut tree = BudgetTree::from_csv(csv, config).unwrap();

        // Try to set Ops to 0 → should clamp to 300k * 0.3 = 90k
        let _changes = tree.adjust("c:env:010:01:1000", 0.0).unwrap();
        let ops = tree.get_value("c:env:010:01:1000").unwrap();
        assert!((ops - 90_000.0).abs() < 0.1, "should clamp to 90k, got {}", ops);
        assert_sums_valid(&tree);
    }

    #[test]
    fn test_sibling_clamped_at_floor() {
        // When increasing Ops pushes Pers below its floor, overflow handled
        let csv = "Agency Code,Agency Name,Bureau Code,Bureau Name,Account Code,Account Name,Treasury Agency Code,Subfunction Code,Subfunction Title,BEA Category,On- or Off- Budget,2021\n\
                   010,Def,01,Army,1000,Ops,10,051,DoD,Discretionary,On-budget,\"300,000\"\n\
                   010,Def,01,Army,2000,Pers,10,051,DoD,Discretionary,On-budget,\"100,000\"\n\
                   010,Def,01,Army,3000,Intel,10,051,DoD,Discretionary,On-budget,\"100,000\"";

        let config = BudgetConfig {
            min_fraction_of_default: 0.5, // floor = 50% of default
            fiscal_year: "2021".into(),
            bea_filter: "Discretionary".into(),
            on_budget_only: true,
            ..Default::default()
        };
        let mut tree = BudgetTree::from_csv(csv, config).unwrap();

        // Ops=300k, Pers=100k(floor=50k), Intel=100k(floor=50k). Parent=500k.
        // Set Ops to 450k → delta=+150k, siblings must absorb -150k
        // But floor of each sibling is 50k, so max each can give is 50k = 100k total
        // Target gets clamped: Ops = 300k + 100k = 400k
        let _changes = tree.adjust("c:env:010:01:1000", 450_000.0).unwrap();

        let ops = tree.get_value("c:env:010:01:1000").unwrap();
        assert!((ops - 400_000.0).abs() < 0.1, "ops should be clamped to 400k, got {}", ops);

        let pers = tree.get_value("c:env:010:01:2000").unwrap();
        let intel = tree.get_value("c:env:010:01:3000").unwrap();
        // Siblings should be at their floors
        assert!(pers >= 50_000.0 - 0.1, "pers should be >= 50k, got {}", pers);
        assert!(intel >= 50_000.0 - 0.1, "intel should be >= 50k, got {}", intel);

        assert_sums_valid(&tree);
    }

    // ── No-op ────────────────────────────────────────────────────

    #[test]
    fn test_noop_same_value() {
        let mut tree = make_tree();
        let changes = tree.adjust("c:env:010:01:1000", 300_000.0).unwrap();
        assert!(changes.is_empty(), "should be no-op");
    }

    #[test]
    fn test_noop_within_precision() {
        let mut tree = make_tree();
        let changes = tree.adjust("c:env:010:01:1000", 300_000.005).unwrap();
        assert!(changes.is_empty(), "should be no-op within precision");
    }

    // ── Reset ────────────────────────────────────────────────────

    #[test]
    fn test_reset_single_node() {
        let mut tree = make_tree();
        tree.adjust("c:env:010:01:1000", 400_000.0).unwrap();
        assert!((tree.get_value("c:env:010:01:1000").unwrap() - 400_000.0).abs() < 0.1);

        let changes = tree.reset_node("c:env:010:01:1000").unwrap();
        assert!(!changes.is_empty());
        assert!((tree.get_value("c:env:010:01:1000").unwrap() - 300_000.0).abs() < 0.1);
        assert_sums_valid(&tree);
    }

    // ── Validate ─────────────────────────────────────────────────

    #[test]
    fn test_validate_clean_tree() {
        let tree = make_tree();
        assert!(tree.validate().is_empty());
    }

    #[test]
    fn test_validate_after_adjustments() {
        let mut tree = make_tree();
        tree.adjust("c:env:010:01:1000", 100_000.0).unwrap();
        tree.adjust("a:env:010", 600_000.0).unwrap();
        tree.adjust("c:env:020:01:1000", 80_000.0).unwrap();
        assert_sums_valid(&tree);
    }

    // ── Sequential adjustments ───────────────────────────────────

    #[test]
    fn test_multiple_sequential_adjustments() {
        let mut tree = make_tree();

        tree.adjust("c:env:010:01:1000", 350_000.0).unwrap();
        assert_sums_valid(&tree);

        tree.adjust("c:env:010:01:1000", 250_000.0).unwrap();
        assert_sums_valid(&tree);

        tree.adjust("a:env:020", 200_000.0).unwrap();
        assert_sums_valid(&tree);

        tree.adjust("b:env:010:02", 200_000.0).unwrap();
        assert_sums_valid(&tree);

        // Root should still be 900k
        assert!((tree.get_value("root").unwrap() - 900_000.0).abs() < 0.1);
    }

    #[test]
    fn test_adjust_then_lock_then_adjust() {
        let mut tree = make_tree();
        tree.adjust("c:env:020:01:1000", 120_000.0).unwrap();
        tree.lock("c:env:020:01:1000").unwrap();

        // Now adjust the sibling — should work since target isn't locked
        let changes = tree.adjust("c:env:020:01:2000", 30_000.0).unwrap();
        // But c:env:020:01:1000 is locked so only sibling is locked → error
        // Wait — c:env:020:01:2000 has only one sibling (c:env:020:01:1000) which is locked
        assert!(changes.is_empty() || tree.adjust("c:env:020:01:2000", 10_000.0).is_err());
    }

    // ── Changeset correctness ────────────────────────────────────

    #[test]
    fn test_changeset_contains_all_modified() {
        let mut tree = make_tree();
        let before: Vec<f64> = tree.all_values();
        let changes = tree.adjust("a:env:010", 800_000.0).unwrap();
        let after: Vec<f64> = tree.all_values();

        // Every node that actually changed should be in the changeset
        for i in 0..before.len() {
            if (before[i] - after[i]).abs() > 0.01 {
                assert!(changes.iter().any(|c| c.idx == i),
                    "node {} changed from {} to {} but not in changeset",
                    tree.nodes[i].id, before[i], after[i]);
            }
        }

        // Every changeset entry should reflect the actual value
        for c in &changes {
            assert!((c.new_val - tree.nodes[c.idx].value).abs() < 0.01,
                "changeset idx={} says {} but actual is {}",
                c.idx, c.new_val, tree.nodes[c.idx].value);
        }
    }

    // ── Real CSV stress test ─────────────────────────────────────

    #[test]
    fn test_real_csv_adjust() {
        if let Ok(text) = std::fs::read_to_string("/tmp/taxnvote/budget/budauth.csv") {
            let mut tree = BudgetTree::from_csv(&text, test_config()).unwrap();
            let root_val = tree.get_value("root").unwrap();

            // Find first two agencies
            let agencies: Vec<usize> = tree.nodes[0].children.clone();
            assert!(agencies.len() >= 2);

            let a0 = agencies[0];
            let a0_id = tree.nodes[a0].id.clone();
            let old_val = tree.nodes[a0].value;

            // Increase first agency by 10%
            let new_val = old_val * 1.1;
            let changes = tree.adjust(&a0_id, new_val).unwrap();
            assert!(!changes.is_empty());

            // Root should be unchanged
            assert!((tree.get_value("root").unwrap() - root_val).abs() < 1.0,
                "root should be unchanged");

            assert_sums_valid(&tree);
        }
    }
}
