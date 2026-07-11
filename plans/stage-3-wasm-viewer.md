# Stage 3 — Leptos wasm Viewer, skinned to the published ARA design (static manifest)

**PR target:** `stage3-wasm-viewer` → `main`. **Depends on:** Stage 2.
**Version bump:** `0.0.4 → 0.0.5` (Stage 2 already took `0.0.4`).

> **Cross-stage flags (eng review):** the Stage 4 plan also claims `0.0.4 → 0.0.5`
> — if Stage 3 takes `0.0.5`, Stage 4 becomes `0.0.5 → 0.0.6` (fix the Stage 4
> plan). And the `0.1.0` release publishes `ara-core → ara-wasm → ara-cli` but not
> `ara-viewer`'s generated frontend assets: `cargo install ara-cli` cannot serve a
> Trunk `dist/` unless the release embeds/copies it. Resolve in Stage 4 / the
> release cut — captured as `T-VIEWER-DIST-PACKAGING`.

## Design reference (the North Star)

The viewer must **look and behave like the already-published ARA viewer**, not a
new visual direction. The canonical reference is the `research-visualizer`
scaffold shipped in `ARA-Labs/ARA-Demo` (`nanogpt_ara/trajectory.html`,
`arc-agi3/ls20/trajectory.html`) — a **FIXED** rendering scaffold whose design is
the published standard. Its design language:

- **Warm-cream light theme, single terracotta accent; node type is read from a
  glyph + label, never from colour.** Tokens (copy verbatim):
  `--bg:#faf6ef --panel:#fffdf8 --panel2:#f4ecdf --line:#e6ddcc --ink:#2f2a23`
  `--muted:#90856f --accent:#bf6a2e --chip:#f1e9da --ok:#5d7c3f --warn:#a23b2d`
  `--glyph-bg:#e7ddca --glyph-ink:#5a5142 --sel-bg:#f7ead2`.
  Body `ui-sans-serif`, ids/code `ui-monospace`. **No default font stack, no new
  palette** — the cream theme is the established brand, so it passes the AI-slop
  bar by construction.
- **Two-pane `<main>` grid `minmax(320px,38%) 1fr`:** left `#map` (exploration
  tree) + right `#detail` (drill-down). A `<header>` carries title, subtitle, and
  toolbar.
- **Kind encoding:** a lettered glyph chip + a lowercase text badge. All kinds use
  the neutral `--glyph-bg`; **only `dead_end` is coloured** (`--warn`, white ink).
  This is colourblind-safe by construction.
- **Detail pane** = collapsible `.block`s, a primary `.block.reason` with an
  accent left-spine, `.claim` cards with supported/refuted/hypothesis status
  pills, `.quote`/`figure`/`table.md`/`pre.diff`, provenance `.chip`s, and
  `support_level` pills. Every richer layer is **inert unless its data is
  present**.
- **Toolbar:** search, type filter, "dead ends only" checkbox. Selectable text
  throughout; Esc-closable overlays.

## Problem background

The product is a text-heavy drill-down viewer: prose, structured fields, claims,
and (as the schema widens) quotes/figures/tables — all selectable, searchable,
accessible. A DOM framework (Leptos) delivers this at sub-megabyte wasm cost;
egui/eframe cannot (canvas, no native selection/search/a11y, multi-MB bundle) and
stays a documented fallback only. This stage renders a **checked-in static
`manifest.json`** (from Stage 2) — no server yet — proving the render path end to
end, in the published visual language.

## Proposed solution — hybrid: SVG graph, published skin

A Leptos 0.8 CSR app built into **`crates/ara-viewer`** (repurposing the reserved
umbrella placeholder — its `main.rs` install-message stub is replaced by the wasm
CSR entry; fits `members = ["crates/*"]` with no workspace change) that:

- **Left pane (`#map`):** renders the DAG as declarative **SVG** from Stage-2
  `Node.pos` + `Manifest.bounds`, **skinned to the published tokens**: each node is
  a `180×60` rect carrying a lettered glyph + `label ?? id`; `dead_end` is red;
  `Child` edges solid, `DependsOn` edges dashed; pan/zoom via `viewBox`. This keeps
  the graph-visualisation direction and consumes Stage-2's layout, but adopts the
  published look. **Validated by an early spike (step 3a) before the full
  interaction layer is built.** (A faithful DOM tree-list is the documented pivot —
  see NOT in scope.)
