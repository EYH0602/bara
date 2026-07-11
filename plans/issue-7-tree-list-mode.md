# Issue #7 — Viewer: DOM tree-list as an alternate display mode + replay stepper

## Problem background

The Stage-3 viewer (`crates/ara-viewer`) renders the exploration graph as an
**SVG DAG** with pan/zoom (`GraphView` + the pure `scene.rs` model). The
published reference — `ARA-Labs/ARA-Demo`'s `research-visualizer` scaffold
(`nanogpt_ara/trajectory.html`) — instead renders a **DOM indented tree-list**
and ships a **replay stepper** and **layer-panel overlays** we don't have.

Stage 3 deliberately chose the SVG-DAG hybrid (eng + design reviewed) and named
the DOM tree-list as the documented pivot. Issue #7's decision: **keep the SVG
graph as the default, and add the published DOM tree-list as an alternate
display mode** (a Graph ⇄ Tree toggle) plus the replay stepper, so the viewer
can match the published ARA interaction/display when desired. This is
**additive** — the SVG graph and the Stage-2 layout stay untouched.

### Fidelity mandate (human dev)

> "Tree mode should render the tree the same way as the current static artifact."

The tree mode is **not a new design** — it must reproduce the published
`research-visualizer` scaffold (`ARA-Labs/ARA-Demo` → `nanogpt_ara/trajectory.html`)
pixel-for-pixel and interaction-for-interaction. This plan was audited directly
against that file (4964 lines; the tree render lives in `renderMap` / `nodeRow` /
`renderSubtree`, the replay in `step` / `play` / `stop`, the filter in
`applyFilters` / `rstat`). Every class name, glyph, marker, spacing token, and
control is fixed by the reference; where this plan and the reference disagreed,
**the reference wins**. Concrete deltas found during the audit are folded into
the sections below and called out in the review report.

**Reference ground truth (verbatim from `trajectory.html`):**

- Node row (`nodeRow`): `<div class="node [dead]" data-id data-type>` →
  `<span class="glyph {type}">{glyph}</span>` + a wrapper `<span>` holding
  `<span class="meta"><span class="nid">{id}</span>[<span class="dep">⇠ {ids}</span>]</span>`
  then `<div class="ntitle">{title||body||"(untitled)"}</div>`. Note the classes
  are **`.dead`** (not `.node.dead_end`), **`.sel`** (not `.selected`),
  **`.dim`** (matches), **`.glyph`** (not `.chip`), and the dep marker is
  **`.dep`** (not `.dep-marker`).
- Nesting (`renderSubtree`): children go in a sibling `<div class="kid">`, not
  inside the parent row.
- Isolation (`renderMap`): roots are split by the node's own boolean
  **`isolated`** field — `normalRoots` render at top level, `isoRoots` render
  inside one `<div class="isobox"><div class="isohdr">isolated subtree</div>…`.
- Glyphs (`GLYPH`): `question:"Q"`, `experiment:"✦"`, `decision:"→"`,
  `dead_end:"✗"`, `insight:"!"`, plus `pivot:"↻"` and `default:"•"`.
  **These differ from our SVG viewer's `kind_meta` glyphs** (`E`/`D`/`X`/`I`).
- Dep marker text: `⇠ {comma-joined ids}` with `title="depends on {ids}"`.
- Replay interval: **1300 ms** (not 1.1 s). Buttons: `‹` / `▶ Replay`⇄`⏸ Pause`
  / `›`. Prev/next call `stop()` first. Arrow keys guarded by
  `if(e.target.tagName==="INPUT"||e.target.tagName==="SELECT") return;`.
- Step count (`rstat` / `applyFilters`): the **same** `#rstat` span shows either
  `"{shown} / {N} steps"` (while filtering) or `"step {i+1} / {N}"` (when a node
  is selected). It is not tree-only chrome — it lives in the toolbar and is
  shared by filter + replay.
- Traversal `order`: `DATA.order` if present, else DFS from roots. Our manifest
  has no `order`, so DFS-from-roots — which equals `manifest.nodes` order only
  when nodes are already pre-order DFS (they are, per the manifest contract).

### Scope decision (confirmed with human dev)

- **This PR ships parts 1–3** of issue #7: the display-mode toggle, the DOM
  tree-list mode, and the replay stepper. All three are user-visible and
  testable against today's `Manifest`.
