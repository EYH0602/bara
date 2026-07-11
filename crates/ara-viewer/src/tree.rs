//! DOM tree-list display mode — pure `tree_model` builder + `TreeView` component.
//!
//! The file is split into two halves, mirroring `detail.rs`:
//! 1. **Pure model** (`TreeRow`/`TreeNode`/`TreeModel`, `tree_model`): no
//!    web-sys deps, fully native-testable. Turns a `&Manifest` into a renderable
//!    forest exactly as the published `research-visualizer` `renderMap` does.
//! 2. **Leptos component** (`TreeView`): renders that model as scoped DOM inside
//!    `.tree-map`, reproducing the reference markup 1:1.
//!
//! The tree model reproduces the reference scaffold
//! (`ARA-Labs/ARA-Demo` → `nanogpt_ara/trajectory.html`): rows label off
//! `title ?? body ?? "(untitled)"`, roots split by the node's own `isolated`
//! flag (`normalRoots` vs `isoRoots`), children nest via `Child` links, and each
//! row carries its outgoing `DependsOn` targets as the `⇠` dep marker.

use std::collections::{HashMap, HashSet};

use ara_core::{LinkKind, Manifest, NodeId};

use crate::kind::kind_meta;

// ── Pure model ──────────────────────────────────────────────────────────────

/// One rendered row of the tree-list, matching the reference `nodeRow`.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeRow {
    pub id: NodeId,
    /// `title ?? body ?? "(untitled)"` — the reference `nodeRow` fallback chain.
    pub label: String,
    /// Kind glyph, from `kind_meta` (the single glyph source of truth).
    pub glyph: char,
    /// Kind wire tag (`question`/`experiment`/…/`other`) for the `.glyph {type}`
    /// class — equals `kind_meta(&kind).css_class`.
    pub css_class: &'static str,
    /// True for `dead_end` nodes → the row gets the `.node.dead` treatment.
    pub is_dead_end: bool,
    /// Outgoing `DependsOn` edge targets, in source order — rendered as the
    /// single `⇠ {ids}` dep marker.
    pub dep_targets: Vec<NodeId>,
}

/// A node in the tree forest: its row plus recursively-built children.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    pub row: TreeRow,
    pub children: Vec<TreeNode>,
}

/// The full renderable forest: normal roots plus isolated-subtree roots.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreeModel {
    /// Non-isolated roots, rendered at the top level.
    pub roots: Vec<TreeNode>,
    /// Isolated-subtree roots, rendered inside the `.isobox`.
    pub isolated: Vec<TreeNode>,
}

/// Build the renderable [`TreeModel`] from a `&Manifest`.
///
/// Deterministic and source-order preserving:
/// - **Child adjacency** from `LinkKind::Child` links: `from → [to…]` in link
///   source order.
/// - **Roots** = nodes (in `manifest.nodes` order — already pre-order DFS) with
///   no incoming `Child` edge.
/// - Each root is expanded recursively through the child map; a **visited set
///   guards against Child cycles** so a malformed manifest cannot infinite-loop.
/// - **`dep_targets`** per row = the `to` ids of the node's outgoing
///   `DependsOn` links, in source order.
/// - **Isolated partition**: roots whose node carries `isolated: true` go into
///   [`TreeModel::isolated`]; the rest into [`TreeModel::roots`]. Isolation is a
///   property of the root only; children inherit their placement.
/// - Empty manifest → empty [`TreeModel`].
pub fn tree_model(manifest: &Manifest) -> TreeModel {
    // Child adjacency (source order) + the set of nodes that are some node's
    // Child (→ have an incoming Child edge, so are not roots).
    let mut children_of: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();
    let mut has_parent: HashSet<&NodeId> = HashSet::new();
    for link in &manifest.links {
        if link.kind == LinkKind::Child {
            children_of.entry(&link.from).or_default().push(&link.to);
            has_parent.insert(&link.to);
        }
    }

    // Outgoing DependsOn targets per node (source order).
    let mut deps_of: HashMap<&NodeId, Vec<NodeId>> = HashMap::new();
    for link in &manifest.links {
        if link.kind == LinkKind::DependsOn {
            deps_of.entry(&link.from).or_default().push(link.to.clone());
        }
    }

    // Node lookup by id, for isolated-flag + row building.
    let by_id: HashMap<&NodeId, &ara_core::Node> =
        manifest.nodes.iter().map(|n| (&n.id, n)).collect();

    let mut roots = Vec::new();
    let mut isolated = Vec::new();

    for node in &manifest.nodes {
        if has_parent.contains(&node.id) {
            continue; // not a root
        }
        let mut visited: HashSet<NodeId> = HashSet::new();
        visited.insert(node.id.clone());
        let tree_node = build_node(&node.id, &by_id, &children_of, &deps_of, &mut visited);
        if node.isolated {
            isolated.push(tree_node);
        } else {
            roots.push(tree_node);
        }
    }

    TreeModel { roots, isolated }
}

