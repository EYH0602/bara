//! Pure scene model, `GraphRenderer` trait, and `SvgRenderer`.
//!
//! The scene layer is the bridge between a [`Manifest`] (logical graph) and a
//! concrete renderer.  Everything in this file is free of `web-sys` and
//! browser-only imports so it compiles and is fully tested on native targets.
//!
//! Edge paths use **center-to-center straight lines**: each path goes from the
//! center `(pos.x, pos.y)` of the source node to the center of the target node.
//! Pan/zoom is applied at the SVG `viewBox` level, not here; scene geometry is
//! view-independent.

use std::collections::HashMap;

use ara_core::{LinkKind, Manifest, NodeId, NodeKind, Rect};
use leptos::prelude::*;

use crate::kind::kind_meta;
use crate::state::safe_viewbox;

// ── Layout parameters ─────────────────────────────────────────────────────────

/// Fixed rendering parameters matching `ara-core`'s `LayoutOptions` defaults.
///
/// Node box size is fixed at 180×60 px — the same values used during layout so
/// the rendered boxes align with the precomputed positions.  Pan/zoom is applied
/// at the SVG `viewBox` level and is not encoded here.
pub struct LayoutView {
    pub node_width: f64,
    pub node_height: f64,
}

impl Default for LayoutView {
    fn default() -> Self {
        Self {
            node_width: 180.0,
            node_height: 60.0,
        }
    }
}

// ── Scene types ───────────────────────────────────────────────────────────────

/// A single node in the scene with all display-relevant fields precomputed.
pub struct SceneNode {
    // `id` and `kind` are consumed by tests and will be used by the interaction
    // layer in Step 3b (selection, focus, hit-testing).  Suppress the dead-code
    // lint that fires because the current binary path only uses the derived fields.
    #[allow(dead_code)]
    pub id: NodeId,
    /// Top-left origin bounding box: `x = pos.x - w/2`, `y = pos.y - h/2`.
    pub rect: Rect,
    #[allow(dead_code)]
    pub kind: NodeKind,
    pub glyph: char,
    pub css_class: &'static str,
    pub label: String,
    pub badge: String,
    pub is_dead_end: bool,
}

/// A directed edge in the scene with a precomputed SVG path string.
pub struct SceneEdge {
    // `from`/`to` are consumed by tests and will be used by the interaction
    // layer in Step 3b.  See note on SceneNode above.
    #[allow(dead_code)]
    pub from: NodeId,
    #[allow(dead_code)]
    pub to: NodeId,
    /// SVG path `d` attribute: straight center-to-center line.
    pub path: String,
    pub link_kind: LinkKind,
}

/// The complete scene derived from a [`Manifest`] and [`LayoutView`].
pub struct GraphScene {
    pub nodes: Vec<SceneNode>,
    pub edges: Vec<SceneEdge>,
    /// Bounds for the SVG `viewBox`.  Falls back to `(0,0,100,100)` when the
    /// manifest carries no valid bounds (reuses [`safe_viewbox`] semantics).
    pub bounds: Rect,
}

// ── GraphRenderer trait ───────────────────────────────────────────────────────

/// Seam between the logical graph and a concrete renderer.
///
/// `scene` is a pure function; it never panics and has no side effects.
/// Concrete renderers implement the rendering step (SVG now, canvas if needed).
pub trait GraphRenderer {
    fn scene(&self, manifest: &Manifest, view: &LayoutView) -> GraphScene;
}

// ── SvgRenderer ───────────────────────────────────────────────────────────────

/// Renders a [`GraphScene`] as static, skinned SVG inside a Leptos `view!`.
pub struct SvgRenderer;

