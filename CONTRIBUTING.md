# Contributing

## Local checks before opening a PR

CI runs these exact commands; run them locally first to get a green PR on the
first try. The pinned toolchain (`rust-toolchain.toml`) and the
`wasm32-unknown-unknown` target are installed automatically by `rustup` on the
first `cargo` invocation.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build -p ara-core -p ara-wasm --target wasm32-unknown-unknown --locked
```

Notes:

- **`--locked`** makes CI fail if `Cargo.lock` is out of date. After bumping a
  crate version or changing a dependency, run a plain `cargo build` to refresh
  `Cargo.lock` and commit it in the same PR.
- **Lints:** clippy warnings are errors in CI (`-D warnings`). Local `cargo
  clippy` without `-D warnings` only warns, so rely on the command above.
- **wasm:** `ara-core` and `ara-wasm` must build for `wasm32-unknown-unknown`
  (the browser path). Keep them free of OS-only APIs.

## Versioning

Every PR bumps the workspace patch version in `Cargo.toml` and adds an entry to
`CHANGELOG.md` under `## [Unreleased]`. See `CLAUDE.md` for the full policy.