- **Part 4 (layer panels + abstract) is deferred** to the `T-REAL-CORPUS` PR
  that actually widens the schema to carry context / glossary / dependencies /
  recipes / abstract. There is nothing to render inertly that isn't already a
  no-op today, so we do not add dead layer-panel chrome now. The reference
  tokens part 4 needs (`--code-bg --reason-bg --iso-*` etc.) are added only as
  far as the tree-list itself uses them (`--iso-*`); the diff/scrim/shadow
  tokens land with part 4.
- **Tree CSS classes** use the **published reference names** (`.node`, `.kid`,
  `.nid`, `.ntitle`, `.isobox`, `.deptarget`, `.dim`) but are **scoped under a
  `.tree-map` container** so they never collide with the SVG graph's existing
  `.graph-svg .node` / `.node.dimmed` rules.

## Reuse (already built, display-agnostic — carries over unchanged)

`kind::kind_meta`, `detail.rs` (`DetailPane` + `detail_model`), the
`filter::node_matches` predicate, and the shared `selected` / `filter` /
`pan_zoom` / `layout` signals in `App`. The pure `scene.rs` model stays for
Graph mode. `ManifestSource` and the live-reload path are untouched.

## Proposed solution

### 1. `DisplayMode` value type (`state.rs`, native-testable)

Mirror the existing `LayoutMode` pattern exactly:

```rust
/// Which renderer the `#map` pane uses for the exploration graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayMode {
    /// Today's interactive SVG DAG (pan/zoom). The default.
    #[default]
    Graph,
    /// The published DOM indented tree-list.
    Tree,
}
```

with `css_class()` (`"display-graph"` / `"display-tree"` — unused by CSS today
but kept for symmetry/future), `as_token()` (`"graph"` / `"tree"`), and
`from_token()` (unknown → `Graph`). Unit tests match the `LayoutMode` tests:
default, token round-trip, unknown-token fallback.

A `display: RwSignal<DisplayMode>` signal is owned by `App` alongside `layout`
(session-only; survives manifest swaps).

### 2. `DisplayToggle` control (`toolbar.rs`)

A second segmented two-button group (`graph | tree`), structurally identical to
`LayoutToggle`, rendered in the header `.toolbar-area` before `LayoutToggle`.
Reuses the existing `.layout-toggle*` CSS classes (rename the CSS comment to
"segmented control", the class names already read generically) so no new toggle
skin is needed — or a shared `.seg-toggle` class if cleaner. Active segment gets
`is-active` + `aria-pressed="true"`; `data-mode` carries the token for tests.

### 3. Pure tree model (`tree.rs`, new module, native-testable)

A pure builder — no `web-sys`, fully unit-tested on native — that turns a
`&Manifest` into a renderable forest:

```rust
pub struct TreeRow {
    pub id: NodeId,
    pub label: String,           // title(label) ?? body(description) ?? "(untitled)" — matches nodeRow
    pub glyph: char,
    pub css_class: &'static str, // kind wire tag (question/experiment/…/other) for .glyph {type}
    pub is_dead_end: bool,
    pub dep_targets: Vec<NodeId>, // outgoing DependsOn edges, source order
}
pub struct TreeNode { pub row: TreeRow, pub children: Vec<TreeNode> }
pub struct TreeModel { pub roots: Vec<TreeNode>, pub isolated: Vec<TreeNode> }

