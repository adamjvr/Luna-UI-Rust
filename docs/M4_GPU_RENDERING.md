# M4 GPU Backend and Rendering Scalability

## Scope

M4 adds an optional native `wgpu` presentation path without changing Luna's widget or application
contracts. The existing CPU renderer remains the deterministic reference and default fallback path.
The proof gallery can run the same immutable `UiFrame` through either backend, making differences in
geometry, clipping, image composition, text placement, and theme rendering directly observable.

M4 also expands the built-in theme preset list from two to four palettes:

- **Luna Dark** — existing neutral dark reference;
- **Luna Light** — existing neutral light reference;
- **Amber Monitor** — orange/amber phosphor on nearly black surfaces;
- **Green Terminal** — bright green phosphor on nearly black surfaces.

The terminal-inspired palettes are semantic Luna themes, not syntax themes. They exercise every
existing widget and text surface through the same foreground, panel, border, hover, selection, and
accent derivation rules.

## New crate boundaries

### `luna-render-wgpu`

This backend consumes one or more immutable `DisplayList` layers and performs no widget or
application policy. It provides:

- ordered solid-color and image quads;
- consecutive draw batching by physical scissor rectangle;
- nested logical clip stacks translated with the CPU renderer's exact DPI coverage rule;
- a bounded per-frame BGRA8 image atlas with deterministic fingerprint deduplication;
- nearest-neighbor sampling for already-rasterized Luna text and images;
- straight-alpha blending and sRGB-aware solid-color conversion;
- render statistics for command, batch, vertex, index, image, and atlas-upload counts;
- pure scene-compiler tests that do not require a GPU or native window.

The GPU backend does not shape text. `luna-text-cosmic` continues to own text layout and raster
images, so CPU and GPU presentation consume identical glyph pixels and logical placement.

### `luna-host-wgpu`

The native GPU host drives the existing `luna-host-winit::NativeApplication` trait. It owns:

- winit window and event-loop lifecycle;
- `wgpu` instance, surface, adapter, device, queue, and surface configuration;
- resize and scale-factor reconfiguration;
- `CurrentSurfaceTexture` success, suboptimal, outdated, lost, timeout, occlusion, and validation
  handling;
- device-loss callback state, event-loop wakeup, and complete renderer/device reconstruction;
- retained-static plus dynamic display-list submission in painter order;
- the same input translator and AccessKit bridge used by the CPU host;
- per-second GPU timing, batching, atlas, surface-recovery, and device-recovery diagnostics.

Dropping and rebuilding the GPU runtime never recreates application state or semantic IDs.

## Display-list clip contract

`DisplayCommand` now includes `PushClip` and `PopClip`. Both renderers maintain a root target clip and
intersect every nested push with the current clip. Extra pops are ignored. An image's existing
command-local clip is intersected with the active stack; disjoint clips draw nothing.

This preserves the governing invariant:

```text
logical widget clip
    -> one immutable display-list command stream
    -> CPU physical coverage
    -> GPU scissor coverage
```

Clear remains a complete-target operation and is not constrained by the current clip stack.

## CPU/GPU proof selection

The proof gallery defaults to the established CPU path:

```bash
cargo run --release -p luna-ui-rust-proof-gallery
```

Select the GPU path without changing application code:

```bash
LUNA_RENDER_BACKEND=wgpu cargo run --release -p luna-ui-rust-proof-gallery
```

On Linux, a specific driver backend can be selected for diagnosis:

```bash
WGPU_BACKEND=vulkan LUNA_RENDER_BACKEND=wgpu \
  cargo run --release -p luna-ui-rust-proof-gallery
```

## Theme validation

The proof-gallery theme card cycles in stable order:

```text
Luna Dark -> Luna Light -> Amber Monitor -> Green Terminal -> Luna Dark
```

The editor exposes the same presets through **View > Color Scheme** with one checked row. It also
honors `LUNA_RENDER_BACKEND=wgpu`, allowing the complete editor shell and all four palettes to run
through either presentation backend. A theme change clears retained label pixels and editor text
raster caches but does not alter document text, pane topology, command state, sessions, or workspace
state.

## Automated acceptance

Run:

```bash
./scripts/test-m4.sh
```

The script formats the workspace, checks every target and feature, focuses the two M4 crates, runs
strict Clippy, runs all tests, builds rustdoc with warnings denied, and writes a manual runtime
checklist to `/tmp/luna-m4-comparison/README.txt`.

## Manual acceptance

The phase is accepted only after both CPU and GPU proof-gallery runs are checked in a graphical
Pop!_OS session:

1. all four themes render with readable foreground, selection, borders, and hover states;
2. multilingual glyph pixels and alpha-composited images occupy the same logical geometry;
3. nested clips and card edges do not leak;
4. repeated resize and DPI changes do not produce stale surfaces or distorted output;
5. accessibility activation and actions remain available on the GPU host;
6. stderr contains no `wgpu` validation failure;
7. the editor runs through both CPU and GPU presentation and its Color Scheme menu checks one row;
8. CPU remains runnable when the GPU backend is unavailable or intentionally not selected.

## Deferred work

M4 deliberately does not add vector paths, gradient primitives, texture-atlas persistence across
frames, syntax spans, Sublime color-scheme import, undo/redo, multiple cursors, or IME pre-edit.
Those remain M5 concerns. More sophisticated GPU resource retention may build on the M4 statistics
without changing display-list or widget APIs.