/// Recursively build a [`TreeNode`] for `id`, guarding against Child cycles via
/// `visited` (a node already on the current path is not re-expanded). The caller
/// inserts `id` into `visited` before the first call.
fn build_node(
    id: &NodeId,
    by_id: &HashMap<&NodeId, &ara_core::Node>,
    children_of: &HashMap<&NodeId, Vec<&NodeId>>,
    deps_of: &HashMap<&NodeId, Vec<NodeId>>,
    visited: &mut HashSet<NodeId>,
) -> TreeNode {
    let row = build_row(id, by_id, deps_of);

    let mut children = Vec::new();
    if let Some(kids) = children_of.get(id) {
        for &kid in kids {
            // `insert` returns false if `kid` is already on the current path
            // (a Child cycle) — skip it so recursion terminates.
            if visited.insert(kid.clone()) {
                children.push(build_node(kid, by_id, children_of, deps_of, visited));
            }
        }
    }

    TreeNode { row, children }
}

/// Build a single [`TreeRow`] for `id`, resolving label + glyph + deps.
fn build_row(
    id: &NodeId,
    by_id: &HashMap<&NodeId, &ara_core::Node>,
    deps_of: &HashMap<&NodeId, Vec<NodeId>>,
) -> TreeRow {
    let node = by_id.get(id).copied();
    let meta = node.map(|n| kind_meta(&n.kind));

    // Reference fallback chain: title ?? body ?? "(untitled)".
    let label = node
        .and_then(|n| n.label.clone().or_else(|| n.description.clone()))
        .unwrap_or_else(|| "(untitled)".to_string());

    let (glyph, css_class, is_dead_end) = match (node, &meta) {
        (Some(n), Some(m)) => (
            m.glyph,
            m.css_class,
            matches!(n.kind, ara_core::NodeKind::DeadEnd),
        ),
        // Unknown id (dangling child link) — render a neutral "other" row.
        _ => ('•', "other", false),
    };

    let dep_targets = deps_of.get(id).cloned().unwrap_or_default();

    TreeRow {
        id: id.clone(),
        label,
        glyph,
        css_class,
        is_dead_end,
        dep_targets,
    }
}