pub fn tree_model(manifest: &Manifest) -> TreeModel;
```

**Glyph source — divergence to resolve.** The reference tree glyphs
(`Q ✦ → ✗ !`) differ from our SVG viewer's `kind_meta` glyphs (`Q E D X I`).
The fidelity mandate says match the reference; but `kind_meta` is documented as
"the single source of truth for glyph". Reproducing the reference tree exactly
means the tree uses the reference glyph set while the SVG graph keeps its own —
i.e. glyphs become renderer-specific, not a single source of truth. **Decision
needed (see review report):** either (i) change `kind_meta` glyphs to the
reference set (`✦ → ✗ !`) so both renderers match the published artifact and the
single-source-of-truth invariant holds — this visibly changes the existing SVG
graph, or (ii) add a tree-specific glyph map in `tree.rs` and note `kind_meta`
is SVG-graph-specific. Option (i) is more faithful to "match the published ARA"
and keeps one glyph authority; confirm before implementing. `label` also follows
the reference precisely: `title ?? body ?? "(untitled)"` (the SVG path uses
`label ?? id`; the tree must use the reference fallback chain).

Build rules (deterministic, source-order preserving):

- **Child adjacency** from `LinkKind::Child` links: `from → [to…]` in link
  source order.
- **Roots** = nodes (in `manifest.nodes` order — already pre-order DFS) with no
  incoming `Child` edge.
- Each root is expanded recursively via the child map into a `TreeNode`. A
  **visited set guards against cycles** (a malformed manifest with a Child cycle
  must not infinite-loop — a node already visited on the current path is not
  re-expanded).
- **`dep_targets`** per row = the `to` ids of that node's outgoing
  `LinkKind::DependsOn` links, in source order.
- **Isolated partition — CORRECTED after reading the reference.** The reference
  does **not** infer isolation from root position. It reads a per-node boolean
  **`isolated`** field off the JSON (`normalRoots = roots.filter(id=>!byId.get(id).isolated)`).
  **Our `ara_core::Node` has no `isolated` field**, and `docs/manifest-schema.md`
  never mentions one. So we cannot reproduce the reference's isobox partition
  from today's schema. Options (this is now the primary open decision — see the
  review report):
  - **(A) Widen the schema** (small `ara-core` change): add
    `Node.isolated: bool` (default `false`, `#[serde(default, skip_serializing_if
    = "is_false")]`) sourced from an `isolated:` key on the raw node, mirroring
    how the reference JSON carries it. This is the **only** way to match the
    reference exactly, but it breaks the plan's "no `ara-core` changes" claim.
  - **(B) Render every root at top level, no isobox this PR.** Faithful for the
    common single-root ARA (our demo has exactly one child-root, `N01`, so the
    isobox never appears anyway). The isobox lands with the schema-widening PR
    (aligns naturally with `T-REAL-CORPUS`, which already touches the schema).
  - **(C) Heuristic** (the original "first root = main, rest isolated"). Rejected:
    it fabricates isolation the reference derives from data, and would mis-group
    legitimately-parallel roots.
- Empty manifest → empty `TreeModel`.

Unit tests: single-tree nesting + depth; `dep_targets` populated from DependsOn
only (not Child); dead-end row flagged; cycle guard terminates; `label ?? id`
fallback; a round-trip against the checked-in `public/manifest.json` (asserts
the demo's single root `N01` + 15-node count). Isolated-partition tests are
gated on the decision above (only meaningful under A).

### 4. `TreeView` component (`tree.rs`)

Renders a `TreeModel` as scoped DOM inside `.tree-map`, reproducing the
reference `renderMap` markup exactly:

- Recursive `render_subtree(&TreeNode) -> AnyView`: emits a **`.node`** flex row
  matching `nodeRow` — `<span class="glyph {type}">{glyph}</span>` then a wrapper
  `<span>` with `<span class="meta"><span class="nid">{id}</span>[<span class="dep">]</span>`
  and `<div class="ntitle">{label}</div>`. When it has children, a **sibling**
  `<div class="kid">` holds the recursively-rendered children (not nested inside
  the row). Reference class names are used verbatim: **`.glyph`** (not `.chip`),
  **`.dep`** (not `.dep-marker`), **`.sel`** (not `.selected`), **`.dead`** (not
  `.dead_end`) — all scoped under `.tree-map`.
- `.node.dead` (dead-end, i.e. `row.className = "node dead"`) applies the
  reference rule `color:--warn; text-decoration:line-through` to `.ntitle`.
- Isolated roots (only under isolated-decision A) render inside a trailing
  `<div class="isobox"><div class="isohdr">isolated subtree</div>…`.
- **`depends_on`** rendered as `<span class="dep" title="depends on {ids}">⇠ {ids}</span>`
  where `{ids}` is the comma-joined dep target list — one marker per row, exactly
  as `nodeRow` does it (not one marker per target).
- **Selection:** the reference row is a plain `<div>` with a click handler and no
  a11y attributes. To match the reference *and* keep our stricter a11y bar (the
  SVG nodes are `tabindex=0`/`role="button"`), the row gets `tabindex=0`,
  `role="button"`, `aria-label="{label}, {kind}"`, and Enter/Space + click set
  the shared `selected` signal. Selected row gets **`.sel`**. `DetailPane`
  updates unchanged. *(This is an intentional a11y superset of the reference,
  not a divergence in look — noted so review doesn't flag it as drift.)*
- **Filter dimming + step count:** reuse the `matching: Memo<HashSet<NodeId>>`
  from `MapPane`; rows not in the set get **`.dim`**. The **`{shown} / {N} steps`**
  readout is the reference's shared `#rstat` span and lives in the **toolbar**,
  not inside `.tree-map` — it is written by both the filter (`applyFilters` →
  `"{shown} / {N} steps"`) and replay (`rstat` → `"step {i+1} / {N}"`). This
  plan therefore moves the count into the toolbar as a shared readout used in
  **both** Graph and Tree modes (the reference shows it regardless of the map
  renderer), resolving the earlier "Tree-only?" open question in favour of the
  reference behaviour.