- **Right pane (`#detail`):** the published detail-pane structure, wired to the
  fields `ara-core` emits today and degrading gracefully for the rest.

Reuse `ara-core` types over the wire via `serde_json` (no hand-written TS; the
speculative `ara-wasm` dep is **dropped** — the crate is an empty skeleton and the
viewer needs no hand-written interop).

**Two seams keep the stage honest and Stage-4-ready:**

- **`GraphRenderer` = pure scene-model trait**, NOT view-returning (a
  view-returning trait can't abstract SVG vs canvas — they share no
  selection/focus/ARIA/lifecycle — and devolves to `AnyView` boxing). The trait
  computes a `GraphScene { nodes:[{id, rect, kind, glyph}], edges:[{path,
  link_kind}], bounds }` from the `Manifest`; `SvgRenderer` renders that scene to
  SVG; a future `CanvasRenderer` renders the same scene to canvas. Scene compute is
  **pure → native unit-testable** (no browser).
- **`ManifestSource` seam** so Stage 4 slots in without a rewrite: `Static(url)`
  (fetch `manifest.json`) now; `Api(endpoint, live)` in Stage 4. An
  `apply_manifest(new)` reducer swaps the manifest while **preserving selection +
  pan/zoom** (makes the step-6 view-state-survival promise concrete).

**Schema is authoritative.** All rendering binds to the frozen Stage-2 `Manifest`
(`crates/ara-core/src/manifest.rs`). CSS classes bind to the serde **snake_case
wire form** (`dead_end`, `depends_on`, `question`, …), not Rust identifiers, via a
single `kind_meta(&NodeKind) -> { wire, glyph, label }` source of truth (used by
the SVG glyph, the detail badge, the type-filter dropdown, and search). For
`NodeKind::Other(raw)` the CSS class is a **fixed sanitized token** (`other`) — the
raw string is display text only, never a class name.

## Implementation steps

1. **`crates/ara-viewer` crate** (Leptos CSR, Trunk). Replace the placeholder
   `main.rs` with the wasm CSR entry. `Cargo.toml` depends on `ara-core`
   (default-features off; wasm-safe). **Do not** add `ara-wasm` (dropped). Add
   `index.html`, `Trunk.toml`, and `crates/ara-viewer/dist` to `.gitignore`. Vendor
   the published token set into the stylesheet. Define `kind_meta(&NodeKind)` as the
   single source of truth for wire string / glyph / label / css class.
2. **Load manifest via `ManifestSource` + states.** Introduce `ManifestSource`
   with a `Static(url)` variant that fetches the checked-in `manifest.json` (under
   `crates/ara-viewer/public/`; state the exact fetch URL and reconcile Trunk's
   `public/`→`dist/` copy) → `serde_json::from_str::<Manifest>` → Leptos signal.
   `Api(endpoint, live)` is Stage 4. An `apply_manifest(new)` reducer replaces the
   manifest while preserving selection + pan/zoom. Handle every state in the
   **Interaction states** table — the real failure to guard is the fetch/parse
   path, not any LLM call.
3. **Scene model + `GraphRenderer` trait.** `GraphRenderer::scene(&Manifest, &LayoutView) -> GraphScene`
   computes `GraphScene { nodes:[{id, rect (from `pos`, fixed `180×60`), kind,
   glyph}], edges:[{path, link_kind}], bounds }`. **Edge paths are derived
   client-side** from a `NodeId → pos` map over endpoints (there is no `Link.route`
   in the schema — routing is deferred to the client per `docs/manifest-schema.md`).
   Scene compute is **pure and native-unit-tested**. `SvgRenderer` renders the
   scene; a `CanvasRenderer` **stub** (web-sys `CanvasRenderingContext2d` off a
   `NodeRef`) is left for the swap if the fps probe (step 8) demands it — canvas is
   not built this stage.
   - **3a. SVG spike gate (do BEFORE step 3b).** Render the largest corpus tree as
     static skinned SVG; confirm it reads well and clears the fps bar. **If it
     fails, pivot to the published DOM tree-list now** (cheap) rather than after
     building the interaction layer. Record the result in `docs/`.
   - **3b. Interaction layer (only after 3a passes).** Per node: glyph chip + `label
     ?? id` clamped to 2 lines + ellipsis, full text in `<title>`; kind badge; only
     `dead_end` coloured (`--warn`) + strikethrough title; `Other(raw)` = neutral
     glyph + raw-kind badge (fixed `other` class). `Child` edges solid, `DependsOn`
     dashed. Selected (`--sel-bg` + accent border) and **focus** (distinct ring)
     states. Pan/zoom via `viewBox`; per-element hit-testing. a11y: node `<g>`
     focusable (`tabindex`, `role="button"`, `aria-label = (label ?? id) + kind`),
     Enter/Space selects.
