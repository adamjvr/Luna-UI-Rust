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
- cosmic-text shaping, fallback, bidi, caret stops, hit geometry, and raster images.
- Immutable raster-image display commands and straight-alpha CPU composition.
- Reusable `TextView` with shared paint, hit, scroll, caret, selection, and accessibility geometry.
- Native multilingual editable-text proof.

## M3 — twin native demos and reusable editor anatomy — complete

- Separate proof-gallery and editor-integration applications.
- Reusable shaped labels, buttons, toggles, progress indicators, and card borders.
- Responsive proof-gallery layout with logical-time animation and theme switching.
- Editor shell with menu bar, dirty tabs, project/sidebar tree, editor viewport, and status bar.
- Command palette and find/replace overlays.
- Expanded accessibility roles for menus, tabs, trees, dialogs, status, checkboxes, and progress.
- Optional host update cadence through winit `WaitUntil`, with rendering in `RedrawRequested`.

## M3.1 — incremental frame pipeline and performance baseline — active

### M3.1a — retained host pipeline and instrumentation — complete

- Stable invalidation classes and source classification.
- Application-selected invalidation through `HostControl::Invalidate`.
- Retained CPU framebuffer with dimension-based recreation.
- Size-gated softbuffer configuration.
- Direct packed-pixel conversion into acquired presentation storage.
- First-frame and periodic frame-stage timing.
- Allocation, resize, accessibility, and invalidation counters.

### M3.1b — persistent editor text and chrome — complete

- Shared immutable document snapshots and per-document retained cosmic-text layout state.
- Monotonic revision keys for actual text edits.
- Complete logical caret/hit/selection geometry retained across paint-only changes.
- Viewport-height glyph rasterization with vertical overscan.
- Visible-run cosmic-text rendering after scroll/band configuration.
- Raster-only theme invalidation.
- Stable-slot bounded cache for shell and overlay labels.
- Precise editor invalidation classes and no-op input suppression.
- Text-layout, raster, and label-cache diagnostics.

### M3.1c — gallery, accessibility, and input optimization — complete

- Shared retained static display lists and host-side static framebuffer caching.
- Dirty-region restoration for the animation lane instead of complete static-scene rerendering.
- Responsive gallery layout retained across animation and non-geometric state changes.
- Independently retained static paint, semantic trees, and stable label slots.
- Semantic hover-target tracking and pointer-motion coalescing.
- Deterministic accessibility-tree fingerprints.
- Shared semantic snapshots through `Arc`.
- AccessKit translation only after semantic/scale changes while active.
- Initial activation and deactivation-safe native accessibility behavior.
- Retained-scene, semantic-skip, cache, and input-coalescing diagnostics.

### M3.1d / M3.1d.1 — dropdown menus and routing correction — implemented, validation required

- M3.1d.1 priority routing: menu-heading presses replace any open palette/find surface in one event.
- Command palette removed from dropdown projection; Ctrl+P remains its exclusive opening path.
- Pointer-path regression test proves an open palette becomes an anchored File dropdown.
- Product-neutral menu definitions, commands, separators, interaction state, and layout snapshots.
- One command catalog projected into dropdown menus and the searchable command palette.
- Independent menu, palette, and find-panel presentation state.
- Anchored and viewport-clamped File/Edit/Find/View/Help dropdowns.
- Pointer opening, hover selection, heading traversal, activation, and outside-click dismissal.
- Up/Down/Home/End row navigation and Left/Right menu switching.
- Enter/Space activation and Escape dismissal.
- Disabled and checked command presentation with shortcut labels.
- Menu/menu-item accessibility and expanded top-level heading state.
- Shared command execution across keyboard, menus, palette, pointer, and accessibility.
- Overlay-only menu invalidation with no document reshape or reraster.

## M3.2 — real document and workspace runtime — next

- Real file open/save lifecycle and external-change detection.
- Document identity, revision ownership, dirty state, and save-conflict handling.
- Shared buffers with independent editor views.
- Product-neutral file/project/workspace adapter contracts.
- Project tree snapshots and watcher integration.
- Untitled/new-file and Save As lifecycle without Moth-specific policy.

## M3.3 — expanded editor shell

- Tab overflow, scrolling, pinned tabs, and targeted close routing.
- Split panes, draggable splitters, and independent view state.
- Nested submenus, mnemonic traversal, and broader menu focus integration.
- Product-neutral context menus and completion popups.
- Richer find/replace behavior and scrollbar interaction.

## M4 — GPU backend and rendering scalability

- `luna-render-wgpu` consuming the existing immutable display list.
- Surface lifecycle, device-loss recovery, batching, clip stacks, and glyph/image atlas upload.
- Retained CPU renderer as test oracle and fallback.
- Proof-gallery CPU/GPU comparison fixtures.

## M5 — broader editor component parity

- Syntax spans and theme adapters, including Sublime color-scheme compatibility.
- Richer command routing and accessibility actions.
- Undo/redo integration, multiple cursors, and full IME pre-edit handling.
- Behavior-parity fixtures against additional Swift Luna phases.

## M6 — Moth integration and platform parity

- Moth-owned document/session/project adapters.
- Packaging for Linux and macOS, followed by Windows host validation.
- Replay fixtures, accessibility audits, product performance gates, and integration hardening.

## Swift parity checkpoints

See [`SWIFT_PARITY.md`](SWIFT_PARITY.md). M3.1 now combines selected later-stage performance
mechanics with first-level desktop dropdown menus. M3.2 and M3.3 are still expected to produce the
largest visible parity gain.
