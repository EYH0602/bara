# Stage 0 — CI & Tooling Foundation

Design record for the CI gate and pinned toolchain shipped in Stage 0 (PR #1,
workspace `0.0.1`). This stage adds no product logic; it establishes the single
green/red signal that validates every later stage the same way.

## Why a gate before any real code

`ara-core` (and `ara-wasm`) must compile to `wasm32-unknown-unknown` — one crate,
native + wasm is the core architectural promise. Catching a wasm-incompatible
dependency by hand in a later stage is expensive; CI catches it at compile time.
The other jobs (fmt, clippy, native tests) keep every stage PR uniformly clean.

## Pinned, reproducible toolchain

`rust-toolchain.toml` pins the channel **exactly** (`1.94.1`, not the floating
`1.94`) plus `rustfmt`/`clippy` components and the `wasm32-unknown-unknown`
target. An exact patch pin matters because a rustc patch bump can silently change
codegen or clippy output, which would break the byte-determinism invariant the
parse/layout stages depend on. `rustfmt.toml` sets `max_width = 100`,
`edition = "2024"`.

No `[workspace.lints]` table exists: CI's `cargo clippy -- -D warnings` is the
single lint-enforcement point (a `[workspace.lints]` table is inert unless every
crate also opts in with `[lints] workspace = true`, so it would be dead config).

The unverified `rust-version = "1.85"` MSRV was removed from `[workspace.package]`
and all four crate manifests — an MSRV that CI never builds is a claim that
drifts. (Re-adding + testing it is tracked as `T-MSRV`.)

## The workflow

`.github/workflows/ci.yml` runs on `pull_request` and pushes to `main`, with
`concurrency` cancel-in-progress (a new push doesn't waste minutes finishing an
outdated run) and least-privilege `permissions: contents: read`. All jobs run on
`ubuntu-24.04`, cache via `Swatinem/rust-cache`, and SHA-pin every action.

| Job | Command |
| --- | ------- |
| `fmt` | `cargo fmt --all --check` |
| `clippy` | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| `test` | `cargo test --workspace --locked` |
| `wasm` | `cargo build -p ara-core -p ara-wasm --target wasm32-unknown-unknown --locked` |

`--locked` on every building job is load-bearing: it fails CI if `Cargo.lock` is
stale, which is exactly the determinism guard this stage exists to provide. After
the version bump, `Cargo.lock` is regenerated with a plain `cargo build` and
committed, so the first `--locked` run doesn't fail with "lock file needs update".

The `wasm` build guards both the shared core and the browser-facing cdylib. It is
a **compile-time** guard only: some `std` APIs compile for wasm and fail only at
runtime — runtime browser behaviour is covered later by the Stage 3 browser tests.

## Dependency freshness and merge enforcement

`.github/dependabot.yml` covers **both** the `cargo` and `github-actions`
ecosystems (weekly), keeping the exact pins (cargo deps *and* SHA-pinned actions)
fresh as reviewed PRs rather than churn. A branch-protection ruleset on `main`
requires the `fmt`, `clippy`, `test`, and `wasm` checks to pass before merge — a
workflow file only *runs* checks; without the ruleset a red CI could still merge.

## Acceptance (met)

Green CI on `main` with the four jobs; toolchain pinned exactly; `main` ruleset
requires those checks, so every subsequent stage PR is genuinely gated (enforced,
not just signalled).

## Deferred

Artifact distribution (crates.io publish, GitHub Releases) — first publish is
`0.1.0`; MSRV verification job (`T-MSRV`); clippy on the wasm32 target
(`T-WASM-CLIPPY`, no `#[cfg(target_arch="wasm32")]` code exists yet); a permanent
wasm-incompatibility regression harness (the `wasm` build job is the standing
guard while the crates are near-empty).
