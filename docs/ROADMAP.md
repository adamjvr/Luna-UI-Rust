# Roadmap

## M0 — deterministic foundation (this package)

- Rust 2024 workspace and quality gates.
- Core IDs, geometry, diagnostics, input, theme tokens.
- Display list and safe CPU rectangle renderer.
- Accessibility tree validation.
- Frame invalidation runtime.
- Widget contract and headless proof application.

## M1 — native host and reusable layout

- `luna-layout` with row/column/stack/split primitives and explicit measured snapshots.
- `luna-commands` with typed command IDs, bindings, and dispatch requests.
- `luna-host-winit` using the modern `ApplicationHandler`/`run_app` lifecycle.
- DPI-aware coordinates, resize handling, pointer/keyboard/text translation.
- AccessKit adapter wired to the same semantic geometry.

## M2 — editor-grade text

- UTF-8 document positions and ranges with explicit conversion boundaries.
- cosmic-text shaping adapter using advanced shaping for complex scripts, fallback, bidi, and
  ligatures.
- glyph cache and CPU glyph composition.
- selection, caret, scrolling, clipping, and hit testing.
- deterministic text fixtures ported from Swift tests.

## M3 — GPU backend

- `luna-render-wgpu` consuming existing immutable display lists.
- surface lifecycle, device-loss recovery, batching, atlas upload, and frame pacing.
- retained CPU renderer as test oracle and fallback.

## M4 — reusable editor anatomy

- tabs, sidebar/project tree, status bar, menus, quick panel, find/replace, completion popup.
- command routing, focus traversal, and accessibility parity.
- Sublime `.sublime-color-scheme` adapter.

## M5 — Moth integration and parity

- Moth-owned document/session/project adapters.
- behavior-parity fixtures against the Swift Luna implementation.
- packaging for Linux and macOS, followed by Windows host validation.
