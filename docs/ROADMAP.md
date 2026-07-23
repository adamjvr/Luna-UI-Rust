# Roadmap

## M0 — deterministic foundation — complete

- Rust 2024 workspace and quality gates.
- Core IDs, geometry, diagnostics, input, and theme tokens.
- Display list and safe CPU rectangle renderer.
- Accessibility tree validation.
- Frame invalidation runtime.
- Widget contract and headless proof application.

## M1 — native host and reusable layout — complete

- `luna-layout` row/column/stack/split primitives with explicit immutable snapshots.
- `luna-commands` typed IDs, metadata, bindings, repeat policy, and dispatch requests.
- `luna-host-winit` using `ApplicationHandler` and `EventLoop::run_app`.
- DPI-aware CPU rendering, resize handling, input translation, and softbuffer presentation.
- AccessKit adapter using stable IDs and the same semantic geometry.
- Live native workspace proof application.

## M2 — editor-grade text — implemented in this overlay

- `luna-text` line-plus-UTF-8 coordinates, explicit snapping, anchor/focus ranges, immutable line
  snapshots, grapheme-safe movement/deletion, compact editing state, and scroll coordinates.
- `luna-text-cosmic` long-lived font/glyph caches, advanced shaping, fallback, bidi, ligatures,
  caret stops, hit testing, selection spans, visible ranges, and transparent glyph snapshots.
- immutable raster-image display commands and correct straight-alpha CPU composition.
- reusable `TextView` with clipping, current-line paint, selection, caret reveal, pointer mapping,
  scrolling limits, and text accessibility ranges from shared geometry.
- native multilingual editable-text proof application.
- deterministic fixtures ported from Swift Luna Phase 3A–3D text tests.

## M3 — GPU backend

- `luna-render-wgpu` consuming existing immutable display lists.
- surface lifecycle, device-loss recovery, batching, atlas upload, and frame pacing.
- retained CPU renderer as test oracle and fallback.

## M4 — reusable editor anatomy

- tabs, sidebar/project tree, status bar, menus, quick panel, find/replace, and completion popup.
- command routing, focus traversal, and accessibility parity.
- Sublime `.sublime-color-scheme` adapter.

## M5 — Moth integration and parity

- Moth-owned document/session/project adapters.
- behavior-parity fixtures against the Swift Luna implementation.
- packaging for Linux and macOS, followed by Windows host validation.