- **Dependency hover highlight:** matches `nodeRow`'s `mouseenter`/`mouseleave`.
  A `hovered_deps: RwSignal<HashSet<NodeId>>` local to `TreeView`;
  `on:pointerenter`/`on:pointerleave` set/clear it to the row's `dep_targets`.
  Rows whose id is in the set get **`.deptarget`** (`background:--sel-bg;
  outline:1px dashed --accent`). Keyboard-only users still get the `⇠` text
  marker (the reference has no keyboard path for this; our text marker is the
  fallback).

### 5. `MapPane` — branch on `DisplayMode`

`MapPane` gains a `display: RwSignal<DisplayMode>` prop. The `MapSurface::Graph`
arm (nodes present) becomes: build the shared `matching` Memo once, render the
**`ReplayBar`** (step 6) above, then switch on `display.get()`:

- `Graph` → today's `GraphView` (+ the pan/zoom map-hint).
- `Tree` → `TreeView`.

The `{shown} / {N} steps` / `step {i} / {N}` readout is a toolbar-level shared
signal (see step 6 / the tree-view note), shown in both modes exactly as the
reference does. Loading / Error / Empty surfaces are unchanged and
mode-independent.

### 6. Replay stepper (`replay.rs` pure helpers + `ReplayBar` component)

Works in **both** modes; steps the shared `selected` signal through node order.

Pure (native-testable):

```rust
pub enum Step { Next, Prev }
pub fn node_order(manifest: &Manifest) -> Vec<NodeId>; // manifest.nodes order (pre-order DFS)
pub fn step(order: &[NodeId], current: Option<&NodeId>, dir: Step) -> Option<NodeId>;
pub fn counter(order: &[NodeId], current: Option<&NodeId>) -> (usize, usize); // (i, N), i is 1-based, 0 when no selection
```

- Reference `step(delta)` semantics: `i = clamp(0, N-1, indexOf(selected)+delta)`
  with `indexOf(None) = -1`. So `Next` from `None` → `order[0]`; `Prev` from
  `None` → `order[0]` too (`-1+(-1)=-2` clamps to 0), **not** last. Match the
  reference: `Prev` from no-selection selects the first node. Clamps at both ends
  (no wrap). Unknown id → `indexOf = -1`, same as `None`.
- `ReplayBar` component matches the reference toolbar controls: `‹` (id `rprev`)
  / `▶ Replay`⇄`⏸ Pause` (id `rplay`) / `›` (id `rnext`), + the shared `#rstat`
  count. `rprev`/`rnext` call `stop()` then `step(∓1)` (stop first, per the
  reference). Play toggles a **1300 ms** interval (reference value, not 1.1 s):
  if no selection it selects `order[0]`, sets the label to `⏸ Pause`, then each
  tick advances; at `i >= N-1` it calls `stop()` (auto-stop, no loop). `stop()`
  clears the timer and resets the label to `▶ Replay`. Interval setup/teardown is
  `#[cfg(target_arch = "wasm32")]`; on native the play button is inert.
- **`←` / `→` keys:** a document-level `keydown` listener (wasm-only, installed in
  `App` via an effect) mirrors the reference guard **exactly**:
  `if (target.tagName === "INPUT" || target.tagName === "SELECT") return;` then
  ArrowLeft → `stop(); step(-1)`, ArrowRight → `stop(); step(1)`. (Escape/panel
  hotkeys `c/g/d/r` are part 4, deferred.)

Unit tests (native): `node_order` equals `manifest.nodes` ids (DFS-from-roots ==
manifest order for a pre-order manifest); `step` clamp-at-both-ends /
`Prev`-from-None → first (reference quirk) / unknown-id; `counter` 1-based +
`(0, N)` when unselected; `rstat` string forms `"step {i} / {N}"` vs
`"{shown} / {N} steps"`.