impl GraphRenderer for SvgRenderer {
    fn scene(&self, manifest: &Manifest, view: &LayoutView) -> GraphScene {
        // Build NodeId → center-Point map from nodes that have positions.
        let pos_map: HashMap<&NodeId, (f64, f64)> = manifest
            .nodes
            .iter()
            .filter_map(|n| n.pos.map(|p| (&n.id, (p.x, p.y))))
            .collect();

        // Build scene nodes (skip nodes without positions).
        let nodes = manifest
            .nodes
            .iter()
            .filter_map(|n| {
                let (cx, cy) = *pos_map.get(&n.id)?;
                let meta = kind_meta(&n.kind);
                let label = n.label.clone().unwrap_or_else(|| n.id.as_str().to_string());
                Some(SceneNode {
                    id: n.id.clone(),
                    rect: Rect {
                        x: cx - view.node_width / 2.0,
                        y: cy - view.node_height / 2.0,
                        width: view.node_width,
                        height: view.node_height,
                    },
                    is_dead_end: matches!(n.kind, NodeKind::DeadEnd),
                    glyph: meta.glyph,
                    css_class: meta.css_class,
                    badge: meta.badge,
                    kind: n.kind.clone(),
                    label,
                })
            })
            .collect();

        // Build scene edges (skip any edge whose endpoint has no position).
        let edges = manifest
            .links
            .iter()
            .filter_map(|link| {
                let (x1, y1) = *pos_map.get(&link.from)?;
                let (x2, y2) = *pos_map.get(&link.to)?;
                let path = format!("M {x1} {y1} L {x2} {y2}");
                Some(SceneEdge {
                    from: link.from.clone(),
                    to: link.to.clone(),
                    path,
                    link_kind: link.kind,
                })
            })
            .collect();

        // Derive bounds from manifest; fall back to (0,0,100,100) via safe_viewbox.
        let (bx, by, bw, bh) = safe_viewbox(manifest.bounds.as_ref());
        let bounds = Rect {
            x: bx,
            y: by,
            width: bw,
            height: bh,
        };

        GraphScene {
            nodes,
            edges,
            bounds,
        }
    }
}

// ── SVG render function ───────────────────────────────────────────────────────

