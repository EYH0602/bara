# Plan: `ara check` — ARA lint/format checker with `--fix` + a reusable CI action (#39)

## Problem / background

Issue [#39](https://github.com/ARA-Labs/ara-cli/issues/39) asks for CI support:
a way to drop a format/lint check into a repo that contains an ARA artifact, so
the ARA stays clean the same way `ruff` keeps a Python repo clean. From the
follow-up: the check should behave like a linter with an autofix flag (`--fix`),
and `ruff` is the **UX analogy** — `ara` stays self-contained and does **not**
shell out to `ruff` or any external tool.

Today `ara validate <dir>` already parses an ARA and emits errors/warnings, but:

- It has no autofix. A human must hand-edit every drift.
- The common, tolerated drift forms (see `docs/ara-format-feedback.md` items 2,
  5, 7) are handled today in three *different* ways, none of which gives the
  author a fixable signal (verified against `parse.rs` / `schema.rs` /
  `claims.rs`):
  - `root:` vs `tree:` — **silently normalized** (both accepted; `root:` becomes
    a one-element list). No diagnostic at all.
  - dead-end `reason:` / decision `justification:` — **not** recognized aliases;
    they fall into `extra` and surface as an `unknown field` **warning**, and the
    value is **dropped** (the canonical keys are `why_failed:` / `rationale:`).
  - em-dash vs colon claim headers (`## C01 — T`) — `parse_header` requires a
    `:` separator, so a non-colon header makes the **entire claim silently
    disappear** (claims.rs emits no diagnostics); it only surfaces indirectly as
    an "unknown claim" error if some node references the dropped id.
- There is no packaged CI artifact a downstream repo can reference.

## Goals

1. `ara check <dir>` — a lint front-end that reports diagnostics and marks the
   ones that are auto-fixable, exiting non-zero when the ARA is not clean.
2. `ara check --fix <dir>` — apply **surgical, safe** source edits that
   canonicalize the fixable drift, leaving everything else (comments, ordering,
   unrelated formatting) untouched.
3. A **reusable GitHub Action** (`ARA-Labs/ara-cli/.github/actions/check`) plus a
   copy-paste workflow snippet, so a downstream repo can gate its ARA in CI.

## Non-goals (confirmed with the maintainer)

- **Not** a full canonical re-serializer / formatter (`ruff format` style). We do
  **not** re-emit the YAML/Markdown from the parsed model — that would destroy
  comments, key order, and author style. Fixes are surgical text edits only.
- **Not** a dependency on the `ruff` binary or any external linter. `ruff` is a
  design analogy for the UX (fast, `--fix`, clear diagnostics), nothing more.
- **Not** changing `ara validate`'s current output or exit behavior.

## Design

### 1. Command surface

New subcommand, reusing the existing parse engine:

```
ara check <dir> [--fix] [--strict] [--json]
```

| flag       | behavior |
| ---------- | -------- |
| (none)     | Parse, run format-lint, print diagnostics; annotate auto-fixable ones with `[fixable]`. Exit non-zero if there are errors **or** unfixed fixable issues. |
| `--fix`    | Apply the fixable rules to the source files in place, re-check, and report what changed. Exit reflects the **post-fix** state. |
| `--strict` | Treat remaining warnings as failure (mirrors `validate --strict`). |
| `--json`   | Machine-readable report (for CI annotations). |

Rationale for a **new** `check` command rather than adding `--fix` to
`validate`: `validate` is the deterministic semantic gate that other tooling and
docs already lean on; keeping its output byte-stable matters. `check` is the
opinionated, fixable, CI-facing front-end. They share the parse engine but have
distinct contracts. (Open question OQ1 below if you'd rather fold this into
`validate`.)

### 2. Two diagnostic sources

`ara check` composes two layers:

- **Validate diagnostics (reused):** the exact errors/warnings from the existing
  `parse_dir` pipeline — duplicate ids, unknown references, missing `id`/`type`,
  unknown fields, etc. These are reported as-is. Almost all require human
  judgment and are reported **not fixable**.
- **Format-lint rules (new):** a small, closed set of rules that detect the
  *canonicalizable* drift and know how to rewrite it. These carry the `[fixable]`
  marker and are the only thing `--fix` touches. Two of them (`ARA002/003`)
  correspond to drift `validate` **already** warns about (the `unknown field`
  warnings for `reason:`/`justification:`) — `check` upgrades those specific
  warnings to `[fixable]` and the fix stops the value being dropped. The other
  two (`ARA001/004`) cover drift `validate` is currently **silent** about (see
  Background), so the lint layer is what first makes them visible.

The format-lint rules live in a **new `ara-core` module** (`lint.rs`) that reads
the raw source text. They are **not** wired into `parse_dir`, so `validate`'s
output is unchanged (non-goal above).

### 3. Initial fixable rule set

Ship **all four** in v1 (decision below). Each maps to a documented drift in
`docs/ara-format-feedback.md`.

| id   | detects | fix | kind |
| ---- | ------- | --- | ---- |
| `ARA001` root-dialect | top-level `root:` (single node) | rewrite to `tree:` with a one-element list (re-indent the block) | structural |
| `ARA002` dead-end-reason-alias | `reason:` on a `dead_end` node | rename key to `why_failed:` | line-level (context-scoped) |
| `ARA003` decision-rationale-alias | `justification:` on a `decision` node | rename key to `rationale:` | line-level (context-scoped) |
| `ARA004` claim-header-style | `## C01 — Title` / `## C01 - Title` in `claims.md` | rewrite separator to `## C01: Title` | line-level |

`ARA001` is the only structural edit and the riskiest. It ships in v1 but leans
hard on the §4 re-parse-equivalence guard: if a specific `root:` block can't be
rewritten to a semantically identical `tree:`, the guard discards that edit and
`check` reports it as detected-but-not-auto-fixed rather than risking a corrupt
file. A follow-up update will harden the structural rewrite after v1 shakes out.

### 4. Fix safety guarantee (the load-bearing correctness property)

Every `--fix` write must be a **semantic no-op**: applying the surgical edits and
re-parsing must yield a normalized `Manifest` byte-identical to the pre-fix
normalized `Manifest`. Concretely, per file:

1. Parse the dir → `manifest_before` (normalized).
2. Apply candidate edits to the raw text.
3. Re-parse → `manifest_after`.
4. If `manifest_after == manifest_before`, write the file; otherwise **discard**
   that edit, keep the original, and report the rule as non-applied with a note.

This makes autofix stable and non-destructive by construction (analogous to
ruff's fix-stability check) and protects the structural `ARA001` edit from
corrupting indentation. `Manifest` already derives the traits needed for
equality comparison (verify during implementation; add `PartialEq` if missing).

### 5. Exit codes (precise)

- `0` — no errors and no unfixed fixable issues (and, under `--strict`, no
  warnings).
- `1` — errors present, or fixable issues remain (in non-`--fix` mode this is the
  "run --fix" signal, like `ruff check` without `--fix`).
- `2` — internal failure (unreadable dir, serialization error).

### 6. Reusable GitHub Action + docs snippet

- `.github/actions/check/action.yml` — a **composite** action that installs the
  released `ara` binary (via the cargo-dist shell installer, pinned by tag) and
  runs `ara check` on an input path. Inputs: `path` (required), `strict`
  (default `false`), `version` (tag to install, default the action's release),
  `args` (escape hatch). No `fix` input — CI checks, it does not mutate.
- **Docker image (fast path, like `ruff`/`uv`):** publish a small image with
  `ara` preinstalled so CI can skip the install step entirely. The repo already
  has a `Dockerfile`; add a slim `ara`-only image built + pushed (GHCR) on
  release, and document a `container:`-based usage. Ordering: land the composite
  action first (works immediately off release binaries), add the image as a
  fast-path follow-up within this feature if the release wiring is
  straightforward.
- A documented workflow snippet in the docs / README that downstream repos copy:

  ```yaml
  # .github/workflows/ara.yml (in the user's repo)
  jobs:
    ara-check:
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v4
        - uses: ARA-Labs/ara-cli/.github/actions/check@v0
          with:
            path: ./my-ara
  ```

- Wire the same action into **this** repo's `ci.yml` against a corpus artifact as
  a dogfood/self-test job (guarded so it does not require the heavy submodule on
  every run — mirror the existing corpus handling).

## Implementation steps

1. `ara-core`: add `lint.rs` with the rule trait, the initial rules
   (`ARA001–004`), and a `check_dir(dir) -> LintReport` entry that returns
   diagnostics + fix candidates. Ensure `Manifest: PartialEq` for the safety
   check.
2. `ara-core`: add the fix applier with the re-parse-equivalence guard; expose
   `fix_dir(dir) -> FixOutcome`.
3. `ara-cli`: add the `Check` subcommand + `--fix/--strict/--json`, human and
   JSON renderers, exit-code mapping. Reuse `emit_report` plumbing where sensible.
4. `.github/actions/check/action.yml` composite action + a self-test job in
   `ci.yml`.
5. Docs: `docs/stage-5-check.md` design doc; README usage block; CHANGELOG
   `Added` entry; bump patch version in `Cargo.toml` (functional change).

## Testing

- Unit tests per rule: a non-canonical fixture → detected as `[fixable]`; after
  `--fix` the file is canonical **and** re-parses to the same `Manifest`.
- Negative tests: a `reason:`/`justification:` key that is **not** on a
  dead-end/decision node is left untouched (no false rewrite).
- Safety test: a hand-broken edit that would change semantics is discarded, file
  unchanged.
- CLI integration tests (assert_cmd-style, matching existing patterns) for exit
  codes across clean / fixable / error / `--strict` / `--fix` cases.
- Idempotence: `ara check --fix` twice ⇒ second run is a no-op, exit `0`.
- Run `ara check` over the real corpus submodule locally to confirm no panics and
  sane fixable counts.

## Resolved decisions

- **D1 — command name:** `ara check` with an `--fix` option. ✓
- **D2 — rule set:** ship all four (`ARA001–004`) in v1; a follow-up update
  hardens the structural `ARA001` rewrite after v1. ✓
- **D3 — action install:** cargo-dist shell installer pinned by tag, **plus** a
  published Docker image as a fast path (mirrors `ruff`/`uv`). ✓
- **D4 — rule config:** hardcode the rule set for v1; `.ara-check.toml` per-rule
  config tracked as follow-up [#40](https://github.com/ARA-Labs/ara-cli/issues/40). ✓
