//! Integration tests over the pinned fixture corpus.

use std::path::{Path, PathBuf};

use ara_core::parse_sources;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(fixtures().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// Both official artifacts must parse with **zero** errors and **zero**
/// warnings (every canonical field is modeled) — the Stage-1 acceptance bar.
#[test]
#[cfg(feature = "native")]
fn official_fixtures_are_clean() {
    for dir in ["minimal-artifact", "resnet-ara-example"] {
        let path = fixtures().join("official").join(dir);
        let (manifest, report) =
            ara_core::parse_dir(&path).unwrap_or_else(|r| panic!("{dir} failed: {r}"));
        assert!(report.is_ok(), "{dir} has errors: {report}");
        assert!(report.warnings().is_empty(), "{dir} has warnings: {report}");
        assert!(!manifest.nodes.is_empty(), "{dir} produced no nodes");
    }
}

#[test]
#[cfg(feature = "native")]
fn minimal_manifest_snapshot() {
    let path = fixtures().join("official/minimal-artifact");
    let (manifest, _) = ara_core::parse_dir(&path).expect("ok");
    insta::assert_json_snapshot!("minimal_manifest", manifest);
}

#[test]
#[cfg(feature = "native")]
fn resnet_manifest_snapshot() {
    let path = fixtures().join("official/resnet-ara-example");
    let (manifest, _) = ara_core::parse_dir(&path).expect("ok");
    insta::assert_json_snapshot!("resnet_manifest", manifest);
}

/// Parsing the same input twice yields byte-identical JSON across all four
/// vectors — determinism from source-order preservation.
#[test]
#[cfg(feature = "native")]
fn parse_is_deterministic() {
    let path = fixtures().join("official/resnet-ara-example");
    let (a, _) = ara_core::parse_dir(&path).expect("ok");
    let (b, _) = ara_core::parse_dir(&path).expect("ok");
    let ja = serde_json::to_string_pretty(&a).unwrap();
    let jb = serde_json::to_string_pretty(&b).unwrap();
    assert_eq!(ja, jb);
}

/// `parse_dir` on the real minimal artifact resolves the C01 binding (claims.md
/// present) and leaves no unresolved-binding warning.
#[test]
#[cfg(feature = "native")]
fn parse_dir_resolves_bindings() {
    let path = fixtures().join("official/minimal-artifact");
    let (manifest, report) = ara_core::parse_dir(&path).expect("ok");
    assert!(!manifest.bindings.is_empty());
    assert!(
        !report
            .warnings()
            .iter()
            .any(|w| w.message.contains("unresolved"))
    );
}

#[test]
fn root_single_dialect_normalizes() {
    let yaml = read("synthetic/root_single.yaml");
    let (manifest, report) = parse_sources(&yaml, None).expect("ok");
    assert!(report.is_ok());
    assert_eq!(manifest.nodes.len(), 2);
    assert_eq!(manifest.links.len(), 1); // N01 -> N02
}

#[test]
fn broken_claim_ref_errors() {
    let yaml = read("broken/broken_claim_ref.yaml");
    // Provide claims that lack C99 so the reference is genuinely broken.
    let err = parse_sources(&yaml, Some("## C01: only claim\n")).unwrap_err();
    assert!(
        err.errors()
            .iter()
            .any(|d| d.message.contains("unknown claim")),
        "expected broken-claim error, got: {err}"
    );
}

#[test]
fn dup_id_errors() {
    let yaml = read("broken/dup_id.yaml");
    let err = parse_sources(&yaml, None).unwrap_err();
    assert!(
        err.errors()
            .iter()
            .any(|d| d.message.contains("duplicate node id"))
    );
}

#[test]
fn cycle_errors() {
    let yaml = read("broken/cycle.yaml");
    let err = parse_sources(&yaml, None).unwrap_err();
    assert!(err.errors().iter().any(|d| d.message.contains("cycle")));
}

#[test]
fn ambiguous_root_errors() {
    let yaml = read("broken/ambiguous_root.yaml");
    let err = parse_sources(&yaml, None).unwrap_err();
    assert!(err.errors().iter().any(|d| d.message.contains("both")));
}

/// A missing directory (or missing `exploration_tree.yaml`) is a clean error,
/// not a panic.
#[test]
#[cfg(feature = "native")]
fn missing_dir_is_clean_error() {
    let err = ara_core::parse_dir(Path::new("/no/such/ara/dir")).unwrap_err();
    assert!(!err.is_ok());
    assert!(err.errors()[0].message.contains("cannot read"));
}