/// Render a [`GraphScene`] as Leptos SVG content.
///
/// Produces the *inner* content to be placed inside `<svg class="graph-svg">`.
/// Edges are rendered first (under nodes).  This is a static render — no event
/// handlers, no tabindex, no selection classes.
pub fn render_scene(scene: &GraphScene) -> impl IntoView {
    // Pre-collect edges into owned data for the `view!` macro.
    let edges: Vec<(String, &'static str)> = scene
        .edges
        .iter()
        .map(|e| {
            let cls = match e.link_kind {
                LinkKind::Child => "edge edge-child",
                LinkKind::DependsOn => "edge edge-depends",
            };
            (e.path.clone(), cls)
        })
        .collect();

    // Pre-collect node data for the `view!` macro.
    #[allow(clippy::type_complexity)]
    let nodes: Vec<(
        f64,
        f64,
        f64,
        f64,
        String,
        char,
        &'static str,
        String,
        String,
        bool,
    )> = scene
        .nodes
        .iter()
        .map(|n| {
            (
                n.rect.x,
                n.rect.y,
                n.rect.width,
                n.rect.height,
                n.label.clone(),
                n.glyph,
                n.css_class,
                // class string for the <g> element
                format!("node {}", n.css_class),
                n.badge.clone(),
                n.is_dead_end,
            )
        })
        .collect();

    view! {
        <g class="edges">
            {edges
                .into_iter()
                .map(|(path, cls)| {
                    view! { <path d=path class=cls /> }
                })
                .collect_view()}
        </g>
        <g class="nodes">
            {nodes
                .into_iter()
                .map(|(rx, ry, rw, rh, label, glyph, _css, group_class, badge, is_dead_end)| {
                    // Chip dimensions: 20×20, placed at top-left corner of the node box.
                    let chip_x = rx + 4.0;
                    let chip_y = ry + 4.0;
                    let chip_size = 20.0;
                    // Label text: centered horizontally, positioned after the chip.
                    let label_x = rx + chip_size + 10.0;
                    let label_y = ry + 26.0;
                    // Badge: bottom-right corner.
                    let badge_x = rx + rw - 4.0;
                    let badge_y = ry + rh - 5.0;
                    // Clip path id for label truncation.
                    let chip_fill = if is_dead_end {
                        "var(--warn)"
                    } else {
                        "var(--glyph-bg)"
                    };
                    let chip_ink = if is_dead_end { "#fff" } else { "var(--glyph-ink)" };
                    view! {
                        <g class=group_class>
                            // Node background rect
                            <rect
                                x=rx
                                y=ry
                                width=rw
                                height=rh
                                rx="6"
                                ry="6"
                                fill="var(--panel)"
                                stroke="var(--line)"
                                stroke-width="1"
                            />
                            // Glyph chip background
                            <rect
                                x=chip_x
                                y=chip_y
                                width=chip_size
                                height=chip_size
                                rx="4"
                                ry="4"
                                fill=chip_fill
                            />
                            // Glyph character
                            <text
                                x=chip_x + chip_size / 2.0
                                y=chip_y + chip_size / 2.0 + 5.0
                                text-anchor="middle"
                                fill=chip_ink
                                font-size="11"
                                font-weight="700"
                                font-family="ui-monospace, monospace"
                            >
                                {glyph.to_string()}
                            </text>
                            // Label with native tooltip via <title>
                            <text
                                x=label_x
                                y=label_y
                                fill="var(--ink)"
                                font-size="11"
                                font-family="ui-sans-serif, system-ui, sans-serif"
                                clip-path=format!("inset(0 0 0 0)")
                            >
                                {label.clone()}
                                // Truncation is handled visually by the clip below.
                                <title>{label}</title>
                            </text>
                            // Kind badge (bottom-right)
                            <text
                                x=badge_x
                                y=badge_y
                                text-anchor="end"
                                fill="var(--muted)"
                                font-size="9"
                                font-family="ui-sans-serif, system-ui, sans-serif"
                            >
                                {badge}
                            </text>
                        </g>
                    }
                })
                .collect_view()}
        </g>
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::parse_manifest;
    use ara_core::{Link, Manifest, Node, NodeFields, NodeKind};

    fn checked_in_manifest() -> Manifest {
        let json = include_str!("../public/manifest.json");
        parse_manifest(json).expect("checked-in manifest.json must parse")
    }

    // ── scene compute from the checked-in manifest ───────────────────────────

    #[test]
    fn scene_node_count_matches_nodes_with_pos() {
        let manifest = checked_in_manifest();
        let renderer = SvgRenderer;
        let scene = renderer.scene(&manifest, &LayoutView::default());

        // All 15 nodes in the demo have positions, so we expect 15 SceneNodes.
        let expected = manifest.nodes.iter().filter(|n| n.pos.is_some()).count();
        assert_eq!(
            scene.nodes.len(),
            expected,
            "scene node count must equal nodes-with-pos count"
        );
    }

    #[test]
    fn scene_node_rects_are_centered_on_pos() {
        let manifest = checked_in_manifest();
        let view = LayoutView::default();
        let renderer = SvgRenderer;
        let scene = renderer.scene(&manifest, &view);

        for (scene_node, manifest_node) in scene
            .nodes
            .iter()
            .zip(manifest.nodes.iter().filter(|n| n.pos.is_some()))
        {
            let pos = manifest_node.pos.unwrap();
            let expected_x = pos.x - view.node_width / 2.0;
            let expected_y = pos.y - view.node_height / 2.0;
            assert!(
                (scene_node.rect.x - expected_x).abs() < 1e-9,
                "rect.x mismatch for {}: {} vs {}",
                scene_node.id,
                scene_node.rect.x,
                expected_x
            );
            assert!(
                (scene_node.rect.y - expected_y).abs() < 1e-9,
                "rect.y mismatch for {}: {} vs {}",
                scene_node.id,
                scene_node.rect.y,
                expected_y
            );
            assert!((scene_node.rect.width - view.node_width).abs() < 1e-9);
            assert!((scene_node.rect.height - view.node_height).abs() < 1e-9);
        }
    }

    #[test]
    fn scene_node_glyph_and_css_match_kind_meta() {
        let manifest = checked_in_manifest();
        let renderer = SvgRenderer;
        let scene = renderer.scene(&manifest, &LayoutView::default());

        for sn in &scene.nodes {
            let meta = kind_meta(&sn.kind);
            assert_eq!(sn.glyph, meta.glyph, "glyph mismatch for {}", sn.id);
            assert_eq!(
                sn.css_class, meta.css_class,
                "css_class mismatch for {}",
                sn.id
            );
        }
    }

    // ── edge derive ──────────────────────────────────────────────────────────

    #[test]
    fn child_link_produces_child_edge() {
        let manifest = checked_in_manifest();
        let renderer = SvgRenderer;
        let scene = renderer.scene(&manifest, &LayoutView::default());

        let child_edges: Vec<_> = scene
            .edges
            .iter()
            .filter(|e| e.link_kind == LinkKind::Child)
            .collect();
        assert!(
            !child_edges.is_empty(),
            "demo manifest must produce at least one Child edge"
        );
        // Each child edge path must start with "M" and contain "L".
        for e in &child_edges {
            assert!(
                e.path.starts_with('M'),
                "edge path must start with M: {}",
                e.path
            );
            assert!(e.path.contains('L'), "edge path must contain L: {}", e.path);
        }
    }

    #[test]
    fn depends_on_link_produces_depends_on_edge() {
        let manifest = checked_in_manifest();
        let renderer = SvgRenderer;
        let scene = renderer.scene(&manifest, &LayoutView::default());

        let depends_edges: Vec<_> = scene
            .edges
            .iter()
            .filter(|e| e.link_kind == LinkKind::DependsOn)
            .collect();
        assert!(
            !depends_edges.is_empty(),
            "demo manifest must produce at least one DependsOn edge"
        );
    }

    #[test]
    fn child_edge_path_references_endpoint_coords() {
        let manifest = checked_in_manifest();
        let view = LayoutView::default();
        let renderer = SvgRenderer;
        let scene = renderer.scene(&manifest, &view);

        // Find N01→N02 (first child link in the demo).
        let edge = scene
            .edges
            .iter()
            .find(|e| e.from.as_str() == "N01" && e.to.as_str() == "N02")
            .expect("N01→N02 Child edge must exist");

        // N01 pos = (560, 30), N02 pos = (90, 140) per manifest.json.
        assert!(
            edge.path.contains("560"),
            "path must reference N01 x=560: {}",
            edge.path
        );
        assert!(
            edge.path.contains("140"),
            "path must reference N02 y=140: {}",
            edge.path
        );
    }

    // ── pos:None skip — no panic ─────────────────────────────────────────────

    #[test]
    fn node_without_pos_is_skipped_no_panic() {
        let mut manifest = checked_in_manifest();
        // Remove pos from N02.
        let n02 = manifest
            .nodes
            .iter_mut()
            .find(|n| n.id.as_str() == "N02")
            .expect("N02 must exist");
        n02.pos = None;

        let renderer = SvgRenderer;
        let scene = renderer.scene(&manifest, &LayoutView::default());

        // N02 must not appear in scene.nodes.
        assert!(
            scene.nodes.iter().all(|sn| sn.id.as_str() != "N02"),
            "N02 with pos=None must be absent from scene.nodes"
        );
        // Any edge touching N02 must also be absent.
        assert!(
            scene
                .edges
                .iter()
                .all(|e| e.from.as_str() != "N02" && e.to.as_str() != "N02"),
            "edges touching N02 must be absent when N02 has no pos"
        );
    }

    #[test]
    fn link_to_unknown_id_is_skipped_no_panic() {
        let mut manifest = checked_in_manifest();
        // Inject a link to a non-existent node.
        manifest.links.push(Link {
            from: NodeId::new("N01"),
            to: NodeId::new("N99"),
            kind: LinkKind::Child,
        });

        let renderer = SvgRenderer;
        // Must not panic.
        let scene = renderer.scene(&manifest, &LayoutView::default());

        // The bogus edge must be absent.
        assert!(
            scene.edges.iter().all(|e| e.to.as_str() != "N99"),
            "edge to unknown N99 must be skipped"
        );
    }

    #[test]
    fn small_manifest_node_without_pos_and_link_to_it_no_panic() {
        let manifest = Manifest {
            nodes: vec![
                Node {
                    id: NodeId::new("N01"),
                    kind: NodeKind::Question,
                    label: Some("Q".into()),
                    support_level: None,
                    source_refs: vec![],
                    description: None,
                    fields: NodeFields::Question,
                    evidence_notes: vec![],
                    pos: Some(ara_core::Point { x: 90.0, y: 30.0 }),
                },
                Node {
                    id: NodeId::new("N02"),
                    kind: NodeKind::Experiment,
                    label: None,
                    support_level: None,
                    source_refs: vec![],
                    description: None,
                    fields: NodeFields::Experiment { result: None },
                    evidence_notes: vec![],
                    pos: None, // <-- no pos
                },
            ],
            links: vec![Link {
                from: NodeId::new("N01"),
                to: NodeId::new("N02"),
                kind: LinkKind::Child,
            }],
            bindings: vec![],
            claims: vec![],
            bounds: None,
        };

        let renderer = SvgRenderer;
        let scene = renderer.scene(&manifest, &LayoutView::default());

        assert_eq!(scene.nodes.len(), 1, "only N01 (with pos) appears");
        assert!(scene.edges.is_empty(), "edge to posless N02 is skipped");
    }

    // ── bounds fallback ──────────────────────────────────────────────────────

    #[test]
    fn bounds_fallback_when_manifest_has_none() {
        let manifest = Manifest {
            nodes: vec![],
            links: vec![],
            bindings: vec![],
            claims: vec![],
            bounds: None,
        };

        let renderer = SvgRenderer;
        let scene = renderer.scene(&manifest, &LayoutView::default());

        assert!(
            scene.bounds.width > 0.0 && scene.bounds.height > 0.0,
            "fallback bounds must have positive extent"
        );
        // Must be the (0,0,100,100) fallback.
        assert_eq!(scene.bounds.x, 0.0);
        assert_eq!(scene.bounds.y, 0.0);
        assert_eq!(scene.bounds.width, 100.0);
        assert_eq!(scene.bounds.height, 100.0);
    }

    #[test]
    fn bounds_from_checked_in_manifest_are_not_fallback() {
        let manifest = checked_in_manifest();
        let renderer = SvgRenderer;
        let scene = renderer.scene(&manifest, &LayoutView::default());

        // The demo manifest bounds are ~1547×390, not the fallback.
        assert!(
            scene.bounds.width > 100.0,
            "real manifest bounds must be larger than the fallback"
        );
    }
}
