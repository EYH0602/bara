# Resizable viewer panels (drag the map/detail divider)

## Problem background

The viewer's `.app-main` is a CSS grid with **hard-coded** pane proportions.
There is no way for the user to change how much space the map vs. detail pane
gets — the divider between them is a static border, not a draggable handle.

- **Split** (side-by-side): `styles.css:216` →
  `grid-template-columns: minmax(320px, 38%) 1fr`. The map is pinned at ~38%
  width regardless of content or screen size.
- **Stack** (top/bottom, default): `styles.css:210` →
  `grid-template-rows: 2fr minmax(180px, 1fr)`. The map is pinned at ~2/3
  height.

Both layouts share the same limitation; they differ only in the grid **axis**.
Users want to drag the middle separator to rebalance the panes:

- Split → drag the **vertical** divider left/right.
- Stack → drag the **horizontal** divider up/down.

The current divider is just a `border-right` / `border-bottom` on `.panel-map`
(`styles.css:229-236`) — purely visual, not interactive.

## Proposed solution

Add a **draggable gutter** element between the two panes and a **split-ratio
signal** owned by `App`. The gutter becomes a real grid track; dragging it
updates the ratio, which drives an inline `grid-template` style that overrides
the stylesheet default. One resizer mechanism serves both layouts — only the CSS
property it writes (`grid-template-columns` vs `grid-template-rows`) and the
cursor axis change with `LayoutMode`.

This mirrors the existing pointer-drag pattern used for graph pan/zoom
(`scene.rs:285-345`): `pointerdown` captures the pointer, `pointermove` computes
a new value from cursor position, `pointerup`/`pointercancel` release it.

### Design decisions (confirmed with developer)

- **In-memory only** — the ratio is a plain `RwSignal`, reset to the default on
  each page load. No `localStorage`/URL persistence for now. This is correct for
  **arahub** too (the same `App` renders in multi-ARA hub mode): the ratio is
  per-page-session and needs no per-ARA storage.
- **Thin gutter, hover-highlight** — keep the current 1px divider look; widen the
  *hit area* to ~6px, show the resize cursor and brighten the line on hover. No
  always-visible grip.
- One shared ratio signal across both modes is acceptable for a first cut. (Open
  question below on whether split and stack should remember separate ratios.)

### 1. `App` (`lib.rs`) — own the ratio signal

Alongside the existing `layout` signal (`lib.rs:73`):

```rust
// Fraction of the main axis given to the map pane (0.0–1.0). In-memory only;
// resets to the default on reload. Clamped on drag so neither pane collapses.
let split_ratio: RwSignal<f64> = RwSignal::new(0.5); // see note on per-mode defaults
```

The three grid children (map, gutter, detail) flow into **3 columns** in split
mode or **3 rows** in stack mode from the same DOM order, so no conditional
markup is needed — only the `grid-template-*` axis differs.

Apply the ratio as an inline style on `<main>` (added next to the existing
`class=move || …` at `lib.rs:149`):

```rust
<main
    class=move || format!("app-main {}", layout.get().css_class())
    style=move || {
        let pct = (split_ratio.get() * 100.0).clamp(0.0, 100.0);
        match layout.get() {
            LayoutMode::Split =>
                format!("grid-template-columns: minmax(320px, {pct}%) var(--gutter) 1fr;"),
            LayoutMode::Stack =>
                format!("grid-template-rows: minmax(180px, {pct}%) var(--gutter) 1fr;"),
        }
    }
>
```

The inline `style` wins over the stylesheet's `.layout-split` /
`.layout-stack` grid-template rules, so those CSS rules become the *fallback*
(e.g. before first interaction, or if JS is disabled). Keeping the `minmax()`
floor here means pane 1 can't be dragged below its minimum; pane 2 is floored by
clamping the ratio in the drag handler (below).

### 2. New gutter element + `Splitter` handler

Insert a gutter `<div>` between the two `<section>`s (`lib.rs`, between lines
161 and 162):

```rust
<section id="map" class="panel panel-map" …>…</section>

<div
    class="panel-gutter"
    role="separator"
    aria-orientation=move || match layout.get() {   // "vertical" split / "horizontal" stack
        LayoutMode::Split => "vertical",
        LayoutMode::Stack => "horizontal",
    }
    on:pointerdown=…   // capture pointer (el.set_pointer_capture)
    on:pointermove=…   // if dragging: ratio = cursor pos relative to .app-main bbox, clamped
    on:pointerup=…     // stop
    on:pointercancel=… // stop
>
</div>

<section id="detail" class="panel panel-detail" …>…</section>
```