### 7. `styles.css` — scoped tree-list skin + `--iso-*` tokens

- Add the reference tokens **verbatim**: `--iso-line:#cdbfa6`, `--iso-bg:#f7f1e6`,
  `--iso-ink:#8a7a5c` (already the values in `trajectory.html`).
- Add a `.tree-map` block copying the reference rules 1:1, only re-scoped:
  `.tree-map .node{display:flex;gap:9px;align-items:flex-start;padding:7px 9px;
  border-radius:9px;cursor:pointer;border:1px solid transparent}`,
  `.tree-map .node:hover{background:var(--panel2)}`,
  `.tree-map .node.sel{background:var(--sel-bg);border-color:var(--accent)}`,
  `.tree-map .node.deptarget{background:var(--sel-bg);outline:1px dashed var(--accent);outline-offset:-1px}`,
  `.tree-map .node.dim{opacity:.4}`,
  `.tree-map .glyph{width:21px;height:21px;border-radius:7px;…--glyph-bg/--glyph-ink}`,
  `.tree-map .glyph.dead_end{background:var(--warn);color:#fff}`,
  `.tree-map .nid{color:var(--muted);font-size:11px;mono}`,
  `.tree-map .ntitle{font-size:13px}`,
  `.tree-map .node.dead .ntitle{color:var(--warn);text-decoration:line-through;text-decoration-color:rgba(162,59,45,.4)}`,
  `.tree-map .kid{margin-left:19px;border-left:1px solid var(--line);padding-left:7px}`,
  `.tree-map .isobox{border:1px dashed var(--iso-line);…}` + `.isohdr`,
  `.tree-map .dep{color:var(--muted);font-size:10.5px;border:1px solid var(--line);border-radius:6px;padding:0 5px}`.
  Values are the reference's exact px/colours so the tree is visually identical.
- **Reuse `.node.dim`, not a new class** — matches the reference (`.dim`, same as
  our SVG `.node.dimmed`? no: SVG uses `.dimmed`, reference uses `.dim`; the tree
  uses `.dim` scoped under `.tree-map`, no conflict).
- The replay controls reuse the reference `.btn` / `.btn.primary` skin and the
  `.count` (`#rstat`) span — added to the toolbar area, not a separate
  `.replay-bar` (the reference has no separate bar; the controls sit inline in
  `.toolbar` after a `.spacer`). Add `.btn`/`.btn.primary`/`.count`/`.spacer`
  rules matching the reference if not already present.
- All tree rules are **prefixed with `.tree-map`** so `.node`/`.sel`/`.dim`/
  `.glyph` never touch the SVG graph. The `≤800px` responsive rules already stack
  the panes and need no tree-specific change.

### 8. Docs

- Add a **"Display modes"** section to `docs/stage-3-viewer.md` (next to the
  existing "Layout modes"): Graph (SVG DAG, default) vs Tree (DOM tree-list),
  the toggle, and the replay stepper.
- Note the tree model's root/isolated rule and that `depends_on` shows as `⇠ id`
  + hover `.deptarget`.
- After merge, per `AGENTS.md`, fold this plan into the design doc and remove it
  from `plans/`.

## Architecture summary (new/changed files)

| File | Change |
|------|--------|
| `state.rs` | + `DisplayMode` enum + tests |
| `tree.rs` | **new** — pure `tree_model` + `TreeView` component + tests |
| `replay.rs` | **new** — pure `node_order` / `step` / `counter` + `ReplayBar` + tests |
| `toolbar.rs` | + `DisplayToggle` component |
| `lib.rs` | + `display` signal; pass to `MapPane`; render `DisplayToggle`; wasm-only ←/→ key listener; branch `MapPane` on mode + render `ReplayBar` |
| `public/styles.css` | + `.tree-map` scoped skin, `.replay-bar`, `--iso-*` tokens |
| `tests/web.rs` | + tree render / toggle / replay browser tests |
| `docs/stage-3-viewer.md` | + "Display modes" section |
| `ara-core/src/{manifest,schema,parse}.rs` | **only under isolated-decision A** — add `Node.isolated` + `docs/manifest-schema.md` |
| `kind.rs` | **only under glyph-decision (i)** — reference glyph set `✦ → ✗ !` (touches the shipped SVG graph) |

