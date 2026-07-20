# Plan: Client-side markdown rendering of exhibit bodies (issue #32)

Tracking: [ARA-Labs/ara-cli#32](https://github.com/ARA-Labs/ara-cli/issues/32) ·
Design-of-record: `docs/hub-parity.md` (D4) · deferred sibling: D3 / `T-MATH-RENDER`.

## Problem background

The RESULT block of the viewer's detail pane lists the exhibits (figures/tables)
linked to a node, but renders **chips only** — exhibit id + kind label, with the
file/source as a hover tooltip (`crates/ara-viewer/src/detail.rs:544-565`). The
actual figure/table content is not shown.

The data to render it already exists. `ara-core` parses every
`evidence/figures/*.md` and `evidence/tables/*.md` into
`Manifest.exhibits: Vec<Exhibit>`, and each `Exhibit` carries its **raw GFM
markdown body verbatim** in `Exhibit.body: String` (`crates/ara-core/src/manifest.rs:222-236`).
Node→exhibit linkage is resolved into `Manifest.node_exhibits`. The viewer already
deserializes this manifest (`LoadState::Loaded(Manifest)`), so `body` is present in
the browser — the viewer's `ExhibitView` just deliberately drops it
(`detail.rs:45-60`), and there is no markdown renderer in the tree.

This was deferred (hub-parity D4) because a client-side markdown renderer adds weight
to the wasm bundle, which was in tension with the sub-MB bundle gate that also
deferred KaTeX (D3). **That tension turns out to be small in practice** (see the
bundle budget below): the current bundle is ~292 KB uncompressed / ~107 KB brotli
against budgets of 1 MB / 350 KB — roughly 756 KB / 243 KB of headroom.

## Goal / acceptance criteria (from #32)

1. Client-side markdown renderer for exhibit bodies (evaluated: a lean pure-Rust
   crate vs. a lazily-loaded JS lib).
2. **Gated on the wasm bundle-size check** — must stay under the CI `viewer-size`
   budgets (1 MB uncompressed / 350 KB brotli, `.github/workflows/ci.yml:88-89`).
   If it blows the gate, fall back to a core-side table AST.
3. RESULT tables render as real `<table>` for N07 (`fig3_scalability` +
   `figb1_memory_growth`).
4. Wide tables scroll horizontally **inside their block** (`overflow-x:auto`), not
   the whole page (Pass-5 responsive, <800px).

## Proposed solution

### Renderer choice — `pulldown-cmark`, tables subset

Use **`pulldown-cmark`** with only the extensions the corpus needs:
`Options::ENABLE_TABLES` (+ `ENABLE_STRIKETHROUGH`). Rationale:

- Lightest realistic pure-Rust option: 3 small deps (`bitflags`, `memchr`,
  `unicase`), pull parser, no AST. It is exactly the "tables-subset of
  `pulldown-cmark`" the issue names.
- comrak (GitHub-identical, arena AST) is heavier and buys GitHub-exact output we
  don't need; `markdown-rs` is a viable lighter alternative but pulldown-cmark is
  the ecosystem default and what the issue calls out. If the bundle measurement in
  step 3 fails, the fallbacks are (a) a lazily-loaded JS renderer, or (b) a
  core-side table AST — but with ~243 KB brotli of headroom this is unlikely.
- **Math stays inert, consistent with D3.** We do NOT enable the math extension, so
  any `$…$` in an exhibit body renders as literal text — the same "never
  interpreted" posture as `latex_view` today (`panels.rs:57-67`). No special
  handling needed.

### Rendering approach — DECIDED: (A) `inner_html` with raw-HTML neutralized

The viewer today escapes *everything*: there is no `set_inner_html` / `inner_html`
anywhere, all output is safe Leptos view nodes (the `latex-inert` span pattern is
the tell). This introduces the first, deliberate, documented exception.

**Approach (A):** feed `body` through `pulldown-cmark::html::push_html`, then mount
the resulting string via Leptos's `inner_html` attribute on a wrapper
`<div class="exhibit-body">`. pulldown-cmark escapes all text content; the only
injection vector is raw HTML embedded *in* the markdown (`Event::Html` /
`Event::InlineHtml`), which we **filter out or escape** before rendering — a
**required** part of the implementation, not optional — closing the XSS hole.
Exhibit bodies are trusted local ARA files anyway. ~15 lines.

**Why (A) over an event-walk into Leptos nodes (B):** A wins on the axes that
matter — maintainability (inherits pulldown-cmark's GFM correctness + fixes vs.
~150 hand-written lines to own), correctness-robustness (battle-tested/fuzzed HTML
renderer vs. hand-rolled gaps), and runtime speed (one native-parser DOM insertion
+ one wasm↔JS crossing vs. per-node `createElement` calls across the boundary). B's
only edge is preserving the no-`inner_html` invariant, which is a stylistic call,
not a functional one. **Decided: A** (2026-07-20).

## Implementation steps

### 1. Add the dependency — `crates/ara-viewer/Cargo.toml`
- Add `pulldown-cmark = { version = "0.13", default-features = false, features = ["html"] }`
  (`default-features = false` drops the `getopts`/CLI bits; keep only `html`).
- Confirm it compiles to `wasm32-unknown-unknown` (pure Rust, no SIMD in default
  build — fine for wasm).

### 2. New render module — `crates/ara-viewer/src/markdown.rs`
- `pub fn render_exhibit_body(md: &str) -> String` (approach A): build a
  `Parser::new_ext(md, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH)`,
  map `Event::Html`/`Event::InlineHtml` to escaped text (or drop), `push_html` into
  a `String`.
- Unit tests (native, `#[cfg(test)]`) here: a GFM table → contains `<table>`,
  `<th>`, `<td>`; a raw `<script>` in the body → escaped, not passed through; a
  `$x$` fragment → literal, no math markup.
- Wire into `src/lib.rs` module list.

### 3. Measure the bundle delta FIRST (the gate) — ✅ DONE, spike passed (2026-07-20)
- `cd crates/ara-viewer && TRUNK_BUILD_CARGO_PROFILE=wasm-release trunk build --release`
- `WASM=$(ls dist/*_bg.wasm|head -1); wc -c <"$WASM"; brotli -q 11 -c "$WASM"|wc -c`
- **Measured** (pulldown-cmark 0.13.4, tables+strikethrough, wired into the RESULT
  block so it's not dead-code-eliminated):

  | | baseline | with renderer | delta | budget | % of budget |
  |---|---|---|---|---|---|
  | uncompressed | 522,465 | **682,821** | +160,356 | 1,048,576 | **65.1% ✅** |
  | brotli q11 | 172,800 | **229,058** | +56,258 | 358,400 | **63.9% ✅** |

  Both PASS with ~365 KB / ~129 KB brotli headroom. Only 3 tiny transitive deps
  added (`bitflags`, `pulldown-cmark-escape`, `unicase`). The JS-lib / core-side-AST
  fallbacks are **not needed**. (Note: the docs' old ~292 KB/~107 KB figures were
  stale; 522 KB/173 KB is the real current baseline on this branch.)

### 4. Carry `body` into the viewer model — `crates/ara-viewer/src/detail.rs`
- Add `pub body: String` to `ExhibitView` (`detail.rs:51-60`) and populate it in
  `detail_model` from `ex.body.clone()` (`detail.rs:191-196`). Update the struct
  doc comment (drop "intentionally not carried").

### 5. Render in the RESULT block — `crates/ara-viewer/src/detail.rs:544-565`
- After each exhibit chip (or as an expandable body under the chip row), emit
  `<div class="exhibit-body" inner_html=render_exhibit_body(&ex.body)></div>`
  (approach A). Wrap each rendered body in a horizontal-scroll container so wide
  tables scroll inside their block, not the page.
- Update the block comment (`detail.rs:544-547`) — no longer "chips only".
- Consider empty-body guard: skip the body div when `ex.body` is blank.

### 6. Responsive CSS — `crates/ara-viewer/public/styles.css`
- The `table.md` styling already exists (`styles.css:1160-1177`) but pulldown-cmark
  emits a bare `<table>` with no `.md` class. Either (a) retarget the existing rules
  to `.exhibit-body table` / `th` / `td`, or (b) add class in the renderer. Prefer
  (a) — restyle to `.exhibit-body table`.
- Add the scroll container: `.exhibit-body { overflow-x: auto; max-width: 100%; }`
  (mirrors the `pre.diff` overflow pattern at `styles.css:1188`). Verify the page
  body itself never scrolls horizontally at <800px.

### 7. Web tests — `crates/ara-viewer/tests/web.rs`
- Extend the panel fixture (`manifest_with_panels`) with an exhibit whose `body` is a
  GFM table + node→exhibit edge, then assert the rendered detail contains a real
  `<table>` / `<th>` / cell text (headless wasm-bindgen, matches existing style).
- Assert a wide-table wrapper has the scroll container.

### 8. Regenerate the embedded bundle — `scripts/embed-viewer.sh`
- Run `scripts/embed-viewer.sh` to refresh `crates/ara-cli/assets/viewer/` +
  `viewer.source-hash`, so the `viewer-embed-fresh` CI job stays green (viewer
  source changed).

### 9. Version + changelog + docs
- Functional change → bump patch in `Cargo.toml` (`[workspace.package] version`),
  add a `CHANGELOG.md` entry under `## [Unreleased]` → `### Added`.
- After merge: fold this plan into `docs/` (extend `docs/hub-parity.md` / the viewer
  docs) — record the renderer choice, the measured bundle delta, and the D4→shipped
  status flip — then remove it from `plans/` (per `CLAUDE.md` workflow).

## Verification

- `cargo test -p ara-core -p ara-viewer` (unit + web tests).
- The step-3 bundle measurement under budget (the acceptance gate).
- `cargo run -- serve` on an artifact with N07 (`fig3_scalability`,
  `figb1_memory_growth`): confirm both render as real tables, and a wide table
  scrolls inside its block at a narrow viewport.
- `scripts/embed-viewer.sh --check` passes (bundle fresh).

## Risks / open questions

1. **Rendering approach A vs B** (`inner_html` vs event-walk) — the one decision
   needing sign-off before coding. Default: A.
2. **Bundle gate** — measured in step 3 before any UI work; hard stop + fallback if
   it fails. Expected comfortably green given current headroom.
3. **Body placement/UX** — always-visible under the chip, vs. click-to-expand.
   Default: always-visible with the scroll container; revisit if the RESULT block
   gets tall on multi-exhibit nodes.
4. Figure *images* (`T-HUB-FIGURES`) are out of scope — corpus is markdown tables,
   near-zero image files (`docs/hub-parity.md`).