The move handler reads the **`.app-main` bounding rect** (via
`ev.current_target()` → parent element `getBoundingClientRect()`), then:

- Split: `ratio = (clientX - rect.left) / rect.width`
- Stack: `ratio = (clientY - rect.top)  / rect.height`

Clamp so **both** panes keep a minimum (e.g. map ≥ 320px/180px and detail ≥
some floor): convert the min-px floors to fractions from the measured
`rect.width`/`rect.height` and clamp the ratio into that window before writing
the signal. A small drag-active flag (`RwSignal<bool>`, like `scene.rs`'s
`drag_start`) gates `pointermove`.

Whether the `Splitter` lives inline in `lib.rs` or as its own `splitter.rs`
module (props: `layout`, `split_ratio`) is an implementation detail; a small
module keeps `App` readable and is unit-test-friendly for the clamp math.

### 3. CSS (`styles.css`)

- Add a `--gutter` size var (~6px) and gutter styling near the panel rules
  (lines 220-236):

```css
:root { --gutter: 6px; }              /* or scope to .app-main */

.panel-gutter {
  background-color: var(--line);
  align-self: stretch;
  justify-self: stretch;
  touch-action: none;                 /* let pointer drags through on touch */
}
.app-main.layout-split .panel-gutter { cursor: col-resize; }
.app-main.layout-stack .panel-gutter { cursor: row-resize; }
.panel-gutter:hover { background-color: var(--accent); } /* hover highlight */
```

- Drop the now-redundant `border-right` / `border-bottom` divider on
  `.panel-map` (`styles.css:229-236`) — the gutter track is the divider now (or
  keep them as the pre-JS fallback and let the gutter sit flush; decide during
  implementation).
- The existing `.layout-split` / `.layout-stack` `grid-template-*` rules stay as
  the fallback but must add the gutter track so the 3-child grid lays out even
  before the inline style applies, e.g.
  `grid-template-columns: minmax(320px, 38%) var(--gutter) 1fr`.

### 4. Pure-Rust testable core (clamp math)

Keep the ratio-clamping logic in a small free function so it's unit-testable off
the DOM (the WASM `pointermove` handler just calls it):

```rust
/// Clamp a raw 0–1 ratio so both panes keep their px floors, given the measured
/// main-axis length in px.
fn clamp_split_ratio(raw: f64, axis_px: f64, pane1_min_px: f64, pane2_min_px: f64) -> f64 { … }
```

Tests: floors respected at both ends, midpoint passes through, degenerate
`axis_px` (0 / tiny) doesn't panic or produce NaN.

## Implementation steps

1. Add `clamp_split_ratio` (+ unit tests) — `state.rs` or new `splitter.rs`.
2. Add the `split_ratio` signal in `App` and the inline `style` on `<main>`.
3. Add the gutter element + pointer handlers (inline or `Splitter` component).
4. CSS: `--gutter`, `.panel-gutter` (hover + per-mode cursor), add gutter track
   to the fallback grid rules, reconcile the old divider borders.
5. Rebuild the WASM viewer and re-embed:
   `trunk build --release` → `scripts/embed-viewer.sh` →
   `crates/ara-cli/assets/viewer/` (baked via `include_dir!` in
   `crates/ara-cli/src/serve/assets.rs`). This is the "viewer-embed-fresh"
   regen — expected for a functional viewer change.
6. Manual verification (`cargo run -- serve` on a sample corpus): drag the
   divider in both split and stack, confirm the floors hold, the cursor/hover
   affordances show, and switching layout mode keeps a sane split.
7. Version + changelog: this is a **functional** viewer change → bump the patch
   version in `Cargo.toml` and add a `CHANGELOG.md` `### Added` entry.
8. After merge: rewrite this plan as a design doc under `docs/` (extend
   `docs/stage-3-viewer.md` or a new note) and remove it from `plans/`.

## Open questions for review

1. **Per-mode ratio?** One shared `split_ratio` means dragging in split mode also
   moves the stack divider (and vice versa), since the fractions aren't
   comparable across axes. Cleaner UX: two signals (`split_ratio`, `stack_ratio`)
   or a `[f64; 2]` keyed by mode. Slightly more state; recommended if it reads
   odd in testing. Default plan: start with **two signals**.
2. **Default ratios.** Match today's look: split ≈ 0.38, stack ≈ 0.667. (The 0.5
   above is a placeholder.)
3. **Double-click to reset?** Common splitter affordance — double-clicking the
   gutter restores the default ratio. Cheap to add; include now or defer?
4. **Old divider borders** — remove entirely, or keep as a pre-JS fallback?
