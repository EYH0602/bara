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
  height; the **detail row** floors at 180px.

Both layouts share the same limitation; they differ only in the grid **axis**.
Users want to drag the middle separator to rebalance the panes:

- Split → drag the **vertical** divider left/right.
- Stack → drag the **horizontal** divider up/down.

The current divider is just a `border-right` / `border-bottom` on `.panel-map`
(`styles.css:229-236`) — purely visual, not interactive.

## Proposed solution

Add a **draggable gutter** element between the two panes and **per-mode
split-ratio signals** owned by `App`. The gutter becomes a real grid track;
dragging it (or arrow-keying it) updates the ratio, which is written to `<main>`
as a single **CSS custom property `--split`**. The stylesheet's grid-template
rules read `var(--split, <default>)`, so the app's existing `@media` collapse
still wins on mobile with no JS involved.

This mirrors the existing pointer-drag pattern used for graph pan/zoom
(`scene.rs:285-345`): `pointerdown` captures the pointer, `pointermove` computes
a new value from cursor position, `pointerup`/`pointercancel` release it. It also
mirrors the graph nodes' existing keyboard affordance (`tabindex=0` + key
handlers, `scene.rs`) for the keyboard splitter below.

```
                 .app-main (grid, 3 tracks)
   split mode →  [   map   ][gutter][   detail   ]   columns
   stack mode →  [   map   ]                          rows
                 [ gutter  ]
                 [ detail  ]

   Leptos writes:   style="--split: 42%"   on <main>
   CSS owns:        grid-template-columns: minmax(320px, var(--split,38%))
                                           var(--gutter)
                                           minmax(240px, 1fr);
   @media ≤800px:   grid-template-* : (single column) + gutter display:none
                    → overrides grid-template directly; --split is inert
```

### Design + eng review decisions (confirmed with developer)

- **CSS custom property, not inline `grid-template`** (eng review; supersedes the
  earlier matchMedia approach). Leptos writes only `style="--split: {pct}%"` on
  `<main>`; the stylesheet keeps ownership of `grid-template-*` via
  `var(--split, <default>)`. The existing `@media (max-width: 800px)` rule
  overrides `grid-template-*` directly, so the mobile collapse works in **pure
  CSS** — no `matchMedia`, no viewport signal, no listener lifecycle. This is
  strictly less code than gating an inline `grid-template` on a JS signal.
- **In-memory only** — the ratio is a plain `RwSignal`, reset to the default on
  each page load. No `localStorage`/URL persistence. Correct for **arahub**
  multi-ARA hub mode (per-page-session, no per-ARA storage). Because there is no
  persistence, **double-click-to-reset ships in v1** — otherwise an extreme drag
  has no recovery except a full page reload.
