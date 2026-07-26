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

## M3.1 — incremental frame pipeline and performance baseline — complete

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

### M3.1d — dropdown menus and routing correction — complete

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

## M3.2 — real document and workspace runtime — complete after local validation

### M3.2a — document identity and lifecycle model — complete

- Product-neutral `luna-documents` crate.
- Stable document IDs and monotonic untitled naming.
- Adapter-canonicalized file identities and duplicate-open prevention.
- Saved edit revisions, dirty state, storage snapshots, and external conflict state.
- Explicit Save and close requirements without filesystem or product policy.
- Save As identity reassignment and index release on close.
- Editor-demo integration with dirty-close protection and honest non-I/O Save status.

### M3.2b — file and dialog service boundaries — complete

- Product-neutral UTF-8 file read/write service contracts.
- Strict invalid-encoding errors without lossy replacement.
- Deterministic content revisions and optimistic write preconditions.
- Same-directory temporary writes and atomic replacement.
- Open, Save As, overwrite/conflict, and dirty-close dialog contracts.
- In-memory file and scripted dialog adapters with deterministic lifecycle tests.
- Linux native dialogs through Zenity with KDialog fallback.
- Editor integration for Open, Save, Save As, duplicate activation, close resolution, and conflict
  reload/overwrite/cancel.

### M3.2c — recent files and external-change delivery — complete

- Bounded MRU recent-file model with canonical identities.
- File-menu and command-palette recent-file projections.
- Content revision plus concrete storage-instance snapshots.
- Modified, replaced, missing, and recreated-file distinctions.
- UI-thread storage observation with unchanged-state frame suppression.
- Status/accessibility notices and explicit Reload from Disk.
- Save/conflict handling for clean or dirty externally changed documents.

### M3.2d — workspace and project-tree runtime — complete

- Product-neutral recursive workspace snapshots and scan adapters.
- Exact stable path identities and directories-before-files ordering.
- Hidden, symlink, depth, permission, and unreadable-node policies.
- Expansion, selection, ancestor reveal, and refresh preservation.
- Native Open Folder and real editor sidebar rows.
- Duplicate-safe file activation and one-second UI-thread refresh.

### M3.2e — workspace operations, views, and persistent sessions — complete

- Product-neutral create file/folder, rename, and recursive delete operations.
- Explicit fail/replace collision policy and native confirmation boundaries.
- Dirty affected-document Keep Open, Discard & Close, or Cancel handling.
- File and ancestor-directory rename propagation into open document and recent-file identities.
- Versioned persistent recent-file and workspace tree state through `luna-session`.
- Startup restoration of workspace root, expanded paths, and selection.
- Multiple document-view identities sharing one buffer identity.
- Native watcher-event and full/subtree refresh-scope boundaries.
- Deterministic standard-library, in-memory, scripted-dialog, session, and editor tests.

## M3.3 — expanded editor shell — complete

### M3.3a — shared-buffer split panes — complete

- Recursive horizontal and vertical pane trees with stable identities.
- Multiple `DocumentViewId` records sharing one `DocumentId` lifecycle buffer.
- Synchronized shared text revisions with independent caret, selection, scroll, focus, and caches.
- Pane-local tabs, close/collapse behavior, and unique-document rehoming.
- Draggable splitters, minimum pane sizes, depth-first focus traversal, and accessibility.

### M3.3b — advanced tabs and command surfaces — complete

- Tab overflow, scrolling, pinned tabs, preview tabs, and active-tab visibility.
- Local drag reordering and cross-pane movement.
- Nested submenus, mnemonic traversal, and broader menu focus integration.
- Product-neutral tab context menus and completion popups.
- Richer find/replace behavior and interactive scrollbar geometry.

### M3.3c — desktop interaction hardening — complete

- Versioned recursive pane/tab/view session persistence with V1 compatibility.
- Per-view caret, directional selection, scroll, active-view, and focused-pane restoration.
- Dirty-buffer and storage-baseline persistence with restart-time external-change detection.
- Keyboard tab reordering and previous/next-pane movement.
- Arbitrary-depth cascading popup paths, pointer intent, and delayed-hover state.
- Asynchronous completion-provider contracts, cancellation, and stale-result rejection.
- Search history, wrap/selection options, and scrollbar page-track affordances.
- Native-first Linux watcher delivery, polling fallback, event coalescing, and incremental subtree
  reconciliation.

## M4 — GPU backend and rendering scalability — complete after local validation

- `luna-render-wgpu` consumes the existing immutable display list through ordered solid/image quads.
- `luna-host-wgpu` drives the existing native application contract with AccessKit preserved.
- Surface resize/loss/suboptimal/outdated recovery and event-driven device-loss reconstruction.
- Consecutive scissor batching, nested display-list clip stacks, and bounded BGRA image-atlas upload.
- Retained CPU renderer remains the default deterministic oracle and fallback.
- Proof-gallery and editor CPU/GPU runtime selection with a comparison checklist.
- Stable Luna Dark, Luna Light, Amber Monitor, and Green Terminal theme presets.

## M5 — broader editor component parity — implemented; local validation pending

- Product-neutral `luna-editor` crate for syntax snapshots, themes, history, selections, IME, and
  behavior fixtures.
- Validated syntax spans and a comment-tolerant Sublime `.sublime-color-scheme` adapter.
- Retained foreground shaping plus backend-neutral syntax backgrounds and underlines.
- Bounded coalescing edit history with text-based saved checkpoints and deterministic undo/redo.
- Multiple directional selections, simultaneous edits, vertical cursor creation, and grapheme-safe
  multi-cursor deletion.
- Native IME enable/disable/pre-edit/commit delivery plus candidate-window caret geometry.
- Dynamic command availability and explicit accessibility action/value routing.
- Editor integration for typing, deletion, completion, find/replace, IME, accessibility, and history.
- Rendering-independent parity fixtures suitable for matching Swift or other implementations.
- Advisory macOS build/test lane and documented Apple-Silicon graphical acceptance protocol.

## M6 — macOS hardening and downstream integration

- Close macOS-specific host, Metal/wgpu, IME, VoiceOver, dialog, watcher, and packaging gaps.
- Promote macOS CI from advisory to blocking only after repeated real-hardware acceptance.
- Add downstream adapter examples without committing Luna-UI-Rust to any particular product rewrite.
- Expand replay fixtures, accessibility audits, performance gates, and integration hardening.
- Windows remains best-effort and non-blocking: no official CI, packaging, release, or support target.

## Swift parity checkpoints

See [`SWIFT_PARITY.md`](SWIFT_PARITY.md). M3.1 now combines selected later-stage performance
mechanics with first-level desktop dropdown menus. M3.2 supplies the real file/workspace runtime;
M3.3a supplies the first live recursive pane runtime. M3.3b adds advanced tabs, nested menus, context menus, completion, richer find/replace, and
scrollbars. M3.3c adds durable pane sessions, recursive popup routing, asynchronous providers, and
native-first incremental workspace delivery. M4 adds the optional `wgpu` renderer/host and
four-palette comparison matrix. M5 adds reusable syntax/theme, history, multiple-selection, IME,
command-state, and accessibility-action mechanics. M6 concentrates on macOS hardening; Windows is
not an official project target.
