# Plan: Rename the "Recipes" viewer panel to "Solution files"

## Problem background

Issue [#35](https://github.com/ARA-Labs/ara-cli/issues/35) asked whether one "recipe"
is one `logic/solution/*.md` file or one `##`-level entry inside those files. The two
units disagree by ~10x on the same corpus (e.g. `arc-agi3/ls20`: 2 files vs 22 `##`
blocks), and nothing in the ARA schema makes one canonical. The viewer currently ships a
per-file fallback (`read_recipes` in `crates/ara-core/src/parse.rs`) but labels the panel
"Recipes", which over-claims — the number is a file count, not a grounded recipe count.

Maintainer decision (AmberLJC, on #35): **do not block T6 on defining "recipe."** Rename
the panel so the number is honestly a file count, keep the per-file fallback as-is, and
define "recipe" properly later once something needs to reference a single one.

## Proposed solution

UI-only rename. The count stays the per-file count of `logic/solution/*.md` (correct
by definition once the label matches). No parsing change, no schema decision. Internal
`Recipe` type / `manifest.recipes` field / `read_recipes` names stay untouched to avoid
premature churn — they get renamed alongside the real definition later.

**Scope: user-facing strings + tests + version/changelog only.**

## Implementation steps

### 1. `crates/ara-viewer/src/panels.rs` (`RecipesPanel`)
- Launcher label `"Recipes"` → `"Solution files"` (line ~309).
- Modal title `"Recipes"` → `"Solution files"` (line ~315).
- Filter `aria-label` `"Filter recipes"` → `"Filter solution files"` (line ~320).
- Empty-state `"No recipes match the filter."` → `"No solution files match the filter."`
  (line ~332).
- Rewrite the doc comment (lines ~288–290): drop the "E8 unresolved / fallback / not the
  ungrounded 28" framing. State plainly that the panel counts `logic/solution/*.md` files
  and cite the #35 decision. Keep the internal name `RecipesPanel` / `recipes` signal.
- Leave `recipe_entry` / `.recipe-entry` / `.recipe-body` CSS classes and the per-item
  render as-is (still one entry per solution file).

### 2. `crates/ara-viewer/tests/web.rs` (`recipes_shows_count_and_opens`)
- Update the launcher assertion: `.expect("Recipes launcher present")` message and
  `assert!(btn_text.contains("Recipes"), ...)` → expect `"Solution files"`.
- Keep the count assertion (`contains('4')`) — still one per solution file.
- Keep `"Recipe 1"` / `"step body for recipe 1"` assertions: those are fixture titles/
  bodies from `manifest_with_panels`, still valid content.

### 3. Docs (`docs/hub-parity.md`) — required by CLAUDE.md, ship in this PR
Eng-review finding (confidence 9/10): the docs go stale on this change and one note
becomes factually wrong. Fix in the same commit:
- Line ~6 and line ~132 (`**Recipes** (solution files)`): rename the panel reference
  from "Recipes" to "Solution files".
- Lines ~146–148, the E8 deferred note (`The "recipe" unit (E8) — undefined upstream;
  the Recipes count uses the fallback ... pending a maintainer answer.`): rewrite from
  "pending" to **resolved per #35** — maintainer decided to label the panel
  "Solution files", keep the per-file count, and defer the canonical "recipe" unit
  until something needs to reference a single one.
- Leave the internal `recipes: Vec<Recipe>` field-mapping row (~line 68) as-is — that
  documents the manifest field, which is not renamed.

### 4. Version + changelog (functional change → patch bump)
- `Cargo.toml`: bump `version = "0.1.8"` → `"0.1.9"`. `v0.1.8` is already tagged, so
  0.1.8 is taken; next patch is 0.1.9. (Flag to maintainer, separate housekeeping: the
  `[Unreleased]` homebrew entries were never rolled under a `## [0.1.8]` heading despite
  the tag — not part of this PR.)
- `CHANGELOG.md` under `## [Unreleased]` → `### Changed`:
  `- Viewer: renamed the Recipes panel to "Solution files" so its count is honestly a`
  `  per-file count of `logic/solution/*.md` rather than an ungrounded "recipe" count`
  `  (#35).`

## Verification

- `cargo build` — compiles.
- `cargo test -p ara-viewer` — the wasm web test `recipes_shows_count_and_opens`
  exercises the new label; native panel unit tests still pass.
- `cargo test -p ara-core` — unchanged, confirms `read_recipes` untouched.
- Manual: `cargo run -- serve` on `arc-agi3/ls20` (2 files) and the DoD artifact
  (4 files); confirm the launcher reads "Solution files 2" / "Solution files 4" and the
  modal lists each file's body.
- Grep check: `grep -rn '"[^"]*Recipe' crates/ara-viewer/src/` returns nothing after
  the edit (no user-facing "Recipe" label left behind); `docs/hub-parity.md` no longer
  says the recipe unit is "pending".

## Out of scope (deferred per #35)

- Defining the canonical "recipe" unit.
- Renaming the internal `Recipe` struct, `manifest.recipes`, `read_recipes`, or CSS
  classes.
- Parsing `##`-level entries inside solution files.

## Follow-up

- Post the PR link on #35 and close the issue once merged.

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
|--------|---------|-----|------|--------|----------|
| CEO Review | `/plan-ceo-review` | Scope & strategy | 0 | — | — |
| Codex Review | `/codex review` | Independent 2nd opinion | 0 | — | — |
| Eng Review | `/plan-eng-review` | Architecture & tests (required) | 1 | issues_open→resolved | 1 issue (stale docs), folded into plan; 0 critical gaps |
| Design Review | `/plan-design-review` | UI/UX gaps | 0 | — | — |
| DX Review | `/plan-devex-review` | Developer experience gaps | 0 | — | — |

- **VERDICT:** ENG CLEARED — architecture/tests/perf clean; the one finding (stale
  `docs/hub-parity.md` + resolved E8 note) is now step 3 of the plan. Version resolved to
  0.1.9. Ready to implement.

NO UNRESOLVED DECISIONS