4. **Drill-down pane (DOM), published structure, per-kind hierarchy.** On select,
   render (top→bottom):
   1. **Header** — `label ?? id`, kind badge, `support_level` pill
      (explicit/inferred), raw kind string when `Other`.
   2. **Description** — `description` prose (selectable). Omit if `None`.
   3. **Typed fields** — per `NodeKind`, in kind-specific order:
      `Question` → none; `Experiment` → `result`;
      `Decision` → `choice` → `rationale` → `alternatives[]`;
      `DeadEnd` → `why_failed` (**leads** — it is what the user came for);
      `Insight` → none; `Other` → none.
   4. **Evidence** — `evidence_notes` list, then bound claims:
      `bindings.filter(node == selected)` → look up `Claim` by id → render
      `title` + `statement` + `status` (supported/refuted/hypothesis pill).
   5. **Provenance** — `source_refs` as muted `.chip`s.
   - Empty node (no populated fields) → `"Nothing recorded for this node."`
   - **Inert-until-present** richer blocks (published CSS reused, render only when
     data exists): `.quote`, `figure>img`, `table.md`, `pre.diff`, glossary,
     dependencies, recipes. These have no backing in today's schema (the deferred
     `T-REAL-CORPUS` widening) and stay dark until those fields land — no rework
     when they do. **Never call an LLM at view time.**
5. **Toolbar + view-state.** Header toolbar: search (by label/id/kind/claim text),
   type filter `<select>`, "dead ends only" checkbox. Keep pan/zoom + selection +
   filter in signals (reused by Stage 4 live reload to survive manifest refresh).
   Selecting a node in the graph and via search/filter stays in sync with the
   detail pane.
6. **A11y + responsive (full parity).** Toolbar search/filter is the primary
   keyboard nav; focusable SVG nodes give Enter/Space select + a visible focus
   ring distinct from selection; the DOM detail pane is natively selectable; ARIA
   landmarks on `#map`/`#detail`. Two-pane grid stacks/toggles on narrow
   viewports. (Arrow-key spatial graph traversal is a follow-up TODO, not this
   stage.)
7. **App header.** Title + subtitle matching the published `<header>` (abstract
   block inert until the schema carries it).
8. **SVG-vs-canvas decision.** Measure pan/zoom fps on the largest corpus tree
   (Stage-2 scale probe). Record in `docs/`; switch renderer only if fps < ~30 or
   DOM element count exceeds a few thousand. (The step-3a spike is the *early*
   readability/fps check; this is the *post-build* measurement.)
9. **Wasm size budget (enforced).** Add a `profile.wasm-release` (`opt-level="z"`,
   `lto="fat"`, `codegen-units=1`, `panic="abort"` as feasible), run `wasm-opt -Oz`
   in the Trunk build, and add a **CI gate** that fails if the bundle exceeds the
   budget (e.g. <1 MB uncompressed, <350 KB brotli). Turns "sub-MB" into an
   enforced contract, not a hope.

## Visual design system (from the published scaffold)

| Token | Value | Use |
|-------|-------|-----|
| `--bg` | `#faf6ef` | app background |
| `--panel` / `--panel2` | `#fffdf8` / `#f4ecdf` | surfaces |
| `--ink` / `--muted` | `#2f2a23` / `#90856f` | text / secondary text |
| `--accent` | `#bf6a2e` | single accent (selection, spines, links) |
| `--warn` / `--ok` | `#a23b2d` / `#5d7c3f` | dead_end / positive status |
| `--glyph-bg` / `--glyph-ink` | `#e7ddca` / `#5a5142` | neutral kind glyph |
| `--sel-bg` | `#f7ead2` | selected node fill |

Fonts: `ui-sans-serif` (body), `ui-monospace` (ids/code). Kind = glyph + lowercase
badge; colour only for `dead_end`.

## Interaction states (all specified, all tested)