## Implementation steps

1. `DisplayMode` in `state.rs` + native tests.
2. `tree.rs`: pure `tree_model` + `TreeRow`/`TreeNode`/`TreeModel` + native
   tests (build, isolated partition, deps, cycle guard, demo round-trip).
3. `replay.rs`: pure `node_order` / `step` / `counter` + native tests.
4. `TreeView` component in `tree.rs`; `ReplayBar` in `replay.rs`.
5. `DisplayToggle` in `toolbar.rs`.
6. Wire `lib.rs`: `display` signal, `MapPane` mode branch, `ReplayBar`, header
   toggle, wasm-only key listener.
7. `.tree-map` scoped CSS + `.replay-bar` + `--iso-*` tokens in `styles.css`.
8. Browser tests in `tests/web.rs`: tree rows + nesting + `.kid`, dead
   strikethrough class, `.isobox` present, `⇠` dep marker, `DisplayToggle`
   flips + swaps the rendered surface, replay next/prev updates `selected`.
9. `cargo build`, `cargo test --workspace`, `wasm-pack test --headless --chrome
   crates/ara-viewer`.
10. Regenerate the embedded viewer bundle (`scripts/embed-viewer.sh`) so
    `ara serve` ships the new UI; the `viewer-embed-fresh` CI check requires it.
11. Bump patch version in `Cargo.toml` + `CHANGELOG.md` `[Unreleased]` entry.
12. Update `docs/stage-3-viewer.md`.

## Scope / risk

Additive, medium size — **with two caveats the audit exposed:** faithful
reproduction may require an `ara-core` schema field (`isolated`, decision 1A) and
a `kind_meta` glyph change that touches the shipped SVG graph (decision 2i).
Under the viewer-only fallbacks (1B + 2ii) the "no `ara-core`/graph change" claim
holds but the tree isn't yet 100% faithful (no isobox; graph keeps `E D X I`).
Stage-2 layout, `scene.rs`, and the Stage-4 server are untouched either way.
New surface area: one enum, one pure tree builder, one pure replay helper set,
one `TreeView` component, the `ReplayBar` toolbar controls, one toggle, and a
scoped CSS block copied from the reference. Main risks: (a) CSS class collision —
mitigated by the `.tree-map` scope (`.node`/`.sel`/`.dim`/`.glyph` all scoped);
(b) the ←/→ listener hijacking search input — mitigated by the reference's exact
INPUT/SELECT target guard; (c) the play-interval leaking — mitigated by tearing
it down on pause / unmount / reaching the last node.

## Decisions to confirm in review

These are the real forks the audit surfaced. Reference behaviour is quoted; the
recommendation is the most faithful option.

1. **Isolated-subtree rule — needs a schema call.** The reference reads a
   per-node `isolated` boolean; `ara_core::Node` has no such field. **Recommend
   (A):** add `Node.isolated: bool` to `ara-core` (sourced from an `isolated:`
   raw key, `serde default false`) so the isobox reproduces exactly. This breaks
   the plan's "no `ara-core` change" claim but is the only faithful path. If you
   want to keep this PR viewer-only, take **(B)**: render all roots at top level,
   ship the isobox with the schema-widening PR (our demo has one root, so no
   visible difference today). **Not (C)** the position heuristic — it fabricates
   isolation.
2. **Glyph set — one authority or two?** Reference tree glyphs are `Q ✦ → ✗ !`;
   our `kind_meta` (the documented single source of truth) uses `Q E D X I`.
   **Recommend (i):** update `kind_meta` to the reference glyphs so both the SVG
   graph and the tree match the published artifact and there's still one glyph
   authority — this visibly changes the existing SVG graph's glyphs (E→✦, D→→,
   X→✗, I→!). If you'd rather leave the shipped graph untouched, take (ii): a
   tree-local glyph map, and downgrade `kind_meta`'s doc to "SVG-graph glyphs".
3. **Row label fallback.** Reference uses `title ?? body ?? "(untitled)"`; our
   SVG path uses `label ?? id`. Tree mode will follow the reference chain. Flag
   only if you'd prefer the tree keep showing the id when a title is absent
   (less faithful, but more debuggable).
4. **Replay interval.** Fixed at the reference **1300 ms** (was mis-stated as
   1.1 s in the first draft). Auto-stops at the last node, no loop. Confirm the
   feel is fine or name a different value.

