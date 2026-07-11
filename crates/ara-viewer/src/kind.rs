//! Single source of truth for `NodeKind` display metadata.
//!
//! `kind_meta` maps every [`NodeKind`] variant to a [`KindMeta`] that carries
//! the CSS class (equals the serde snake_case wire tag for canonical kinds),
//! a single-character chip glyph, and a human-readable badge string.

use ara_core::NodeKind;

/// Display metadata for a single [`NodeKind`] variant.
#[derive(Debug, PartialEq)]
pub struct KindMeta {
    /// CSS class name. Equals the serde `snake_case` wire tag for the 5
    /// canonical kinds (`question`, `experiment`, `decision`, `dead_end`,
    /// `insight`). Fixed to `"other"` for `Other(_)` — never derived from the
    /// raw string.
    pub css_class: &'static str,
    /// Single-character chip glyph.
    pub glyph: char,
    /// Lowercase display text. For `Other(raw)` this equals the raw string.
    pub badge: String,
}

/// Return display metadata for `kind`.
///
/// This is the **single source of truth** for wire string / glyph / label /
/// CSS class — no other code should hard-code these mappings.
pub fn kind_meta(kind: &NodeKind) -> KindMeta {
    match kind {
        NodeKind::Question => KindMeta {
            css_class: "question",
            glyph: 'Q',
            badge: "question".to_string(),
        },
        NodeKind::Experiment => KindMeta {
            css_class: "experiment",
            glyph: 'E',
            badge: "experiment".to_string(),
        },
        NodeKind::Decision => KindMeta {
            css_class: "decision",
            glyph: 'D',
            badge: "decision".to_string(),
        },
        NodeKind::DeadEnd => KindMeta {
            css_class: "dead_end",
            glyph: 'X',
            badge: "dead end".to_string(),
        },
        NodeKind::Insight => KindMeta {
            css_class: "insight",
            glyph: 'I',
            badge: "insight".to_string(),
        },
        NodeKind::Other(raw) => KindMeta {
            css_class: "other",
            glyph: '?',
            badge: raw.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── per-variant mapping tests ─────────────────────────────────────────────

    #[test]
    fn question_mapping() {
        let m = kind_meta(&NodeKind::Question);
        assert_eq!(m.css_class, "question");
        assert_eq!(m.glyph, 'Q');
        assert_eq!(m.badge, "question");
    }

    #[test]
    fn experiment_mapping() {
        let m = kind_meta(&NodeKind::Experiment);
        assert_eq!(m.css_class, "experiment");
        assert_eq!(m.glyph, 'E');
        assert_eq!(m.badge, "experiment");
    }

    #[test]
    fn decision_mapping() {
        let m = kind_meta(&NodeKind::Decision);
        assert_eq!(m.css_class, "decision");
        assert_eq!(m.glyph, 'D');
        assert_eq!(m.badge, "decision");
    }

    #[test]
    fn dead_end_mapping() {
        let m = kind_meta(&NodeKind::DeadEnd);
        assert_eq!(m.css_class, "dead_end");
        assert_eq!(m.glyph, 'X');
        assert_eq!(m.badge, "dead end");
    }

    #[test]
    fn insight_mapping() {
        let m = kind_meta(&NodeKind::Insight);
        assert_eq!(m.css_class, "insight");
        assert_eq!(m.glyph, 'I');
        assert_eq!(m.badge, "insight");
    }

    #[test]
    fn other_mapping_uses_raw_for_badge_not_css() {
        let m = kind_meta(&NodeKind::Other("custom_thing".into()));
        // css_class is fixed — NEVER derived from raw
        assert_eq!(m.css_class, "other");
        assert_eq!(m.glyph, '?');
        // badge IS the raw string
        assert_eq!(m.badge, "custom_thing");
    }

    #[test]
    fn other_mapping_empty_raw() {
        let m = kind_meta(&NodeKind::Other(String::new()));
        assert_eq!(m.css_class, "other");
        assert_eq!(m.badge, "");
    }

    // ── drift-guard: css_class == serde snake_case wire tag ───────────────────
    //
    // These assert the exact expected strings without pulling in serde_json.
    // If ara-core renames a variant or changes its serde attribute, this test
    // will catch the drift.

    #[test]
    fn drift_guard_css_class_equals_wire_tag() {
        // Wire tags (serde rename_all = "snake_case"):
        //   Question   -> "question"
        //   Experiment -> "experiment"
        //   Decision   -> "decision"
        //   DeadEnd    -> "dead_end"
        //   Insight    -> "insight"
        let cases: &[(&NodeKind, &str)] = &[
            (&NodeKind::Question, "question"),
            (&NodeKind::Experiment, "experiment"),
            (&NodeKind::Decision, "decision"),
            (&NodeKind::DeadEnd, "dead_end"),
            (&NodeKind::Insight, "insight"),
        ];
        for (kind, expected_wire_tag) in cases {
            let m = kind_meta(kind);
            assert_eq!(
                m.css_class, *expected_wire_tag,
                "css_class for {:?} must equal serde wire tag \"{}\"",
                kind, expected_wire_tag
            );
        }
    }
}
