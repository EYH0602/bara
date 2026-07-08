# bara

Blazing-fast **ARA viewer** — a Rust runtime that parses, serves, and renders
Agent-Native Research Artifacts (ARAs) as an interactive, drill-down DAG in the
browser. (`bara` = *blazing ara*.)

> Status: early scaffold. Crate names are reserved on crates.io at `0.0.0`;
> real functionality starts at `0.1.0`. See `plans/ara-runtime-impl-plan.md`.

## Workspace

| Crate         | Kind        | Role                                                         |
| ------------- | ----------- | ------------------------------------------------------------ |
| `ara-core`    | lib         | Shared parse + normalize + layout; builds native **and** wasm |
| `ara-cli`     | bin (`ara`) | Command-line runtime (`ara validate`, `ara serve`)            |
| `ara-wasm`    | cdylib/rlib | `wasm-bindgen` interop for the Leptos browser client         |
| `ara-viewer`  | bin         | Reserved umbrella / front-door name                          |

## Install

```bash
cargo install ara-cli   # ships the `ara` binary
ara --help
```

## Build

```bash
cargo build --workspace
cargo test --workspace
cargo run -p ara-cli
```

## Reserved crate names

The `ara-*` names above are the working crates. The `bara-*` names
(`bara-core`, `bara-cli`, `bara-wasm`, `bara-viewer`) are reserved defensively
and redirect here.

## License

[MPL-2.0](LICENSE).
