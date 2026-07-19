//! CLI integration tests for `ara validate`.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn ara() -> Command {
    Command::cargo_bin("ara").expect("binary builds")
}

fn official(name: &str) -> PathBuf {
    // ara-cli/tests -> ara-cli -> crates -> repo root, then into ara-core fixtures.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ara-core/tests/fixtures/official")
        .join(name)
}

/// Builds a temp ARA artifact with the given tree YAML and optional claims.
fn artifact(tree_yaml: &str, claims_md: Option<&str>) -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("trace")).unwrap();
    std::fs::write(dir.path().join("trace/exploration_tree.yaml"), tree_yaml).unwrap();
    if let Some(claims) = claims_md {
        std::fs::create_dir_all(dir.path().join("logic")).unwrap();
        std::fs::write(dir.path().join("logic/claims.md"), claims).unwrap();
    }
    dir
}

#[test]
fn validate_official_exits_zero() {
    ara()
        .arg("validate")
        .arg(official("minimal-artifact"))
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));

    ara()
        .arg("validate")
        .arg(official("resnet-ara-example"))
        .assert()
        .success();
}

#[test]
fn validate_broken_exits_nonzero() {
    let dir = artifact(
        "tree:\n  - id: N01\n    type: question\n  - id: N01\n    type: insight\n",
        None,
    );
    ara()
        .arg("validate")
        .arg(dir.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains("duplicate node id"));
}

#[test]
fn json_output_is_valid_json() {
    let output = ara()
        .arg("validate")
        .arg(official("minimal-artifact"))
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let parsed: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    assert!(parsed.get("errors").is_some());
    assert!(parsed.get("warnings").is_some());
}

#[test]
fn strict_promotes_warnings_to_failure() {
    // Unknown field -> warning, no error. Exit 0 normally, non-zero with --strict.
    let dir = artifact(
        "tree:\n  - id: N01\n    type: question\n    title: q\n    bogus_field: 1\n",
        None,
    );

    ara()
        .arg("validate")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("unknown field"));

    ara()
        .arg("validate")
        .arg(dir.path())
        .arg("--strict")
        .assert()
        .failure();
}

#[test]
fn missing_dir_is_clean_error_not_panic() {
    ara()
        .arg("validate")
        .arg("/no/such/ara/dir")
        .assert()
        .failure()
        .stdout(predicate::str::contains("cannot read"));
}

#[test]
fn missing_tree_file_is_clean_error() {
    let dir = TempDir::new().unwrap(); // empty, no trace/exploration_tree.yaml
    ara()
        .arg("validate")
        .arg(dir.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains("cannot read"));
}

// ── Layout command tests ──────────────────────────────────────────────────

#[test]
fn layout_json_produces_valid_positioned_manifest() {
    let output = ara()
        .arg("layout")
        .arg(official("minimal-artifact"))
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let manifest: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    // Has nodes with pos
    let nodes = manifest["nodes"].as_array().unwrap();
    assert!(!nodes.is_empty());
    for node in nodes {
        assert!(node.get("pos").is_some(), "node missing pos: {node}");
        let pos = &node["pos"];
        assert!(pos["x"].as_f64().unwrap().is_finite());
        assert!(pos["y"].as_f64().unwrap().is_finite());
    }
    // Has bounds
    let bounds = &manifest["bounds"];
    assert!(bounds["width"].as_f64().unwrap() > 0.0);
    assert!(bounds["height"].as_f64().unwrap() > 0.0);
}

#[test]
fn layout_missing_dir_exits_nonzero() {
    ara()
        .arg("layout")
        .arg("/no/such/ara/dir")
        .arg("--json")
        .assert()
        .failure()
        .stdout(predicate::str::contains("layout skipped"));
}

#[test]
fn layout_parse_error_skips_layout() {
    let cycle_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cycle-dir");
    ara()
        .arg("layout")
        .arg(&cycle_dir)
        .arg("--json")
        .assert()
        .failure()
        .stdout(predicate::str::contains("layout skipped"));
}

#[test]
fn validate_layout_flag_shows_counts_and_bounds() {
    ara()
        .arg("validate")
        .arg(official("minimal-artifact"))
        .arg("--layout")
        .assert()
        .success()
        .stdout(predicate::str::contains("node(s)"))
        .stdout(predicate::str::contains("bounds"));
}

#[test]
fn validate_layout_on_error_matches_validate() {
    let cycle_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cycle-dir");
    ara()
        .arg("validate")
        .arg(&cycle_dir)
        .arg("--layout")
        .assert()
        .failure()
        .stdout(predicate::str::contains("cycle"));
}

// ── Check command tests ───────────────────────────────────────────────────

