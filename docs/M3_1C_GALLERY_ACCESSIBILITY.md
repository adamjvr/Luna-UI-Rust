# M3.1c Gallery, Accessibility, and Input Optimization

M3.1c completes the incremental CPU-frame groundwork before real file/workspace expansion.

## Retained static paint

`UiFrame` can carry a `RetainedDisplayList` containing:

```text
static scene revision
shared immutable static display list
logical dirty region for dynamic paint
```

The host rasterizes the static list only when its revision, physical size, or scale factor changes.
The working framebuffer then receives either:

- one full restore after a new static revision; or
- one clipped restore of the animation lane on ordinary timed samples.

Only the dynamic display list is painted after restoration. The proof gallery separates static card,
control, label, and lane-background paint from the moving square and pulse marker.

## Independent gallery caches

The proof gallery retains separate snapshots for:

- responsive layout, keyed only by viewport geometry;
- static paint, keyed by viewport, theme, control state, activation count, and visual hover target;
- accessibility semantics, keyed by viewport, theme, and semantic control state;
- shaped labels, retained through stable `TextLabelCache` slots.

Animation time participates in none of those keys. Hover does not invalidate semantics.

## Pointer coalescing

Pointer motion is reduced to a small semantic target enum:

```text
none
button
toggle
theme card
```

Movement inside the same target produces no frame. A redraw is requested only when the target—and
therefore visible hover paint—changes. Releases and irrelevant button presses remain no-ops.

## Semantic fingerprints and AccessKit

`AccessibilityTree` computes one deterministic fingerprint during validation. Equivalent trees have
the same fingerprint regardless of node insertion order. `UiFrame` stores semantic snapshots in
`Arc`, allowing paint-only frames to reuse the complete tree without deep copies.

The native host translates a tree only when:

- AccessKit is active; and
- the semantic fingerprint changed; or
- the native scale factor changed.

An `InitialTreeRequested` event always receives a complete current tree. Deactivation clears the
host's translated-snapshot state so later activation cannot reuse stale native semantics.

## Diagnostics

Host diagnostics report:

```text
retained base hits/misses
full and dirty-region restores
accessibility translations/skips
invalidation classes
frame-stage timings
```

Gallery diagnostics report layout, static-scene, semantic, and label-cache hits/misses plus coalesced
pointer motion.

## Acceptance checks

```bash
cargo fmt --all
./scripts/validate.sh
cargo run --release -p luna-ui-rust-editor-demo
cargo run --release -p luna-ui-rust-proof-gallery
```

Expected proof-gallery behavior:

- first frame: layout/static/semantic misses and one retained-base miss;
- ordinary animation: layout/static/semantic hits and dirty-region restores;
- no static label shaping during animation;
- no AccessKit translation during animation when semantics and scale are unchanged;
- movement inside one hover target produces no frame;
- crossing hover targets produces one paint-overlay frame and a new static revision;
- control activation rebuilds static paint and semantics as appropriate;
- theme changes rebuild static paint, label pixels, and semantics without recalculating responsive
  geometry;
- resize rebuilds layout, static paint, semantics, and native retained buffers safely.