// ── Tests (native — no browser required) ──────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::parse_manifest;
    use ara_core::{Link, LinkKind, Manifest, Node, NodeFields, NodeId, NodeKind};

    fn bare_manifest() -> Manifest {
        Manifest {
            nodes: vec![],
            links: vec![],
            bindings: vec![],
            claims: vec![],
            bounds: None,
        }
    }

    fn node(id: &str, kind: NodeKind, label: Option<&str>) -> Node {
        Node {
            id: NodeId::new(id),
            kind,
            label: label.map(|s| s.to_string()),
            support_level: None,
            source_refs: vec![],
            description: None,
            fields: NodeFields::Question,
            evidence_notes: vec![],
            isolated: false,
            pos: None,
        }
    }

    fn child(from: &str, to: &str) -> Link {
        Link {
            from: NodeId::new(from),
            to: NodeId::new(to),
            kind: LinkKind::Child,
        }
    }

    fn depends(from: &str, to: &str) -> Link {
        Link {
            from: NodeId::new(from),
            to: NodeId::new(to),
            kind: LinkKind::DependsOn,
        }
    }

    // ── empty ─────────────────────────────────────────────────────────────────

    #[test]
    fn empty_manifest_yields_empty_model() {
        let m = tree_model(&bare_manifest());
        assert!(m.roots.is_empty());
        assert!(m.isolated.is_empty());
    }

    // ── single-tree nesting + depth ─────────────────────────────────────────────

    #[test]
    fn single_tree_nesting_and_depth() {
        let mut m = bare_manifest();
        m.nodes = vec![
            node("N01", NodeKind::Question, Some("root")),
            node("N02", NodeKind::Experiment, Some("child")),
            node("N03", NodeKind::Insight, Some("grandchild")),
        ];
        m.links = vec![child("N01", "N02"), child("N02", "N03")];

        let model = tree_model(&m);
        assert_eq!(model.roots.len(), 1, "one root (N01)");
        assert!(model.isolated.is_empty());
        let root = &model.roots[0];
        assert_eq!(root.row.id, NodeId::new("N01"));
        assert_eq!(root.children.len(), 1);
        let mid = &root.children[0];
        assert_eq!(mid.row.id, NodeId::new("N02"));
        assert_eq!(mid.children.len(), 1);
        assert_eq!(mid.children[0].row.id, NodeId::new("N03"));
        assert!(mid.children[0].children.is_empty());
    }

    #[test]
    fn children_preserve_link_source_order() {
        let mut m = bare_manifest();
        m.nodes = vec![
            node("N01", NodeKind::Question, Some("root")),
            node("N02", NodeKind::Experiment, Some("b")),
            node("N03", NodeKind::Experiment, Some("a")),
        ];
        // Deliberately add N03 before N02 in link order.
        m.links = vec![child("N01", "N03"), child("N01", "N02")];

        let model = tree_model(&m);
        let kids: Vec<&str> = model.roots[0]
            .children
            .iter()
            .map(|c| c.row.id.as_str())
            .collect();
        assert_eq!(kids, ["N03", "N02"], "children follow Child-link source order");
    }

    // ── isolated partition ──────────────────────────────────────────────────────

    #[test]
    fn isolated_root_lands_in_isolated_with_its_subtree() {
        let mut m = bare_manifest();
        let mut iso_root = node("N10", NodeKind::Question, Some("iso"));
        iso_root.isolated = true;
        m.nodes = vec![
            node("N01", NodeKind::Question, Some("normal root")),
            node("N02", NodeKind::Experiment, Some("normal child")),
            iso_root,
            node("N11", NodeKind::Experiment, Some("iso child")),
        ];
        m.links = vec![child("N01", "N02"), child("N10", "N11")];

        let model = tree_model(&m);
        assert_eq!(model.roots.len(), 1, "one normal root");
        assert_eq!(model.roots[0].row.id, NodeId::new("N01"));
        assert_eq!(model.isolated.len(), 1, "one isolated root");
        let iso = &model.isolated[0];
        assert_eq!(iso.row.id, NodeId::new("N10"));
        assert_eq!(iso.children.len(), 1, "isolated subtree carries its child");
        assert_eq!(iso.children[0].row.id, NodeId::new("N11"));
    }

    #[test]
    fn false_isolated_root_lands_in_roots() {
        let mut m = bare_manifest();
        m.nodes = vec![node("N01", NodeKind::Question, Some("root"))];
        let model = tree_model(&m);
        assert_eq!(model.roots.len(), 1);
        assert!(model.isolated.is_empty());
    }

    // ── dep_targets from DependsOn only ─────────────────────────────────────────

    #[test]
    fn dep_targets_populated_from_depends_on_not_child() {
        let mut m = bare_manifest();
        m.nodes = vec![
            node("N01", NodeKind::Question, Some("root")),
            node("N02", NodeKind::Experiment, Some("child")),
            node("N03", NodeKind::Insight, Some("dep")),
        ];
        m.links = vec![child("N01", "N02"), depends("N02", "N03")];

        let model = tree_model(&m);
        let root = &model.roots[0];
        // Root has a Child link but no DependsOn → empty dep_targets.
        assert!(root.row.dep_targets.is_empty());
        // Child N02 has a DependsOn to N03.
        assert_eq!(root.children[0].row.dep_targets, vec![NodeId::new("N03")]);
    }

    #[test]
    fn dep_targets_preserve_source_order() {
        let mut m = bare_manifest();
        m.nodes = vec![
            node("N01", NodeKind::Question, Some("root")),
            node("N02", NodeKind::Insight, Some("d2")),
            node("N03", NodeKind::Insight, Some("d3")),
        ];
        m.links = vec![depends("N01", "N03"), depends("N01", "N02")];

        let model = tree_model(&m);
        assert_eq!(
            model.roots[0].row.dep_targets,
            vec![NodeId::new("N03"), NodeId::new("N02")]
        );
    }

    // ── dead-end flag ───────────────────────────────────────────────────────────

    #[test]
    fn dead_end_row_flagged() {
        let mut m = bare_manifest();
        m.nodes = vec![
            node("N01", NodeKind::Question, Some("root")),
            node("N02", NodeKind::DeadEnd, Some("dead")),
        ];
        m.links = vec![child("N01", "N02")];

        let model = tree_model(&m);
        assert!(!model.roots[0].row.is_dead_end);
        assert!(model.roots[0].children[0].row.is_dead_end);
        assert_eq!(model.roots[0].children[0].row.glyph, '✗');
    }

    // ── label fallback chain ────────────────────────────────────────────────────

    #[test]
    fn label_fallback_title_then_body_then_untitled() {
        let mut m = bare_manifest();
        // N01: has a title (label).
        let n01 = node("N01", NodeKind::Question, Some("A Title"));
        // N02: no title, but a body (description).
        let mut n02 = node("N02", NodeKind::Experiment, None);
        n02.description = Some("Body prose".to_string());
        // N03: neither → "(untitled)".
        let n03 = node("N03", NodeKind::Insight, None);
        m.nodes = vec![n01, n02, n03];
        m.links = vec![child("N01", "N02"), child("N02", "N03")];

        let model = tree_model(&m);
        assert_eq!(model.roots[0].row.label, "A Title");
        assert_eq!(model.roots[0].children[0].row.label, "Body prose");
        assert_eq!(
            model.roots[0].children[0].children[0].row.label,
            "(untitled)"
        );
    }

    // ── cycle guard ─────────────────────────────────────────────────────────────

    #[test]
    fn child_cycle_terminates() {
        // A malformed hand-built manifest with a Child cycle N01→N02→N01.
        // parse.rs rejects this at load, but tree_model must still terminate.
        let mut m = bare_manifest();
        m.nodes = vec![
            node("N01", NodeKind::Question, Some("a")),
            node("N02", NodeKind::Experiment, Some("b")),
        ];
        m.links = vec![child("N01", "N02"), child("N02", "N01")];

        // Both nodes have an incoming Child edge → no root. Force a root by
        // adding a third node that points into the cycle.
        m.nodes.insert(0, node("N00", NodeKind::Question, Some("start")));
        m.links.insert(0, child("N00", "N01"));

        // Must not infinite-loop.
        let model = tree_model(&m);
        assert_eq!(model.roots.len(), 1);
        assert_eq!(model.roots[0].row.id, NodeId::new("N00"));
        // N00 → N01 → N02 → (N01 already visited, stop).
        let n01 = &model.roots[0].children[0];
        assert_eq!(n01.row.id, NodeId::new("N01"));
        let n02 = &n01.children[0];
        assert_eq!(n02.row.id, NodeId::new("N02"));
        assert!(n02.children.is_empty(), "cycle back-edge to N01 is pruned");
    }

    // ── demo round-trip ─────────────────────────────────────────────────────────

    #[test]
    fn demo_manifest_round_trip() {
        let json = include_str!("../public/manifest.json");
        let manifest = parse_manifest(json).expect("checked-in manifest must parse");
        let model = tree_model(&manifest);

        // The ResNet demo has a single root N01 and no isolated subtrees.
        assert_eq!(model.roots.len(), 1, "demo has one root");
        assert_eq!(model.roots[0].row.id, NodeId::new("N01"));
        assert!(model.isolated.is_empty(), "demo has no isolated nodes");

        // The full forest must cover all 15 nodes.
        fn count(n: &TreeNode) -> usize {
            1 + n.children.iter().map(count).sum::<usize>()
        }
        let total: usize = model.roots.iter().map(count).sum();
        assert_eq!(total, 15, "all 15 demo nodes appear in the tree");
    }
}
