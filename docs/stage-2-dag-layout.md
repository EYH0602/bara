# Stage 2 — Layered DAG Layout in `ara-core`

Design record for the deterministic node layout shipped in Stage 2 (PR #5,
workspace `0.0.4`). It adds **node geometry** to the Stage-1 `Manifest` and
freezes the geometry wire shape, while keeping the logical model additively
extensible.

Companion docs: [`stage-1-core-parse-validate.md`](stage-1-core-parse-validate.md)
(the logical manifest this consumes) and [`manifest-schema.md`](manifest-schema.md)
(the wire shape).

## Why layout in core, not client-side dagre.js

The exploration tree is a typed DAG that reads best with a layered (Sugiyama)
layout, not force-directed. Computing positions in `ara-core` rather than the
client means: one language for the whole pipeline; deterministic, server-side
snapshot testing of geometry; an offline `ara layout --json`; and no JS layout
dependency in the wasm client.

This was contingent on a cross-target determinism spike: the chosen pure-Rust
Sugiyama layout had to be wasm-safe (no threads / `rand` / `SystemTime` /
filesystem), internally deterministic (fixed tie-break, no randomness), and
produce a **byte-identical** positioned `Manifest` when run native vs `wasm32`.
That last point is the real contract — laying out twice on the same binary does
not test it. The spike passed; the crate is pinned exactly. (The fallback, had it
failed, was client-side dagre.js with `ara-core` staying logical-graph-only —
`T-LAYOUT-SPIKE-FALLBACK`.)

## Shape

Layout is a plain function in `layout.rs` (no trait — one impl, one caller):

```
layout(&Manifest, &LayoutOptions) -> LayoutResult
  • rank (top-down)
  • fixed tie-break (sort by NodeId)  → equal-rank order is stable
  • fixed node W/H (LayoutOptions)
  • canonicalize f64 coords
  • bounds = union of node rects
```

`parse_and_layout(dir, opts) -> Result<(Manifest, ParseReport), ParseReport>`
runs the Stage-1 parse and, **on parse error (including cycles), returns the
report unchanged and skips layout**. Stage 1 already treats cycles as errors, so
a cyclic graph never reaches layout; a cheap `debug_assert!` guards the invariant.

## Determinism knobs

`LayoutOptions` pins rank direction (top-down), node/rank separation, a fixed
tie-break (sort by `NodeId`), and **fixed node dimensions** (default `180 × 60`).
No randomness anywhere.

`Point`/`Rect` use `f64` with a canonicalization step before serialization: round
to N decimals and normalize `-0.0 → 0.0` so JSON is byte-stable across platforms.
This is the serialize-side half of the determinism contract; the native≡wasm
golden test is the compute-side half.

**Node-sizing tradeoff:** Sugiyama needs node width/height as *input*, but real
box sizes depend on browser text measurement. Core lays out with the fixed
`LayoutOptions` dimensions and labels are truncated/ellipsized to fit; the client
may relayout (same `ara-core` wasm) if it needs exact text fit. Core's output is
authoritative for the fixed-size case.

## Manifest extension (additive)

`Node.pos: Option<Point>` and `Manifest.bounds: Option<Rect>`, both with
`#[serde(skip_serializing_if = "Option::is_none")]` so Stage-1 snapshots stay
byte-identical when layout is off. **No `Link.route`** — edge routing is deferred
to the client (`T-EDGE-ROUTING`), which draws straight/orthogonal edges from
endpoints.

## CLI

The `parse_dir → handle Err(report) → optional --json emit` preamble was factored
out of `validate()` into a shared helper, then:

- `ara validate --layout` — runs layout, reports node/edge counts + bounds
  (same exit-code semantics as `validate`).
- `ara layout <dir> --json` — dumps the positioned `Manifest` (the JSON the
  Stage 3 client consumes).

## Frozen geometry, extensible model

The `Manifest` JSON is documented in [`manifest-schema.md`](manifest-schema.md).
The **geometry** wire shape (`Point`, `Rect`, `pos`, `bounds`) is frozen. The
logical model stays additively extensible — new node kinds via `NodeKind::Other`,
extra fields via the existing `extra` capture, gated by a `schema_version` — so
real-corpus widening (`T-REAL-CORPUS`) can land without a breaking bump. Only a
geometry change needs a coordinated `ara-core` + client bump.

## Acceptance (met)

The positioned `Manifest` is deterministic in-process **and** native≡wasm, and
snapshot-stable on the corpus. `ara layout --json` emits the frozen geometry wire
format. Ranks are monotonic along `Child` edges; all positions are finite (no
`NaN`/`inf`); the empty and single-node cases produce valid bounds without
panicking.

## Deferred

Rendering and the canvas fast-path (Stage 3, decided empirically); edge routing
(`T-EDGE-ROUTING`); real-corpus schema widening (`T-REAL-CORPUS`); the
client-side dagre.js fallback (`T-LAYOUT-SPIKE-FALLBACK`, only if the spike had
failed).
