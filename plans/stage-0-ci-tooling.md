# Stage 0 — CI & Tooling Foundation

**PR target:** `stage0-ci-tooling` → `main`. **Depends on:** none.
**Version bump:** `0.0.0 → 0.0.1`.

## Problem background

The workspace is scaffolded and the crate names are reserved, but there is no
automated gate. Before any real code lands, we need reproducible checks so every
later stage PR is validated the same way: formatting, lints, tests, and a
verification that `ara-core` (and `ara-wasm`) actually compile to
`wasm32-unknown-unknown` (the core promise of the architecture — one crate,
native + wasm). Catching a wasm-incompatible dependency in Stage 1 by hand is
expensive; CI should catch it.

## Proposed solution

A GitHub Actions workflow plus a **pinned, reproducible** toolchain and CI
environment, giving a single green/red signal for `fmt`, `clippy`, native tests,
and a `wasm32` build check of the wasm path. This stage adds no product logic,
but it does touch manifests (version bump, MSRV removal) and regenerates
`Cargo.lock`.

## Implementation steps

1. **Pin the toolchain (exactly).** Add `rust-toolchain.toml` at the repo root:
   ```toml
   [toolchain]
   channel = "1.94.1"                       # exact patch pin — matches determinism rule
   components = ["rustfmt", "clippy"]
   targets = ["wasm32-unknown-unknown"]     # rustup auto-installs on --target
   ```
   Rationale: `1.94` (floating minor) would let a rustc patch bump silently
   change codegen/clippy output, breaking the byte-determinism invariant.

2. **Formatting config.** Add `rustfmt.toml` (`max_width = 100`, `edition =
   "2024"`). **Do NOT add a `[workspace.lints]` table** — CI's
   `cargo clippy -- -D warnings` is the single lint-enforcement point (a
   `[workspace.lints]` table is inert unless every crate also declares
   `[lints]\nworkspace = true`, so it would be dead config).

3. **Drop the unverified MSRV.** Remove `rust-version = "1.85"` from
   `[workspace.package]` in `Cargo.toml` **and** remove the
   `rust-version.workspace = true` line from all four crate manifests
   (`ara-cli`, `ara-core`, `ara-wasm`, `ara-viewer`) — otherwise inheritance
   errors. (An MSRV that CI never builds is a claim that drifts; re-added +
   tested at the 0.1.0 publish, see `TODOS.md` → `T-MSRV`.)

4. **CI workflow** `.github/workflows/ci.yml`:
   ```yaml
   on:
     pull_request:
     push:
       branches: [main]
   concurrency:
     group: ci-${{ github.ref }}
     cancel-in-progress: true
   permissions:
     contents: read
   ```
   Jobs (runner `ubuntu-24.04`, cached via `Swatinem/rust-cache`, **all actions
   SHA-pinned** with a version comment):
   - `fmt`: `cargo fmt --all --check`.
   - `clippy`: `cargo clippy --workspace --all-targets --locked -- -D warnings`.
   - `test`: `cargo test --workspace --locked`.
   - `wasm`: `cargo build -p ara-core -p ara-wasm --target wasm32-unknown-unknown --locked`
     (guards the full native/wasm dual-build path, not just the shared crate).

   `--locked` on every building job is load-bearing: it fails CI if `Cargo.lock`
   is stale, which is exactly the determinism guard this stage exists to provide.

5. **Regenerate the lockfile.** After the `0.0.0 → 0.0.1` version bump, run
   `cargo build` so `Cargo.lock` records `0.0.1` for all four crates, and commit
   the updated lockfile. Skipping this makes the very first `--locked` CI run
   fail with "lock file needs update".

6. **Dependabot** `.github/dependabot.yml`, scoped to **both** `cargo` **and**
   `github-actions` ecosystems (weekly). This is what keeps the exact pins
   (cargo deps *and* SHA-pinned actions) fresh — each update arrives as a
   reviewed PR, not an automatic merge, so it's controlled updates, not churn.

7. **Enforce the gate (repo setting, not a file).** Enable a branch-protection
   ruleset on `main` requiring the `fmt`, `clippy`, `test`, and `wasm` status
   checks to pass before merge (via repo Settings → Rules, or `gh api`). A
   workflow file only *runs* checks; without this, a red CI can still be merged
   and the "every PR is gated" claim is false. Requires the checks to have run
   once so their names are selectable.

