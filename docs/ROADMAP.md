# Roadmap

## M0 — deterministic foundation — complete

- Rust 2024 workspace and quality gates.
- Core IDs, geometry, diagnostics, input, and theme tokens.
- Display list and safe CPU reference renderer.
- Accessibility tree validation.
- Frame invalidation runtime.
- Widget contract and headless proof application.

## M1 — native host and reusable layout — complete

- Deterministic row/column/stack/split layout snapshots.
- Typed commands, key bindings, and dispatch requests.
- winit application lifecycle, softbuffer presentation, DPI conversion, and input translation.
- Stable-ID AccessKit bridge.
- Native workspace proof.

## M2 — editor-grade text — complete

- Stable line-plus-UTF-8 coordinates and grapheme-safe editing.
- cosmic-text shaping, fallback, bidi, caret stops, hit geometry, and raster caching.
- Immutable raster-image display commands and straight-alpha CPU composition.
- Reusable `TextView` with shared paint, hit, scroll, caret, selection, and accessibility geometry.
- Native multilingual editable-text proof.

## M3 — twin native demos and reusable editor anatomy — implemented in this overlay

- Separate proof-gallery and editor-integration applications matching the Swift project's testing
  split.
- Reusable shaped labels, buttons, toggles, progress indicators, and card borders.
- Responsive proof-gallery layout with logical-time animation and theme switching.
- Editor shell with menu bar, dirty tabs, project/sidebar tree, editor viewport, and status bar.
- Command palette and find/replace overlays.
- Expanded accessibility roles for menus, tabs, trees, dialogs, status, checkboxes, and progress.
- Optional host update cadence through winit `WaitUntil`, while all rendering remains in
  `RedrawRequested`.

## M4 — GPU backend and rendering scalability

- `luna-render-wgpu` consuming the existing immutable display list.
- surface lifecycle, device-loss recovery, batching, clip stacks, and glyph/image atlas upload.
- retained CPU renderer as test oracle and fallback.
- proof-gallery CPU/GPU comparison fixtures.

## M5 — broader editor component parity

- completion popup, context/drop-down menus, scrollbars, split-editor interaction, and focus traversal.
- richer command routing and accessibility actions.
- syntax span/theme adapters, including Sublime color-scheme compatibility.
- behavior-parity fixtures against additional Swift Luna phases.

## M6 — Moth integration and platform parity

- Moth-owned document/session/project adapters.
- packaging for Linux and macOS, followed by Windows host validation.
- performance baselines, replay fixtures, accessibility audits, and product integration gates.