/// A clean official artifact passes `ara check` with exit 0.
#[test]
fn check_clean_official_exits_zero() {
    ara()
        .arg("check")
        .arg(official("minimal-artifact"))
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

/// A fixable format issue (a `reason:` on a `dead_end` node) is reported with its
/// rule id and `[fixable]` marker, and exits 1 without `--fix`.
#[test]
fn check_fixable_without_fix_exits_one() {
    let dir = artifact(
        "tree:\n  - id: N01\n    type: dead_end\n    reason: it diverged\n",
        None,
    );
    ara()
        .arg("check")
        .arg(dir.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains("ARA002"))
        .stdout(predicate::str::contains("[fixable]"));
}

/// `--fix` applies the safe fix in place; the change persists so a follow-up
/// `ara check` (no fix) on the same dir also passes.
#[test]
fn check_fix_applies_and_persists() {
    let dir = artifact(
        "tree:\n  - id: N01\n    type: dead_end\n    reason: it diverged\n",
        None,
    );
    ara()
        .arg("check")
        .arg(dir.path())
        .arg("--fix")
        .assert()
        .success()
        .stdout(predicate::str::contains("fixed ARA002"));

    // The fix reached disk: a plain re-check now passes.
    ara().arg("check").arg(dir.path()).assert().success();

    let fixed = std::fs::read_to_string(dir.path().join("trace/exploration_tree.yaml")).unwrap();
    assert!(fixed.contains("why_failed: it diverged"), "got: {fixed}");
}

/// Running `--fix` twice is idempotent: the second run applies nothing and exits 0.
#[test]
fn check_fix_is_idempotent() {
    let dir = artifact(
        "tree:\n  - id: N01\n    type: dead_end\n    reason: it diverged\n",
        None,
    );
    ara()
        .arg("check")
        .arg(dir.path())
        .arg("--fix")
        .assert()
        .success();

    ara()
        .arg("check")
        .arg(dir.path())
        .arg("--fix")
        .assert()
        .success()
        .stdout(predicate::str::contains("applied 0 fix(es)"));
}

/// A real validate error (duplicate node id) exits 1 and surfaces the error.
#[test]
fn check_validate_error_exits_one() {
    let dir = artifact(
        "tree:\n  - id: N01\n    type: question\n  - id: N01\n    type: insight\n",
        None,
    );
    ara()
        .arg("check")
        .arg(dir.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains("duplicate node id"));
}

/// An artifact that parses with only a warning passes normally but fails under
/// `--strict`.
#[test]
fn check_strict_promotes_warning_to_failure() {
    let dir = artifact(
        "tree:\n  - id: N01\n    type: question\n    title: q\n    bogus_field: 1\n",
        None,
    );

    ara()
        .arg("check")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("unknown field"));

    ara()
        .arg("check")
        .arg(dir.path())
        .arg("--strict")
        .assert()
        .failure();
}

/// `ara check --json` emits a parseable composed report.
#[test]
fn check_json_output_is_valid_json() {
    let output = ara()
        .arg("check")
        .arg(official("minimal-artifact"))
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let parsed: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    assert!(parsed.get("validate").is_some());
    assert!(parsed.get("lint").is_some());
    assert!(parsed.get("summary").is_some());
    assert!(parsed["summary"]["passed"].as_bool().unwrap());
}

/// `--json` must still honor the exit contract: a fixable issue exits 1 while
/// emitting valid JSON (regression: the no-fix JSON path used to always exit 0).
#[test]
fn check_json_fixable_exits_one_with_valid_json() {
    let dir = artifact(
        "tree:\n  - id: N01\n    type: dead_end\n    reason: it diverged\n",
        None,
    );
    let output = ara()
        .arg("check")
        .arg(dir.path())
        .arg("--json")
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let parsed: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    assert_eq!(parsed["summary"]["fixable"].as_u64().unwrap(), 1);
    assert!(!parsed["summary"]["passed"].as_bool().unwrap());
}

/// `--json` on a real validate error (duplicate node id) also exits 1 with valid
/// JSON.
#[test]
fn check_json_validate_error_exits_one_with_valid_json() {
    let dir = artifact(
        "tree:\n  - id: N01\n    type: question\n  - id: N01\n    type: insight\n",
        None,
    );
    let output = ara()
        .arg("check")
        .arg(dir.path())
        .arg("--json")
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let parsed: serde_json::Value = serde_json::from_slice(&output).expect("valid JSON");
    assert!(parsed["summary"]["errors"].as_u64().unwrap() >= 1);
    assert!(!parsed["summary"]["passed"].as_bool().unwrap());
}

/// A non-existent target maps to the internal-failure exit code 2.
#[test]
fn check_missing_dir_exits_two() {
    ara().arg("check").arg("/no/such/ara/dir").assert().code(2);
}