| Surface | State | User sees |
|---------|-------|-----------|
| manifest fetch | loading | `"Loading artifact…"` skeleton; empty map |
| manifest fetch | load failure (404 / parse err) | error card: `"Couldn't load manifest"` + reason |
| graph | empty (`nodes: []`) | `"No nodes in this artifact."` + safe `viewBox` (no divide-by-zero when `bounds` is `None`) |
| node | `pos: None` (layout not run) | skip-with-warning; never panic |
| detail | no selection | `"Select a step on the left."` |
| detail | node with no fields | `"Nothing recorded for this node."` |
| node | `Other`/unknown kind | neutral glyph + raw-kind badge |

## Tests / verification

Two layers (eng-review decision): pure logic in native `cargo test`, DOM
integration in a thin headless-browser layer.

**Native `#[cfg(test)]` unit tests (no browser, run in `cargo test`):**
- **`kind_meta`** — every `NodeKind` (incl. `Other(raw)`) maps to the expected
  wire/glyph/label; `Other` → fixed `other` CSS token, raw string is display only.
- **Scene compute / edge derive** — `GraphRenderer::scene` builds a `NodeId → pos`
  map and derives an edge path per link; `Child`→solid, `DependsOn`→dashed; a link
  whose endpoint has `pos: None` is skipped, not panicked.
- **Manifest deserialize round-trip** — the checked-in `manifest.json` parses into
  `Manifest` (schema-drift guard; the file is produced by `ara layout --json`,
  same serde path as `serde_json::to_string_pretty`).
- **State selection** — `(loading / ok / load-failure / empty-nodes / pos-None)`
  each maps to the specified surface; empty `nodes` + `bounds: None` yields a safe
  `viewBox` (no divide-by-zero).
- **Search/filter predicate** — search + type + dead-ends-only filter the node set
  as expected (predicate is pure).

**Thin `wasm-bindgen-test` headless-browser layer (needs chromedriver — add the CI
job in this PR):**
- N nodes with `pos` → N node elements; `dead_end` nodes carry the highlight class
  + strikethrough; `DependsOn` edges carry the dashed class.
- Click node → detail pane shows that node's content; **search-select syncs the
  graph highlight**.
- **Detail hierarchy:** a `Decision` renders `choice`→`rationale`→`alternatives`; a
  `DeadEnd` renders `why_failed` first; bound claims resolve through `bindings` and
  show their status pill.
- **Degradation:** a node with only `id`/`kind` renders `"Nothing recorded"`, no
  error; a node with no `description` renders structured fields only.
- **A11y:** node `<g>` is focusable and selectable via Enter/Space; `aria-label`
  present.

**Build/CI:** `trunk build --release` succeeds; `wasm-opt -Oz` runs; the **CI size
gate** fails if the bundle exceeds budget (<1 MB uncompressed, <350 KB brotli). Add
a CI job that installs chromedriver and runs the wasm-bindgen-test layer.

## Milestone / acceptance

