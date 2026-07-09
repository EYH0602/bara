# Issue #3 — Real-ARA no-panic regression coverage

**Issue:** https://github.com/EYH0602/bara/issues/3
**PR target:** `issue-3-real-corpus` → `main`. **Depends on:** Stage 1 (#2, merged).
**Version bump:** `0.0.2 → 0.0.3`.

## Problem background

Stage 1 (#2) validated the parser against the **two canonical** examples only
(`minimal-artifact`, `resnet-ara-example`), asserting a *clean* parse (zero
warnings, zero errors). But the real corpora exercise a much wider schema:

- `AmberLJC/ara-paperbench` — 32 artifacts, **CC-BY-4.0**
- `ARA-Labs/ARA-Demo` — 2 artifacts, **no LICENSE** (all rights reserved)

Running `ara validate` over all 34 shows the parser is **panic-free**, but
every artifact emits diagnostics because the real schema is a superset (extra
node fields like `failure_mode`/`hypothesis`/`lesson`, transition fields
`from`/`to`/`trigger`, real cycles, broken `evidence:` claim refs, and one
`ara-2.0` streams document with no `tree:`). See `docs/ara-format-feedback.md`
§13 and `TODOS.md` → `T-REAL-CORPUS`.

We want CI to keep guaranteeing **"runs to completion without panicking on real
data"** — a robustness check, permanently.

### What this is NOT

- **Not** a clean-parse check. Real artifacts legitimately produce
  warnings/errors; the test must **not** assert zero diagnostics.
- **Not** T-REAL-CORPUS (widening the model to parse real artifacts *cleanly*).
  That stays deferred. This issue only adds the **regression net** that proves
  no-panic + report-produced, and locks in the drift knowledge before Stage 2
  freezes the wire schema.

## Assertion semantics (the contract)

For each corpus artifact, the test asserts:

1. `parse_dir` / `parse_sources` **does not panic**, and
2. it **produces a `ParseReport`** — i.e. returns either
   `Ok((manifest, report))` or `Err(report)`. **Both outcomes pass.** A broken
   artifact that returns `Err(report)` is a *success* for this test; only a
   panic (or a hang) is a failure.

Practically, in Rust this means: call `parse_dir`, match on `Ok`/`Err`, and in
both arms confirm a report exists. Wrapping the call so a panic surfaces as a
clear per-artifact failure message (which artifact panicked) is the whole point.

## Proposed solution: **Both** (per the issue's decided approach)

### Part 1 — Vendored subset (required, hermetic CI check)

Copy a curated subset of `ara-paperbench` artifacts into
`crates/ara-core/tests/fixtures/corpus/`, each keeping only the two files the
parser reads (`trace/exploration_tree.yaml` + `logic/claims.md`), plus a
`SOURCE.md` recording origin, pinned commit, and CC-BY attribution (mirroring
`official/SOURCE.md`). An **always-on** test iterates them and asserts
no-panic + report-produced. Stays offline and `--locked`-friendly.

**Subset** (from the issue — chosen to span the drift dimensions):

| artifact | drift dimension it covers |
|----------|---------------------------|
| `extra/andes` | small; warnings-only (`failure_mode`/`hypothesis`/`lesson`) |
| `extra/expbench` | real cycle + transition fields (`from`/`to`/`trigger`) |
| `paperbench/sample-specific-masks` | broken `evidence:` claim refs (errors) |
| `speedrun/nanogpt-speedrun` | many errors (29) — stresses the error path |
| `rebench/rebench-rust_codecontests` | large; many warnings (35) |
| `rebench/rebench-restricted_mlm` | **`ara-2.0`** streams format (no `tree:`) |

Pinned commit: `3fe7ab4d08f68555d8c4661fa2b4fbfd4d597fd8`.

> Note: the `pivot` node type appears only in `ARA-Demo/nanogpt_ara`
> (unlicensed) → it can only be covered by the submodule sweep, not vendored.

### Part 2 — Submodule full sweep (opt-in, maintainer-run)

Add git submodule(s) for the full corpora behind an env-gated / `#[ignore]`
test so maintainers can sweep **all 34** artifacts locally. This is where the
broad coverage — and the unlicensed `ARA-Demo` (pointer only, not vendored) —
lives. Required CI does **not** check out submodules.

- Submodules under e.g. `corpus-external/ara-paperbench` and
  `corpus-external/ara-demo`, pinned to the commits above (`ara-demo` pin:
  `8f184717bb7827fe59d47a1ac44a00f66c6375ee`).
- The sweep test walks each submodule for any directory containing
  `trace/exploration_tree.yaml`, runs `parse_dir`, and asserts no-panic +
  report. Gate: `RUN_CORPUS_SWEEP=1` (skips cleanly, logging why, when unset or
  when the submodule dir is absent — so a fresh clone without submodules still
  passes).

## Implementation steps

1. **Vendor the subset.** Fetch `ara-paperbench` at the pinned commit; copy the
   6 artifacts' `trace/exploration_tree.yaml` (+ `logic/claims.md` where
   present) into `crates/ara-core/tests/fixtures/corpus/<artifact>/`. Do **not**
   hand-edit them.
2. **Write `corpus/SOURCE.md`** mirroring `official/SOURCE.md`: upstream repo,
   pinned commit, CC-BY-4.0 attribution, and the artifact→upstream-path table.
3. **Always-on test** in a new `crates/ara-core/tests/corpus_no_panic.rs`:
   iterate every artifact dir under `fixtures/corpus/`, run `parse_dir`,
   `catch_unwind` around it, assert no panic and that a report is produced in
   both `Ok`/`Err` arms. Fail message names the offending artifact. Discover
   dirs by walking the fixtures tree (don't hardcode the 6 names — future
   additions auto-covered).
   - Gate on `#[cfg(feature = "native")]` (matches existing `parse_dir` tests).
4. **Add submodule(s).** `git submodule add` the two upstream repos under
   `corpus-external/`, pin to the commits, and document each in `.gitmodules`.
5. **Opt-in sweep test** (same file or a sibling): `#[ignore]` **and** env-gated
   (`RUN_CORPUS_SWEEP=1`); recursively finds artifacts in the submodule paths;
   same no-panic + report assertion; skips with a log line when the env var is
   unset or the submodule dir is missing.
6. **Docs:**
   - `CONTRIBUTING.md`: a "Running the full corpus sweep" section — how to
     `git submodule update --init` and run `RUN_CORPUS_SWEEP=1 cargo test`.
   - Note in `corpus/SOURCE.md` that `ARA-Demo` is intentionally **not** vendored
     (no license) and is reachable only via submodule.
7. **CI:** confirm the default `test` job does **not** init submodules (current
   `actions/checkout` has no `submodules:` key → already correct; add an explicit
   comment so nobody adds it later). No new required job.
8. **Version + changelog:** bump `0.0.2 → 0.0.3` in `Cargo.toml`; refresh
   `Cargo.lock` with a plain `cargo build`; add a CHANGELOG `Added` entry.
9. **Close the loop on `TODOS.md`:** `T-REAL-CORPUS` stays open (widening is
   still deferred), but add a line noting the no-panic regression net now exists
   (this issue) so the two aren't conflated.

## Tests / verification

- `cargo test --workspace --locked` green, including the new always-on
  `corpus_no_panic` test over the 6 vendored artifacts.
- Existing `official_fixtures_are_clean` and snapshot tests **unchanged** and
  still green (this issue adds fixtures under a new `corpus/` subtree; it must
  not touch `official/`).
- Manual: `RUN_CORPUS_SWEEP=1 git submodule update --init && cargo test -- --ignored`
  sweeps all 34 and passes (no panics), locally.
- Manual: with submodules **absent**, `cargo test --workspace --locked` still
  passes (sweep test skips cleanly).
- `--locked` + wasm build unaffected (fixtures are test-only, `native`-gated).

## Acceptance (maps to issue checklist)

- [ ] `crates/ara-core/tests/fixtures/corpus/` holds the vendored subset +
      `SOURCE.md` (CC-BY attribution, pinned commit).
- [ ] Always-on test: each corpus artifact parses without panic and returns a
      report; runs in default `cargo test` and CI.
- [ ] Submodule(s) added; opt-in/`#[ignore]` full-sweep test that only runs when
      explicitly enabled (documented in `CONTRIBUTING.md`).
- [ ] `.gitmodules` documented; CI does **not** require submodule checkout for
      the default job.
- [x] Follow-up filed upstream asking `ARA-Labs/ARA-Demo` to add a license.
      *(filed — paste URL here)*

## Out of scope (explicitly deferred)

- Widening the model to parse real artifacts *cleanly* (`T-REAL-CORPUS`): new
  node fields, `pivot` kind, `ara-2.0` streams. This issue only proves no-panic.
- `E##` evidence-proof resolution (`T-EVIDENCE`).

## Open questions for review

1. **Submodule layout:** one submodule per repo under `corpus-external/`, or a
   single `corpus-external/` dir? (Plan assumes per-repo — cleaner pinning.)
2. **Vendoring mechanics:** copy files by hand from the pinned commit vs. a
   scripted `scripts/vendor-corpus.sh` that re-copies from a checkout for
   reproducibility. (Plan assumes a small documented script so re-vendoring at a
   new pin is one command; acceptable to do it by hand for 6 artifacts.)
3. **Gate style:** `#[ignore]` + `RUN_CORPUS_SWEEP=1` both, or env-var only?
   (Plan uses both so `cargo test` never runs it and `--ignored` still respects
   the env gate — belt and suspenders.)

## CHANGELOG (Unreleased → Added)

- Real-ARA no-panic regression coverage: vendored `ara-paperbench` subset under
  `crates/ara-core/tests/fixtures/corpus/` with an always-on test asserting the
  parser never panics and always produces a `ParseReport`; opt-in submodule
  full-sweep test (`RUN_CORPUS_SWEEP=1`) over all 34 real artifacts (#3).