- **Two per-mode ratio signals** — `split_ratio` (default **0.38**) and
  `stack_ratio` (default **0.667**), matching today's shipped look. A single
  shared ratio bleeds a column-fraction into the row split when toggling
  split↔stack (fractions aren't comparable across axes).
- **Gutter-aware ratio math** (eng review). The draggable line is the gutter
  centre, at `pane1 + gutter/2`, not `pane1`. The pointer→ratio conversion
  subtracts `gutter/2`, and `clamp_split_ratio` takes `gutter_px` so the floors
  aren't off by the gutter width (see §4).
- **Correct, structural pane floors** (eng review). Floors are enforced by
  `minmax()` on **both** grid tracks (not just the map), so they hold on initial
  render, reset, mode-toggle, and resize — not only during an active drag:
  - Split: map ≥ **320px** (matches today), detail ≥ **240px** (new resizer floor).
  - Stack: map ≥ **180px** (new resizer floor so the map can't collapse), detail
    ≥ **180px** (**matches today** — the earlier draft's 160px was a regression).
- **Thin visible line, wide invisible hit area** — the painted divider stays
  **1px** (a centered `::before`); the interactive gutter track is ~6px (mouse),
  widening under coarse pointers.
- **Calm hover, loud only while dragging** — hover brightens the 1px line with
  the `0.12s` app-standard transition using a mid-tone, **not** a full-bar
  `--accent` fill. Full `--accent` is used only for the active-drag state.
- **Reactive `.is-dragging`, not imperative** (eng review). `<main>`'s class is
  owned by Leptos (`class=move || …`, `lib.rs:149`); toggling `.is-dragging` via
  `class_list().add()` would be wiped on the next reactive re-render. It is a
  `dragging` signal folded into the class closure.
- **Global body lock while dragging** — a `body.is-resizing` class sets **both**
  `user-select: none` **and** the resize cursor, so a fast drag over the header
  keeps the cursor and never selects text (the cursor lives on `body`, not
  `.app-main`).
- **Accessible window-splitter** — the gutter is a real WAI-ARIA splitter:
  focusable, keyboard-operable, and value-bearing (see §2).

### 1. `App` (`lib.rs`) — per-mode ratio signals + `--split`

Alongside the existing `layout` signal (`lib.rs:73`):

```rust
// Fraction of the main axis given to the map pane (0.0–1.0). In-memory only;
// resets to the default on reload. Two signals so split (width) and stack
// (height) fractions don't bleed into each other on mode toggle.
let split_ratio: RwSignal<f64> = RwSignal::new(0.38);  // matches today's split
let stack_ratio: RwSignal<f64> = RwSignal::new(0.667); // matches today's stack

// A tiny drag-active flag, folded into the <main> class closure (NOT patched
// imperatively — Leptos owns that attribute).
let dragging: RwSignal<bool> = RwSignal::new(false);

// The ratio signal for the active mode (what the gutter handlers read/write).
let active_ratio = move || match layout.get() {
    LayoutMode::Split => split_ratio,
    LayoutMode::Stack => stack_ratio,
};
```

The three grid children (map, gutter, detail) flow into 3 columns (split) or 3
rows (stack) from the same DOM order — no conditional markup, only the
`grid-template-*` axis differs (owned by CSS).

`<main>` gets `--split` as an inline **custom property** and `.is-dragging` via
the reactive class closure:

```rust
<main
    class=move || format!(
        "app-main {}{}",
        layout.get().css_class(),
        if dragging.get() { " is-dragging" } else { "" },
    )
    style=move || format!("--split: {}%;", (active_ratio().get() * 100.0).clamp(0.0, 100.0))
>
```

Because Leptos only writes a custom property, the stylesheet's `grid-template-*`
rules stay authoritative and the `@media (max-width: 800px)` block overrides
them directly (§3). No `matchMedia`, no `use_wide_viewport`, no web-sys
`MediaQueryList` feature, no listener to tear down.

### 2. Gutter element + `Splitter` (in `splitter.rs`)

The `Splitter` component, the pure `clamp_split_ratio`, and the keyboard-step
math live in a **new `splitter.rs` module** (props: `layout`, `split_ratio`,
`stack_ratio`, `dragging`) — colocating the resize logic and keeping the pure
core native-unit-testable, consistent with `scene.rs` / `detail.rs` / `filter.rs`.

Insert the gutter `<div>` between the two `<section>`s (`lib.rs`, between the map
and detail sections):

```rust
<section id="map" class="panel panel-map" …>…</section>

<div
    class="panel-gutter"
    role="separator"
    tabindex="0"                               // focusable splitter widget
    aria-label="Resize panels"
    aria-orientation=move || match layout.get() {
        LayoutMode::Split => "vertical",
        LayoutMode::Stack => "horizontal",
    }
    // Reachable bounds, not a nominal 0–100: reflect the clamp window so AT
    // doesn't announce a range the control can't reach. Computed from the last
    // measured axis; falls back to 0/100 before first measure.
    aria-valuemin=move || value_min_pct()
    aria-valuemax=move || value_max_pct()
    aria-valuenow=move || (active_ratio().get() * 100.0).round() as i64
    on:pointerdown=…   // set_pointer_capture; dragging.set(true); body.is-resizing on
    on:pointermove=…   // if dragging: gutter-aware ratio (below), clamped
    on:pointerup=…     // release_pointer_capture; dragging.set(false); body lock off
    on:pointercancel=… // same cleanup as pointerup (belt-and-suspenders)
    on:keydown=…       // Arrow-by-axis ±step, Home/End → min/max, clamped
    on:dblclick=…      // reset the active mode's ratio to its default
>
</div>

<section id="detail" class="panel panel-detail" …>…</section>
```

**Pointer move** reads the `.app-main` bounding rect (`get_bounding_client_rect()`
→ needs the `DomRect` web-sys feature, see §5) and subtracts the gutter centre:

- Split: `raw = (clientX - rect.left - gutter/2) / rect.width`
- Stack: `raw = (clientY - rect.top  - gutter/2) / rect.height`

**Keyboard** (WAI-ARIA window-splitter) reuses the same clamp core:

- Split (vertical): `ArrowLeft`/`ArrowRight` step ±`0.02`; Stack (horizontal):
  `ArrowUp`/`ArrowDown`.
- `Home` → min ratio, `End` → max ratio (the clamp-window ends).
- Each key `preventDefault()`s (no page scroll), then writes the clamped ratio;
  `aria-valuenow` updates reactively.

Both paths run through `clamp_split_ratio` (§4). The pointer handlers own the
`dragging` signal and the `body.is-resizing` class; **cleanup runs on BOTH
`pointerup` and `pointercancel`** (and `release_pointer_capture`) so a cancelled
drag can't leave the body locked — note the existing `scene.rs:340` pan handler
does *not* release capture, so do not copy it verbatim for this global control.

**Double-click** resets the active mode's ratio to its default (0.38 / 0.667) —
the only recovery path given the in-memory ratio.

### 3. CSS (`styles.css`)

Paint only a centered 1px line; the 6px track is an invisible hit area. The grid
templates read `var(--split, …)` and floor **both** tracks with `minmax()`:

```css
:root { --gutter: 6px; }              /* hit area; visible line is 1px */

/* Grid templates now read the runtime ratio; both tracks are floored. */
.app-main.layout-split {
  grid-template-columns: minmax(320px, var(--split, 38%)) var(--gutter) minmax(240px, 1fr);
}
.app-main.layout-stack {
  grid-template-rows: minmax(180px, var(--split, 66.7%)) var(--gutter) minmax(180px, 1fr);
}

.panel-gutter {
  position: relative;
  align-self: stretch;
  justify-self: stretch;
  background: transparent;            /* track invisible; line is ::before */
  touch-action: none;
}
.panel-gutter::before {
  content: "";
  position: absolute;
  background-color: var(--line);
  transition: background-color 0.12s; /* app-standard timing */
}
.app-main.layout-split .panel-gutter::before { top: 0; bottom: 0; left: 50%; width: 1px; transform: translateX(-50%); }
.app-main.layout-stack .panel-gutter::before { left: 0; right: 0; top: 50%; height: 1px; transform: translateY(-50%); }

.app-main.layout-split .panel-gutter { cursor: col-resize; }
.app-main.layout-stack .panel-gutter { cursor: row-resize; }

/* Calm hover/focus: brighten the 1px line, NOT a full --accent bar. */
.panel-gutter:hover::before,
.panel-gutter:focus-visible::before {
  background-color: color-mix(in srgb, var(--accent) 45%, var(--line));
}
.panel-gutter:focus-visible { outline: 2px solid var(--accent); outline-offset: -1px; }

/* Active drag: full --accent line + GLOBAL body cursor + selection lock, so a
   fast drag over the header keeps the resize cursor and selects nothing. */
.app-main.is-dragging .panel-gutter::before { background-color: var(--accent); }
body.is-resizing { user-select: none; }
body.is-resizing.resizing-col { cursor: col-resize; }
body.is-resizing.resizing-row { cursor: row-resize; }

/* Touch/coarse pointers: grow the invisible hit area, keep the line thin. */
@media (pointer: coarse) { :root { --gutter: 24px; } }
```

- **Remove** the old `border-right` / `border-bottom` on `.panel-map`
  (`styles.css:229-236`) — the gutter's `::before` line is the divider now.
- The `@media (max-width: 800px)` block (`styles.css:1089`) **overrides
  `grid-template-*` directly** (it already sets a single column + `1fr 1fr`
  rows) and additionally hides the gutter: `.app-main .panel-gutter { display: none; }`.
  Because it sets `grid-template-*` (not `--split`), it wins over the var-based
  rules with no JS — this is the whole reason for the custom-property approach.
  The two visible panels then take the two collapse rows; `--split` is inert.

### 4. Pure-Rust testable core (`splitter.rs`)

Gutter-aware clamp; the detail (pane 2) floor is a named constant, and the
signature carries `gutter_px` so the window is correct:

```rust
/// Clamp a raw 0–1 ratio (map fraction of the FULL axis) so both panes keep
/// their px floors, accounting for the gutter. Available space for the two
/// panes is `axis_px - gutter_px`; the window is
/// [pane1_min/axis, (axis - gutter - pane2_min)/axis].
fn clamp_split_ratio(raw: f64, axis_px: f64, gutter_px: f64,
                     pane1_min_px: f64, pane2_min_px: f64) -> f64 { … }
```

Floors: split → map 320 / detail 240; stack → map 180 / detail **180** (matches
today's detail row). Keyboard step: fixed `±0.02`, run through the same clamp.

Tests (native, no DOM): floors respected at both ends **for both mode floor
sets**, gutter subtracted correctly, midpoint passes through, degenerate
`axis_px` (0 / tiny / < gutter) doesn't panic or NaN, keyboard step clamps
identically to the pointer path.

## Implementation steps

1. New `splitter.rs`: `clamp_split_ratio(raw, axis, gutter, min1, min2)` +
   keyboard-step helper + `Splitter` component skeleton; native unit tests for
   the pure core (both floor sets, gutter, degenerate axis, key-step parity).
2. `App` (`lib.rs`): `split_ratio` (0.38), `stack_ratio` (0.667), `dragging`
   signal, `active_ratio` helper; write `--split` as an inline custom property
   and fold `.is-dragging` into the reactive class closure on `<main>`.
3. Gutter element + handlers: pointer (gutter-aware ratio, capture, release on
   up/cancel), keyboard (Arrow/Home/End), `dblclick` reset, `body.is-resizing`
   (+ `resizing-col`/`resizing-row`) toggles, and `aria-valuenow` +
   reachable-bounds `aria-valuemin/valuemax`.
4. CSS: `--gutter`, `var(--split, …)` in both `grid-template-*` rules with
   `minmax()` floors on both tracks, `.panel-gutter` + `::before` 1px line, calm
   hover, focus ring, `.is-dragging` line, global `body.is-resizing` cursor +
   `user-select`, `@media (pointer: coarse)` hit-area, hide the gutter under
   800px, remove the old `.panel-map` borders.
5. Add the `DomRect` feature to the runtime wasm32 `web-sys` (`Cargo.toml:38`) —
   `get_bounding_client_rect()` needs it (the other events come from Leptos).
6. Rebuild + re-embed: `trunk build --release` → `scripts/embed-viewer.sh` →
   `crates/ara-cli/assets/viewer/` (baked via `include_dir!`). This is the
   "viewer-embed-fresh" regen — expected for a functional viewer change.
7. Manual verification (`cargo run -- serve`):
   - Drag in split + stack; floors hold; the gutter line tracks the cursor with
     no half-gutter jump.
   - Fast drag over the header — cursor stays resize, no text selection.
   - Keyboard: Tab to gutter, Arrow/Home/End resize; a screen reader announces a
     value within the reachable min/max.
   - Double-click resets to the mode default.
   - Toggle split↔stack — each keeps its own ratio.
   - Resize below 800px — single-column collapse, gutter gone, no h-scroll.
   - Coarse-pointer emulation — the widened hit area is grabbable.
8. Version + changelog: functional viewer change → bump the patch version in
   `Cargo.toml` and add a `CHANGELOG.md` `### Added` entry.
9. After merge: rewrite this plan as a design doc under `docs/` (extend
   `docs/stage-3-viewer.md`) and remove it from `plans/`.

## Test plan (full coverage — native + wasm)

**Native unit (`splitter.rs`):** clamp floors for both mode sets, gutter
subtraction, midpoint, degenerate axis, key-step/pointer parity.

**Headless-chrome (`tests/web.rs`, wasm-bindgen-test):**
- Pointer drag in split → `--split` / column ratio updates; clamp holds at both floors.
- Pointer drag in stack → row ratio updates; clamp holds.
- Keyboard Arrow/Home/End → ratio + `aria-valuenow` update; bounds reachable.
- Double-click → ratio returns to the mode default.
- Per-mode preservation: set split ratio, toggle to stack and back → split ratio intact.
- `body.is-resizing` is set on `pointerdown` and **cleared on both `pointerup`
  and `pointercancel`** (no stuck global lock).
- **REGRESSION (mandatory):** at a <800px viewport, `.app-main` is single-column
  and the gutter is `display:none` — guards the mobile collapse the custom-prop
  approach exists to preserve.

## Resolved decisions

1. **Mobile approach → CSS custom property** (supersedes the design-review
   matchMedia choice). Media query overrides `grid-template` in pure CSS.
2. **Per-mode ratio → two signals** (0.38 split / 0.667 stack).
3. **Double-click reset → ship in v1.**
4. **Old divider borders → remove.**
5. **Splitter home → new `splitter.rs` module.**
6. **Floors → structural `minmax()` on both tracks;** stack detail floor stays
   180px (no regression); gutter-aware clamp.

## What already exists (reuse, don't reinvent)

- **Design system:** the warm-cream tokens in `styles.css` (`--line`, `--accent`,
  `--sel-bg`, `--accent-text`) + the `0.12s` interactive-control idiom
  (`.btn` `:1037`, `.layout-toggle-btn` `:150`). The gutter speaks this.
- **Pointer-drag mechanic:** `scene.rs:285-345` — but note it does **not**
  release pointer capture on up/cancel; the splitter must, since it holds a
  global body lock.
- **Keyboard affordance:** graph nodes are `tabindex=0` + key-handled (`scene.rs`).
- **Listener/cleanup discipline:** `on_cleanup` for the replay interval
  (`lib.rs:111`) and the document arrow-key listener (`replay.rs`).
- **Responsive collapse:** the `@media (max-width: 800px)` block (`styles.css:1089`)
  already force-stacks; the custom-prop approach lets it win with zero JS.
- **web.rs harness:** existing headless-chrome interaction tests are the home for
  the new drag/keyboard/reset/regression tests.

## NOT in scope (explicitly deferred)

- **Ratio persistence** (`localStorage` / URL) — in-memory only; double-click
  reset covers the recovery gap.
- **Always-visible grip handle** — hover-cursor + focus ring is the power-user minimum.
- **Collapse-to-zero (Enter to collapse)** — the WAI-ARIA splitter's optional
  collapse is not shipped; Enter is a no-op and `aria` doesn't advertise it.
- **Resizing on mobile (<800px)** — meaningless in a forced stack; gutter hidden.
- **rAF-throttling pointermove** — the per-move reactive write matches the
  existing pan/zoom path (~120fps at corpus sizes); only revisit if profiling
  shows jank.

## Implementation Tasks
Synthesized from this review's findings. Each task derives from a specific
finding above. Run with Claude Code or Codex; checkbox as you ship.

- [ ] **T1 (P1, human: ~45min / CC: ~10min)** — lib.rs / styles.css — Drive a CSS custom property `--split` from Leptos; CSS owns `grid-template` reading `var(--split)`; hide gutter under 800px so the media query collapses in pure CSS
  - Surfaced by: Eng Architecture + cross-model — matchMedia was self-inflicted complexity; custom-prop deletes the viewport signal + MediaQueryList feature + listener lifecycle
  - Files: crates/ara-viewer/src/lib.rs, crates/ara-viewer/public/styles.css
  - Verify: web.rs regression test — <800px is single-column, gutter display:none
- [ ] **T2 (P1, human: ~1h / CC: ~15min)** — splitter.rs — New module with gutter-aware `clamp_split_ratio(raw, axis, gutter, min1, min2)` + keyboard-step math + native unit tests
  - Surfaced by: Eng Code Quality (module boundary) + Codex (gutter offset in ratio/clamp math)
  - Files: crates/ara-viewer/src/splitter.rs, crates/ara-viewer/src/lib.rs
  - Verify: cargo test — floors for both mode sets, gutter subtracted, key-step parity
- [ ] **T3 (P1, human: ~3-4h / CC: ~35min)** — splitter — WAI-ARIA window-splitter keyboard pattern (tabindex, Arrow/Home/End) + `aria-valuenow` and reachable-bounds `aria-valuemin/valuemax`; add `DomRect` web-sys feature
  - Surfaced by: Design Pass 6 (a11y) + Codex (hardcoded 0/100 bounds inaccurate) + Eng (DomRect for getBoundingClientRect)
  - Files: crates/ara-viewer/src/splitter.rs, crates/ara-viewer/Cargo.toml
  - Verify: web.rs — key→ratio+aria; screen reader announces reachable value
- [ ] **T4 (P1, human: ~1h / CC: ~15min)** — lib.rs / splitter — Reactive `dragging` signal in the `<main>` class closure (not imperative); global `body.is-resizing` cursor + `user-select` lock; release pointer capture + clear lock on up AND cancel
  - Surfaced by: Codex — imperative class_list wiped by Leptos re-render; body cursor scoped to .app-main; scene.rs doesn't release capture
  - Files: crates/ara-viewer/src/lib.rs, crates/ara-viewer/src/splitter.rs, crates/ara-viewer/public/styles.css
  - Verify: web.rs — is-resizing set on pointerdown, cleared on pointerup+pointercancel; fast drag over header keeps cursor
- [ ] **T5 (P1, human: ~2h / CC: ~20min)** — styles.css — Thin 1px `::before` line + wide invisible hit area; calm `0.12s` hover (mid-tone, not full `--accent`); `.is-dragging` accent line
  - Surfaced by: Design Pass 5 (design system) — 1px/6px contradiction, loud/un-animated hover
  - Files: crates/ara-viewer/public/styles.css
  - Verify: manual — resting divider is 1px, hover is subtle, drag line is accent
- [ ] **T6 (P2, human: ~40min / CC: ~10min)** — lib.rs / styles.css — Two per-mode signals (0.38/0.667) + structural `minmax()` floors on BOTH tracks (split 320/240, stack 180/180 matching today's detail floor)
  - Surfaced by: Design Pass 7 + Codex — 0.5 default regression; stack detail floor was wrongly lowered to 160; pane2 only floored during drag
  - Files: crates/ara-viewer/src/lib.rs, crates/ara-viewer/public/styles.css
  - Verify: web.rs — toggle split↔stack preserves each ratio; first load matches today
- [ ] **T7 (P2, human: ~30min / CC: ~5min)** — splitter — Double-click gutter → reset active mode's ratio to default
  - Surfaced by: Design Pass 3 — in-memory ratio makes an extreme drag a no-recovery trap
  - Files: crates/ara-viewer/src/splitter.rs
  - Verify: web.rs — drag to extreme, dblclick, layout returns to default
- [ ] **T8 (P2, human: ~20min / CC: ~5min)** — styles.css — `@media (pointer: coarse)` widen the gutter hit area (~24px), keep the line thin
  - Surfaced by: Design Pass 6 — 6px << 44px touch target on touch laptops/tablets in split mode
  - Files: crates/ara-viewer/public/styles.css
  - Verify: coarse-pointer emulation — divider grabbable; visible line unchanged
- [ ] **T9 (P1, human: ~3h / CC: ~30min)** — tests/web.rs — Full interaction coverage: drag (split+stack), floor clamp, keyboard+aria, dblclick, per-mode preservation, body-lock cleanup, + the mandatory <800px regression test
  - Surfaced by: Eng Test review — 7 wasm interaction paths + 1 regression uncovered
  - Files: crates/ara-viewer/tests/web.rs
  - Verify: wasm-pack test --headless --chrome (the viewer-web-test CI job)

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
|--------|---------|-----|------|--------|----------|
| CEO Review | `/plan-ceo-review` | Scope & strategy | 0 | — | — |
| Codex Review | `/codex review` | Independent 2nd opinion | 1 | issues_found | outside-voice: 11 findings, 6 folded |
| Eng Review | `/plan-eng-review` | Architecture & tests (required) | 1 | clean | 3 issues, 0 critical gaps |
| Design Review | `/plan-design-review` | UI/UX gaps | 1 | issues_open | score: 6/10 → 9/10, 6 decisions |
| DX Review | `/plan-devex-review` | Developer experience gaps | 0 | — | — |

- **CODEX:** Outside voice found what the 4-section review missed — the gutter-offset ratio/clamp math, the stack-floor regression (detail 180→160), pane-2 floored only during drag, `.is-dragging` patched imperatively vs the reactive class, the body cursor scoped to `.app-main`, and the matchMedia-vs-CSS-var simplification. All folded in after user approval.
- **CROSS-MODEL:** Eng review + Codex agreed on the web-sys feature gap and the thin wasm coverage. The one tension (matchMedia vs CSS custom property) was reopened and resolved to CSS-var, superseding design-review D3 — it deletes the viewport signal, the MediaQueryList feature, and the listener footgun.
- **VERDICT:** DESIGN + ENG CLEARED — ready to implement. All 3 eng findings folded, 0 critical failure-mode gaps, full test coverage (native + web.rs) specified with a mandatory <800px regression test.

NO UNRESOLVED DECISIONS