8. **CONTRIBUTING note** (short): document the local pre-PR commands mirrored by
   CI (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked
   -- -D warnings`, `cargo test --workspace --locked`, and the wasm build).

## Tests / verification

- CI must pass on the scaffold as-is (it already builds + has one test).
- Locally run all CI commands and confirm green, including the wasm build
  (`rustup` auto-installs the `wasm32-unknown-unknown` target from
  `rust-toolchain.toml`).
- Confirm the lockfile regeneration: after the version bump, `cargo build
  --locked` must succeed (proves step 5 landed).
- One-time proof (not a standing test): introduce a wasm-incompatible construct
  in `ara-core` in a throwaway commit and confirm the `wasm` job fails, then
  revert. Note: a plain `wasm32-unknown-unknown` build is a *compile-time* guard
  only — some `std` APIs compile for wasm and only fail at runtime; runtime
  browser behaviour is covered later by Stage 3.

## Milestone / acceptance

Green CI on `main` with `fmt` + `clippy` + `test` + `wasm-build` jobs; toolchain
pinned exactly; `main` ruleset **requires** those checks. Every subsequent stage
PR is genuinely gated (enforced, not just signalled).

## CHANGELOG (Unreleased → Added)

- CI workflow (fmt, clippy, test, wasm-build) with pinned Rust toolchain
  (`1.94.1`), SHA-pinned actions, `--locked` builds, and Dependabot (cargo +
  github-actions).

---

## What already exists (reuse, don't rebuild)

- **`Cargo.lock` is already committed** at repo root (not gitignored) — so
  `--locked` works immediately; no new lockfile-generation work.
- **Local toolchain is already `rustc 1.94.1`** — the exact pin matches reality,
  no upgrade needed.
- **The scaffold already builds and has one test** (`ara-core::version_is_reported`)
  — CI is green on day one; this stage adds the gate, not the first test.
- **No `.github/` yet** — CI is greenfield; nothing to reconcile or migrate.

## NOT in scope (considered, deferred)

- **Artifact distribution** (crates.io publish, GitHub Releases for the `ara`
  binary) — first publish is `0.1.0` at the end of Stage 4 (`stage-overview.md`).
- **MSRV verification job** — deferred to the 0.1.0 release (`TODOS.md` → `T-MSRV`).
- **clippy on the wasm32 target** — no `#[cfg(target_arch="wasm32")]` code exists
  yet; deferred (`TODOS.md` → `T-WASM-CLIPPY`).
- **`docs/` directory + plan→docs migration** — deferred (`TODOS.md` → `T-DOCS`).
- **Reproducible-artifact / byte-for-byte output verification** — the
  determinism that matters (parse/layout snapshots) lands with the code in
  Stages 1–2; `--locked` + exact toolchain pin is the Stage-0-appropriate slice.
- **Permanent wasm-incompatibility regression harness** — the `wasm` build job
  itself is the standing guard; a dedicated compile-fail harness is
  over-engineered while the crates are near-empty.

## Failure modes (per CI guard)

| Guard | Realistic failure | Test? | Error handling | Visibility |
| ----- | ----------------- | ----- | -------------- | ---------- |
| `fmt` | unformatted code | yes (job) | job fails | clear red |
| `clippy -D warnings` | any lint | yes (job) | job fails | clear red |
| `test --locked` | stale `Cargo.lock` after bump | yes (`--locked`) | job fails "lock needs update" | clear red |
| `wasm` build | wasm-incompatible dep/API at compile time | yes (job) | build error | clear red |
| `wasm` build | `std` API that compiles for wasm but fails at *runtime* | no | none at this stage | **known limitation** — covered by Stage 3 browser tests |
| merge gate | red CI merged anyway | n/a | branch-protection ruleset (step 7) | closed by step 7 |

**No critical gaps remain** (a critical gap = no test AND no error handling AND
silent). The only silent-ish case (runtime-only wasm failure) has no runtime
codepath in Stage 0 and is explicitly deferred to Stage 3.

## Parallelization strategy

**Sequential implementation, no parallelization opportunity.** Every step is CI
/ config, most edits land in one `Cargo.toml` and one `ci.yml`, and the whole
stage is a single small PR. Splitting across worktrees would cost more
coordination than it saves.

## Implementation Tasks

Synthesized from this review's findings. Each derives from a specific finding.

- [ ] **T1 (P1, human: ~10min / CC: ~3min)** — CI — add `--locked` to
  clippy/test/wasm jobs; regenerate + commit `Cargo.lock` after the version bump.
  - Surfaced by: Architecture A1 + Codex (lockfile/version interaction).
  - Files: `.github/workflows/ci.yml`, `Cargo.lock`, `Cargo.toml`.
  - Verify: `cargo build --locked` green after bump; CI `test`/`wasm` pass.
- [ ] **T2 (P2, human: ~10min / CC: ~3min)** — Cargo manifests — remove root
  `rust-version` and `rust-version.workspace = true` from all 4 crates.
  - Surfaced by: Architecture A2 + Codex (inheritance breakage).
  - Files: `Cargo.toml`, `crates/*/Cargo.toml`.
  - Verify: `cargo metadata` / `cargo build` succeeds with no inheritance error.
- [ ] **T3 (P2, human: ~10min / CC: ~2min)** — ci.yml — add `on:`,
  `concurrency:` (cancel-in-progress), `permissions: contents: read`.
  - Surfaced by: Architecture A3.
  - Files: `.github/workflows/ci.yml`.
  - Verify: workflow runs on a PR; superseded run auto-cancels.
- [ ] **T4 (P2, human: ~2min / CC: ~1min)** — Cargo.toml — do NOT add
  `[workspace.lints]`; keep CI `-D warnings` as the sole lint gate.
  - Surfaced by: Code Quality CQ1 (inert table).
  - Files: `Cargo.toml` (omission), `.github/workflows/ci.yml`.
  - Verify: a deliberate `dbg!()` fails the `clippy` job.
- [ ] **T5 (P2, human: ~2min / CC: ~1min)** — rust-toolchain.toml — pin
  `channel = "1.94.1"` (exact).
  - Surfaced by: Outside voice OV1.
  - Files: `rust-toolchain.toml`.
  - Verify: `rustc --version` in CI reports `1.94.1`.
- [ ] **T6 (P2, human: ~2min / CC: ~1min)** — ci.yml wasm job — build
  `-p ara-core -p ara-wasm` for wasm32.
  - Surfaced by: Outside voice OV2.
  - Files: `.github/workflows/ci.yml`.
  - Verify: `wasm` job builds both crates green.
- [ ] **T7 (P2, human: ~15min / CC: ~4min)** — ci.yml + dependabot.yml —
  SHA-pin all actions, `runs-on: ubuntu-24.04`, Dependabot for cargo +
  github-actions.
  - Surfaced by: Outside voice OV3.
  - Files: `.github/workflows/ci.yml`, `.github/dependabot.yml`.
  - Verify: workflow uses SHAs; Dependabot config validates.
- [ ] **T8 (P2, human: ~5min / CC: n/a — GitHub UI)** — repo settings —
  branch-protection ruleset on `main` requiring the 4 checks.
  - Surfaced by: Outside voice OV4 (unenforced gate).
  - Files: none (GitHub setting; document in acceptance).
  - Verify: a PR with red CI cannot merge to `main`.
- [ ] **T9 (P3, human: ~5min / CC: ~2min)** — repo — `TODOS.md` created with
  the 3 deferred items (done this review).
  - Surfaced by: TODO triage.
  - Files: `TODOS.md`.
  - Verify: file present with T-MSRV, T-WASM-CLIPPY, T-DOCS.

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
|--------|---------|-----|------|--------|----------|
| CEO Review | `/plan-ceo-review` | Scope & strategy | 0 | — | not run (infra stage) |
| Eng Review | `/plan-eng-review` | Architecture & tests (required) | 1 | CLEAR | 8 issues, 0 critical gaps, all folded |
| Outside Voice | `/codex review` | Independent 2nd opinion | 1 | issues_found | 12 raised; 4 new decisions + 2 corrections folded |
| Design Review | `/plan-design-review` | UI/UX gaps | 0 | — | n/a (no UI) |
| DX Review | `/plan-devex-review` | Developer experience gaps | 0 | — | not run |

Issues resolved this review (8): A1 `--locked`, A2 drop MSRV, A3 workflow
`on`/`concurrency`/`permissions`, CQ1 drop inert `[workspace.lints]`, OV1 exact
`1.94.1` pin, OV2 wasm-build `ara-wasm` too, OV3 SHA-pin actions + fixed runner +
Dependabot scope, OV4 branch-protection gate. Plus 2 Codex correctness folds
(remove `rust-version.workspace` from all crates; regenerate `Cargo.lock` after
bump) and 3 tracked TODOs (T-MSRV, T-WASM-CLIPPY, T-DOCS).

- **CODEX:** outside voice extended the review — surfaced the floating toolchain
  pin, lockfile/version sequencing, unenforced merge gate, and ara-wasm coverage;
  no contradiction with earlier findings.
- **CROSS-MODEL:** no tension — both reviewers agree; Codex's items were additive.
- **VERDICT:** ENG CLEARED — ready to implement. No CEO/Design review needed
  (infra stage, no UI, no product-direction change).

NO UNRESOLVED DECISIONS