**Resolved by reading the reference (no longer open):** the step count shows in
both modes (toolbar-level, shared by filter + replay) — it was never tree-only;
`Prev` from no-selection lands on the first node (reference clamp quirk), not the
last.

## GSTACK REVIEW REPORT

Design review run as a **fidelity audit**: the human dev pinned the design target
to the published `ARA-Labs/ARA-Demo` static artifact ("render the tree the same
way as the current static artifact"), so the correct review is "does the plan
reproduce `nanogpt_ara/trajectory.html` exactly?" — not mockup generation. The
reference file was cloned and read (tree render: `renderMap`/`nodeRow`/
`renderSubtree`; replay: `step`/`play`/`stop`; filter: `applyFilters`/`rstat`;
CSS `:root` tokens + `.node`/`.kid`/`.isobox`/`.deptarget` rules).

| Run | Source | Status | Findings |
|-----|--------|--------|----------|
| 1 | plan-design-review (fidelity audit vs trajectory.html) | issues_found | 7 |

Design-completeness rating: **4/10 before → 8/10 after.** Before: the plan was
written from issue #7's prose table, not the reference source, so it invented
class names (`.chip`/`.dep-marker`/`.selected`), a wrong isolation heuristic, a
wrong replay interval, and Tree-only step-count chrome. After: every class,
glyph, marker, spacing token, and control is pinned to the reference, with the
two genuine schema/authority forks (isolation field, glyph set) raised as
explicit decisions rather than silently assumed. It is not a 10 because two
faithfulness questions (1: `isolated` schema field; 2: `kind_meta` glyphs) are
one-way-ish calls only the human dev should make.

Findings folded into the plan:
1. **[HARD — correctness] Isolation was invented.** Reference reads a per-node
   `isolated` boolean; plan used a "first-root = main" heuristic. `ara-core` has
   no such field. Rewrote §3 with options A (add `Node.isolated`) / B (defer
   isobox) / reject-C (heuristic). Decision 1.
2. **[HARD — fidelity] Wrong class names.** Plan said `.chip`/`.dep-marker`/
   `.selected`/`.dead_end`; reference is `.glyph`/`.dep`/`.sel`/`.dead`. Fixed
   throughout §4 + §7 to the verbatim reference names (scoped under `.tree-map`).
3. **[fidelity] Glyph divergence.** Reference tree uses `Q ✦ → ✗ !`; our
   `kind_meta` uses `Q E D X I`. Raised as decision 2 (one glyph authority vs
   two); recommend updating `kind_meta` to match the published artifact.
4. **[fidelity] Replay interval wrong.** Plan said ~1.1 s; reference is 1300 ms.
   Also captured stop-first on prev/next, `▶ Replay`⇄`⏸ Pause` labels, auto-stop.
5. **[fidelity] Step-count mis-scoped.** Plan made it Tree-only chrome inside
   `.tree-map`; reference `#rstat` is a shared toolbar span used by filter AND
   replay in BOTH modes. Moved to toolbar; resolved the "Graph mode too?" open Q.
6. **[fidelity] Label fallback.** Reference row label is `title ?? body ??
   "(untitled)"`; plan used `label ?? id`. Fixed in §3 (decision 3 if disagreed).
7. **[fidelity] Markup shape / dep marker.** Reference emits ONE `.dep` marker
   per row (comma-joined ids) with a `title`, and nests children in a SIBLING
   `.kid` div. Plan implied per-target markers. Fixed §4.

CROSS-MODEL: not run — a fidelity audit against a fixed published reference is
resolved by reading that reference, not by soliciting alternative designs.
CODEX: not run (same reason).

VERDICT: **REVISE-THEN-PROCEED.** The plan is now faithful and implementation-ready
for everything except the two decisions below, which gate how faithful the first
PR can be. No design mockups generated (correct: the design is published and must
be matched, not re-explored).

**UNRESOLVED DECISIONS:**
- **D1 — Isolation field.** Add `Node.isolated` to `ara-core` (faithful isobox, breaks "viewer-only") or defer the isobox to the schema-widening PR (viewer-only, no visible diff on today's single-root demo)?
- **D2 — Glyph authority.** Change `kind_meta` glyphs to the reference set `✦ → ✗ !` (both renderers match the published artifact, but the shipped SVG graph's glyphs visibly change) or keep a tree-local glyph map (SVG graph untouched, two glyph authorities)?