`trunk serve` (or `trunk build --release` + a static server — **not** `open
index.html`; a CSR app's `fetch` needs an HTTP context, not `file://`), then
navigate a real tree in the **published visual language**: pan/zoom the skinned SVG
graph, click/keyboard-select nodes, read the selectable per-kind drill-down
(description, typed fields, claims with status, provenance). Dead ends are
unmistakable without relying on colour. Every load/empty/error state is handled.
Spike (3a) result + renderer choice + bundle size documented.

## NOT in scope (design decisions considered and deferred)

- **Faithful DOM tree-list left pane** (the published viewer's exact approach) —
  the project keeps its SVG-DAG direction (hybrid), skinned to match. It is the
  **documented pivot** if the step-3a spike fails.
- **Richer detail blocks with real data** (quotes/figures/tables/diffs/glossary/
  recipes) — structure adopted now but inert; real data is the deferred
  `T-REAL-CORPUS` schema widening, not a Stage-3 change.
- **Arrow-key spatial graph traversal** — search/filter + Tab is the keyboard nav
  this stage; spatial node-to-node arrow walking is a follow-up TODO
  (`T-GRAPH-KBD-NAV`).
- **Canvas renderer** — stub only; built only if the fps probe demands it. The
  scene-model trait makes it a real seam (SVG/canvas both render one `GraphScene`).
- **`ara-viewer` frontend distribution** — how `dist/` reaches users
  (`cargo install ara-cli` can't serve generated assets) is a `0.1.0`
  release/Stage-4 concern (`T-VIEWER-DIST-PACKAGING`), not Stage 3.
- Any server/HTTP, live reload, figure streaming from disk — Stage 4 (Stage 3 adds
  only the `ManifestSource::Static` seam so Stage 4 slots in without a rewrite).

## What already exists (reused, not rebuilt)

- **Published `research-visualizer` scaffold** (`ARA-Labs/ARA-Demo`) — the design
  system, token set, glyph/label kind encoding, detail-pane structure, and
  graceful-degradation philosophy are copied, not invented.
- **Stage-2 layout** — `Node.pos` + `Manifest.bounds` drive the SVG graph;
  `ara layout --json` produces the checked-in manifest (same serde path the client
  reads back, so the round-trip is guaranteed by construction).
- **Frozen `Manifest`** (`ara-core`) — the single wire type; classes bind to its
  serde snake_case form.
- **`crates/ara-viewer` placeholder** — the reserved umbrella crate (currently a
  stub binary) is repurposed as the client, rather than adding a new crate.
- **`ara-wasm`** — considered for interop; **dropped** (empty skeleton, no interop
  site). `TODOS.md` `T-WASM-CLIPPY` stays dormant (no wasm cfg-gated code added).

## CHANGELOG (Unreleased → Added)

- Leptos CSR client (`crates/ara-viewer`): SVG DAG **skinned to the published ARA
  design** (warm-cream theme, glyph+label node kinds, dead-end highlighting) from
  Stage-2 positions via a pure scene-model `GraphRenderer`, with pan/zoom,
  keyboard-accessible nodes, a published-style drill-down pane (per-kind field
  hierarchy, claims with status, graceful degradation), toolbar
  search/type/dead-end filters, full loading/empty/error states, and an enforced
  sub-MB wasm size gate — from a static manifest via a `ManifestSource` seam.

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
|--------|---------|-----|------|--------|----------|
| CEO Review | `/plan-ceo-review` | Scope & strategy | 0 | — | — |
| Codex Review | `/codex review` | Independent 2nd opinion | 0 | — | — |
| Eng Review | `/plan-eng-review` | Architecture & tests (required) | 1 | CLEAR (PLAN) | 3 issues + 4 outside-voice tensions, 0 critical gaps, all folded |
| Design Review | `/plan-design-review` | UI/UX gaps | 1 | ISSUES ADDRESSED (FULL) | score 3/10 → 8/10, 7 decisions |
| DX Review | `/plan-devex-review` | Developer experience gaps | 0 | — | — |

- **CODEX:** two outside-voice passes ran. **Design** (Codex `gpt-5.5` + Claude
  subagent): both returned reject-as-written — the UI content model did not match
  the frozen `Manifest` (`narrative`/`img`/`table` absent, `Link.route` not a
  field, `pivot` invented + `Other` unhandled, colour-only dead-ends, no states,
  thin a11y; subagent added unrendered claim `bindings` and `pos`/`bounds`
  `Option`). **Eng** (Codex `gpt-5.5`): found packaging/sequencing gaps the review
  missed — frontend `dist` distribution vs `cargo install ara-cli`, Stage 3/4
  version collision, `open index.html` wrong for a CSR fetch, sub-MB asserted not
  engineered, Stage-4 manifest-source rewrite risk, `GraphRenderer` a fake
  abstraction, `Other(String)` class sanitization. All folded or flagged.
- **CROSS-MODEL:** design — both models independently flagged the same schema
  mismatches; the decisive input was the human's "match the published viewer",
  which reframed the plan via `ARA-Labs/ARA-Demo`. Eng — four tensions resolved:
  early SVG spike gate before the interaction layer, `GraphRenderer` reshaped to a
  **pure scene-model** trait, a `ManifestSource` seam for Stage-4, and a wasm
  size profile + CI gate. Scope decisions accepted: client in `crates/ara-viewer`,
  `ara-wasm` dropped, tests split native/browser + chromedriver CI.
- **VERDICT:** DESIGN + ENG CLEARED — ready to implement. Plan rewritten to the
  published design language and hardened for architecture/tests/packaging; version
  `0.0.4 → 0.0.5`; `T-DESIGN-TOKENS`, `T-GRAPH-KBD-NAV`, `T-VIEWER-DIST-PACKAGING`
  in `TODOS.md`.

NO UNRESOLVED DECISIONS
