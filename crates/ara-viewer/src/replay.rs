//! Replay stepper — pure step/counter helpers + the `ReplayBar` component.
//!
//! The replay works in **both** display modes: it steps the shared `selected`
//! signal through node order, matching the published `research-visualizer`
//! `step` / `play` / `stop`. The pure helpers here are native-testable; the
//! `ReplayBar` component (and its 1300 ms interval) lands with the wiring.

use ara_core::{Manifest, NodeId};

// ── Pure helpers ──────────────────────────────────────────────────────────────

/// Direction of a single replay step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Next,
    Prev,
}

/// Node traversal order for replay: `manifest.nodes` order.
///
/// The manifest contract guarantees `nodes` is pre-order DFS, which equals the
/// reference's "`DATA.order` if present, else DFS-from-roots" for our manifests.
pub fn node_order(manifest: &Manifest) -> Vec<NodeId> {
    manifest.nodes.iter().map(|n| n.id.clone()).collect()
}

/// Advance / retreat the selection by one step, reproducing the reference
/// `step(delta)` semantics exactly:
///
/// `i = clamp(0, N-1, indexOf(current) + delta)`, with `indexOf(None) = -1` and
/// an unknown id treated the same as `None` (`-1`).
///
/// Consequences (all matching the reference):
/// - `Next` from `None` → `order[0]`.
/// - `Prev` from `None` → `order[0]` too (`-1 + -1 = -2` clamps to `0`) — **not**
///   the last node.
/// - Clamps at both ends (no wrap).
///
/// Returns `None` only when `order` is empty.
pub fn step(order: &[NodeId], current: Option<&NodeId>, dir: Step) -> Option<NodeId> {
    if order.is_empty() {
        return None;
    }
    let n = order.len() as isize;
    // indexOf(current); None / unknown → -1.
    let idx = current
        .and_then(|c| order.iter().position(|id| id == c))
        .map(|p| p as isize)
        .unwrap_or(-1);
    let delta = match dir {
        Step::Next => 1,
        Step::Prev => -1,
    };
    let clamped = (idx + delta).clamp(0, n - 1);
    Some(order[clamped as usize].clone())
}

/// Replay counter `(i, N)`: `i` is the **1-based** position of `current` in
/// `order`, or `0` when there is no selection (or an unknown id). `N` is the
/// total node count.
pub fn counter(order: &[NodeId], current: Option<&NodeId>) -> (usize, usize) {
    let n = order.len();
    let i = current
        .and_then(|c| order.iter().position(|id| id == c))
        .map(|p| p + 1)
        .unwrap_or(0);
    (i, n)
}

/// The shared `#rstat` readout string.
///
/// Two forms, matching the reference's single shared span:
/// - **replay form** (a node is selected): `"step {i} / {N}"`.
/// - **filter form** (nothing selected): `"{shown} / {N} steps"`, where `shown`
///   is the number of nodes passing the current filter.
///
/// When a node is selected the replay form wins (the reference's `rstat` write
/// on select overrides the filter write).
pub fn rstat_text(order: &[NodeId], current: Option<&NodeId>, shown: usize) -> String {
    let (i, n) = counter(order, current);
    if i > 0 {
        format!("step {i} / {n}")
    } else {
        format!("{shown} / {n} steps")
    }
}

// ── Tests (native) ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::parse_manifest;

    fn ids(strs: &[&str]) -> Vec<NodeId> {
        strs.iter().map(|s| NodeId::new(*s)).collect()
    }

    // ── node_order ──────────────────────────────────────────────────────────────

    #[test]
    fn node_order_equals_manifest_nodes_order() {
        let json = include_str!("../public/manifest.json");
        let manifest = parse_manifest(json).unwrap();
        let order = node_order(&manifest);
        let expected: Vec<NodeId> = manifest.nodes.iter().map(|n| n.id.clone()).collect();
        assert_eq!(order, expected);
        assert_eq!(order.first(), Some(&NodeId::new("N01")));
        assert_eq!(order.len(), 15);
    }

    // ── step: clamp at both ends ────────────────────────────────────────────────

    #[test]
    fn step_next_advances() {
        let order = ids(&["N01", "N02", "N03"]);
        assert_eq!(
            step(&order, Some(&NodeId::new("N01")), Step::Next),
            Some(NodeId::new("N02"))
        );
    }

    #[test]
    fn step_prev_retreats() {
        let order = ids(&["N01", "N02", "N03"]);
        assert_eq!(
            step(&order, Some(&NodeId::new("N02")), Step::Prev),
            Some(NodeId::new("N01"))
        );
    }

    #[test]
    fn step_next_clamps_at_last() {
        let order = ids(&["N01", "N02", "N03"]);
        assert_eq!(
            step(&order, Some(&NodeId::new("N03")), Step::Next),
            Some(NodeId::new("N03")),
            "Next at last node stays (no wrap)"
        );
    }

    #[test]
    fn step_prev_clamps_at_first() {
        let order = ids(&["N01", "N02", "N03"]);
        assert_eq!(
            step(&order, Some(&NodeId::new("N01")), Step::Prev),
            Some(NodeId::new("N01")),
            "Prev at first node stays (no wrap)"
        );
    }

    // ── step: from None (reference quirk) ────────────────────────────────────────

    #[test]
    fn step_next_from_none_selects_first() {
        let order = ids(&["N01", "N02", "N03"]);
        assert_eq!(step(&order, None, Step::Next), Some(NodeId::new("N01")));
    }

    #[test]
    fn step_prev_from_none_selects_first_not_last() {
        // Reference quirk: indexOf(None) = -1, -1 + -1 = -2 clamps to 0 → first.
        let order = ids(&["N01", "N02", "N03"]);
        assert_eq!(
            step(&order, None, Step::Prev),
            Some(NodeId::new("N01")),
            "Prev from no selection selects the FIRST node (reference clamp)"
        );
    }

    #[test]
    fn step_unknown_id_treated_as_none() {
        let order = ids(&["N01", "N02", "N03"]);
        assert_eq!(
            step(&order, Some(&NodeId::new("N99")), Step::Next),
            Some(NodeId::new("N01")),
            "unknown id → indexOf -1, same as None"
        );
    }

    #[test]
    fn step_empty_order_yields_none() {
        assert_eq!(step(&[], None, Step::Next), None);
        assert_eq!(step(&[], Some(&NodeId::new("N01")), Step::Prev), None);
    }

    // ── counter ─────────────────────────────────────────────────────────────────

    #[test]
    fn counter_is_one_based() {
        let order = ids(&["N01", "N02", "N03"]);
        assert_eq!(counter(&order, Some(&NodeId::new("N01"))), (1, 3));
        assert_eq!(counter(&order, Some(&NodeId::new("N03"))), (3, 3));
    }

    #[test]
    fn counter_zero_when_unselected() {
        let order = ids(&["N01", "N02", "N03"]);
        assert_eq!(counter(&order, None), (0, 3));
        assert_eq!(counter(&order, Some(&NodeId::new("N99"))), (0, 3), "unknown → 0");
    }

    // ── rstat string forms ───────────────────────────────────────────────────────

    #[test]
    fn rstat_replay_form_when_selected() {
        let order = ids(&["N01", "N02", "N03"]);
        assert_eq!(
            rstat_text(&order, Some(&NodeId::new("N02")), 3),
            "step 2 / 3"
        );
    }

    #[test]
    fn rstat_filter_form_when_unselected() {
        let order = ids(&["N01", "N02", "N03"]);
        // 2 of 3 nodes shown by the filter, nothing selected.
        assert_eq!(rstat_text(&order, None, 2), "2 / 3 steps");
    }
}
