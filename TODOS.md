# TODOS

Deferred work captured during reviews. Each item has enough context to pick up
cold. Remove an item when it lands.

## Deferred from Stage 0 eng review (2026-07-08)

### T-MSRV — MSRV verification job at the 0.1.0 release cut
- **What:** Re-add `rust-version` to `[workspace.package]` and add a CI job that
  builds/tests on that exact MSRV toolchain.
- **Why:** Stage 0 dropped the unverified `rust-version = "1.85"` claim (nothing
  tested it). Once ara-core is published and external crates can depend on it, an
  untested MSRV misleads consumers.
- **Context:** Belongs in the `0.0.5 → 0.1.0` release PR (see
  `plans/stage-overview.md`), which is the first crates.io publish.
- **Depends on:** Stage 4.

### T-WASM-CLIPPY — clippy the wasm32 target once cfg-gated code exists
- **What:** Add `cargo clippy --target wasm32-unknown-unknown` (ara-core,
  ara-wasm) to CI.
- **Why:** Native clippy skips `#[cfg(target_arch = "wasm32")]` branches, so
  wasm-only code can rot while CI stays green.
- **Context:** No cfg-gated code exists today (no-op now). Becomes relevant when
  Stage 3 adds `ara-wasm` bindings.
- **Depends on:** First `#[cfg(target_arch = "wasm32")]` usage in the codebase.

### T-DOCS — create docs/ and wire the plan→docs migration lifecycle
- **What:** Create a `docs/` directory and follow `CLAUDE.md`'s rule that a
  merged stage's plan is folded into `docs/` and removed from `plans/`.
- **Why:** `CLAUDE.md` and `stage-overview.md` both assume completed plans
  migrate to `docs/`, but no `docs/` exists yet — the lifecycle has nowhere to
  land.
- **Context:** Stage 0's own plan is the first migration candidate; do this when
  Stage 0 merges.
- **Depends on:** none.
