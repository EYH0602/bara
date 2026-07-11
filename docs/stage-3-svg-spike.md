# Stage 3a — SVG Spike Gate

**Result: SVG is viable. DOM tree-list pivot is NOT triggered.**

## Purpose

This document records the early readability/scale gate that was performed *before* building the interaction layer (Step 3b). Its goal: confirm that a static skinned SVG render of the largest known ARA corpus tree reads clearly and will not hit the DOM threshold that would trigger the canvas swap (Step 8).

## Corpus scale

| Metric | Demo manifest | Largest known corpus |
|--------|--------------|----------------------|
| Nodes  | 15           | ~34                  |
| Edges  | 17           | ~50 (estimated)      |

## SVG element budget per node

Each `SceneNode` produces:

| Element | Purpose |
|---------|---------|
| `<g>` (group) | Node container with kind CSS class |
| `<rect>` (node bg) | Rounded box: fill `--panel`, stroke `--line` |
| `<rect>` (glyph chip) | 20×20 chip: `--glyph-bg` (or `--warn` for dead ends) |
| `<text>` (glyph char) | Single glyph letter in chip |
| `<text>` (label) | Node label, with `<title>` child for native tooltip |
| `<title>` | Full label for browser tooltip |
| `<text>` (badge) | Kind badge in bottom-right corner |

Total per node: **6–7 elements**.

Each `SceneEdge` produces one `<path>` element.

## Total element counts

| Target | Nodes | Edges | SVG elements (approx.) |
|--------|-------|-------|------------------------|
| Demo manifest (15 nodes / 17 edges) | 15 | 17 | ~107–122 |
| Largest real corpus (~34 nodes / ~50 edges) | 34 | 50 | ~254–288 |

These numbers are **far below** the "few thousand DOM elements" threshold where SVG rendering degrades. That threshold is the Step-8 quantitative fps switch criterion.

## Readability assessment

The static skinned render provides:

- **Glyph chip** (Q/E/D/X/I/?) at the top-left of each node — kind is readable without colour.
- **Label** in the node body — truncated visually, full text accessible via `<title>` native tooltip.
- **Kind badge** at the bottom-right corner in `--muted` — secondary kind confirmation.
- **Dead-end colour**: only `DeadEnd` nodes use `--warn` (amber/red) on the chip. All other kinds use the neutral `--glyph-bg`. This keeps the encoding colourblind-safe.
- **Child edges**: solid stroke in `--line`.
- **DependsOn edges**: dashed stroke in `--muted` to distinguish cross-reference edges from parent→child nesting.

## Conclusion

**SVG is viable at ARA corpus scale.** The static render reads clearly. The DOM tree-list pivot (cheap fallback) is **not triggered**.

The quantitative fps measurement is deferred to Step 8 (post-build, post-interaction layer).

### Visual confirmation (orchestrator, `trunk serve`)

A live screenshot of the demo manifest (15 nodes) rendered under `trunk serve`
confirmed the static skinned SVG reads clearly: glyph chips (Q/E/D/X/I), node
labels, and kind badges are legible; dead-end nodes show the red `--warn` chip
while all other kinds stay neutral (colourblind-safe); `Child` edges are solid
and `DependsOn` edges dashed. No console errors. **Gate passed — proceeding to
Step 3b.**

Two framing follow-ups for Step 3b (already in its scope): the initial view
should *fit-to-pane* (the raw `viewBox = bounds` is very wide/short, so it
renders small in a tall pane until zoomed), and node labels need the planned
2-line clamp + ellipsis (they currently overflow the fixed box).

## Canvas swap threshold (for reference)

The `CanvasRenderer` stub in `src/canvas.rs` is the contingency path. It would be promoted to a full implementation only if the Step-8 fps probe shows:

- The SVG DOM approach cannot sustain 60 fps on the largest ARA corpus at the target viewport size.
- OR: a user study shows that the pan/zoom interaction requires `requestAnimationFrame`-level control not achievable with CSS transforms on an SVG subtree.

Neither condition is expected to trigger given the element counts above.
