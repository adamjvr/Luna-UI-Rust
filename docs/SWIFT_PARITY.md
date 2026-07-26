# Swift Luna UI Parity

This document records the functional relationship between Luna-UI-Rust and the original Swift
Luna UI implementation. It is a directional feature-inventory assessment, not a line-count or API
percentage guarantee.

## Current estimate after M4 implementation

```text
foundational architecture       79–87% parity
core text/editing mechanics     68–77% parity
reusable editor UI              68–78% parity
file/workspace/pane integration 68–77% parity
total Swift functional surface  approximately 72–79%
```

Rust is no longer a framework skeleton. It has shared native host contracts, backend-neutral display
lists, CPU and optional GPU renderers, real text shaping, editable Unicode geometry, accessibility,
reusable editor anatomy,
typed invalidation, retained framebuffers, retained document layout/raster state, static gallery
retention, conditional semantic translation, functional first-level desktop dropdown menus,
a product-neutral document lifecycle model, real UTF-8 file operations, atomic optimistic saves,
native Linux dialog boundaries, a mutable recursive workspace runtime, persistent recent/workspace
session restoration, a live recursive split-pane runtime, advanced tab policy and movement, child
arbitrary-depth submenus, tab context menus, asynchronous completion delivery, richer find/replace,
an interactive scrollbar, durable recursive sessions, native-first workspace watching, and four
built-in color schemes. Swift remains broader and closer to a complete reusable editor platform.

## Capability matrix

| Area | Luna-UI-Rust after M4 | Swift Luna UI | Relative state |
|---|---|---|---|
| Core architecture | IDs, geometry, input, commands, layout, four theme presets, CPU/GPU rendering, accessibility, and host boundaries | Same broad spine with more downstream use | Near parity |
| CPU rendering | Display lists, images, alpha composition, retained host/static layers, dirty-region restore | CPU framebuffer and proof-frame caching | Near parity |
| Text shaping | cosmic-text fallback, bidi, grapheme and caret geometry | HarfBuzz/FreeType cluster geometry | Near parity |
| Text caching | Retained per-document logical layout and overscanned raster bands | Exact shaped rows; visible-row virtualization remains an active scalability target | Rust ahead in scroll-time raster reuse |
| Basic editing | Insertion, deletion, caret, selection, scrolling, hit testing, and editable semantics | Same foundation with deeper gesture/view integration | Mostly parity |
| Editor shell | Menu bar, pane-local tabs, recursive editor lanes, real workspace sidebar, and status bar | Richer docking, tab policy, and product integration | Swift moderately ahead |
| Command palette | Filtering, selection, activation, and accessibility | Unified command runtime, availability, and disabled-state behavior | Swift ahead |
| Find/replace | Literal search with case/whole-word, wrap, selection-only, bounded history, replace-current/all, shared geometry, and accessibility | Regex and deeper product integration | Swift moderately ahead |
| Menus | Arbitrary-depth anchored cascades, checked/disabled rows, shortcuts, mnemonics, hover intent, pointer/keyboard navigation, and accessibility | Deeper product command-state integration | Rust core implemented; Swift broader |
| Context menus | Tab-anchored context state reusing dropdown commands, geometry, pointer, keyboard, and accessibility | Broader product-neutral context definitions and routing | Rust foundation implemented; Swift broader |
| Completion popup | Caret-anchored asynchronous providers, cancellation, stale-result rejection, replacement ranges, details, keyboard/pointer/accessibility activation, and insertion | Richer documentation and product/language-service integration | Rust core implemented; Swift broader |
| Documents | Stable buffer/view identities, canonical files, lifecycle decisions, synchronized shared text revisions, independent pane state, relocation/detachment, and retained caches | Live shared buffers with deeper product integration and richer transaction history | Swift moderately ahead |
| File lifecycle | Strict UTF-8 Open, Save, Save As, atomic replacement, duplicate activation, dirty-close resolution, and optimistic conflict handling | Real UTF-8 lifecycle with broader host/product integration | Swift moderately ahead |
| Native dialogs | Product-neutral contract plus Zenity/KDialog Linux adapter and deterministic scripted adapter | Host-owned cross-platform Open, Save As, dirty-close, and product integration | Swift ahead in breadth/platform coverage |
| Projects/workspaces | Open Folder, recursive snapshots, mutations, native-first watching, polling fallback, incremental subtree reconciliation, restoration, and file activation | Broader platform-native watchers, deeper product integration, and broader operations | Rust close in core runtime; Swift ahead in breadth |
| Split panes | Recursive horizontal/vertical trees, shared buffers, independent persisted view state, keyboard movement, splitters, focus, close/collapse, and accessibility | Richer docking, cross-window movement, and product integration | Rust core implemented; Swift ahead in breadth |
| Tab mechanics | Persisted pinned/preview/order/active state, overflow geometry, drag and keyboard reorder, cross-pane movement, and close controls | Richer docking and cross-window transfer | Rust core implemented; Swift ahead in breadth |
| Accessibility | Validated semantic trees and a concrete AccessKit bridge with fingerprinted update suppression | Broader semantic coverage because more widgets exist | Rust stronger bridge; Swift broader surface |
| Runtime scheduling | Typed invalidation, event-driven editor, retained animation lane | Persistent semantic scheduler and presentation deadlines | Different strengths |
| GPU renderer | Optional `wgpu` display-list backend and native proof host with batching, scissors, atlas upload, metrics, and recovery | Metal-oriented production path remains broader in paired applications | Rust proof path implemented; production integration still broader in Swift |
| Moth integration | Not started | Used by paired Moth convergence work | Swift far ahead |

## Where Rust is strongest

### Architectural spine

The foundational Swift module boundaries have direct Rust destinations:

```text
LunaCore             -> luna-core
LunaInput            -> luna-input
LunaLayout           -> luna-layout
LunaCommands         -> luna-commands
LunaTheme            -> luna-theme
LunaRender           -> luna-render + luna-render-wgpu
LunaAccessibility    -> luna-accessibility
native accessibility -> luna-accessibility-accesskit
LunaHostCore         -> luna-host-core
LunaHostSDL/Metal    -> luna-host-winit + luna-host-wgpu
LunaDocuments        -> luna-documents
file/dialog services  -> luna-document-services
workspace trees       -> luna-workspaces
persistent sessions    -> luna-session
LunaText             -> luna-text + luna-text-cosmic
LunaUI               -> luna-ui
```

The remaining gap is primarily feature breadth within those layers, not a missing architectural
layer.

### Scroll-time text reuse

M3.1b retains full logical document geometry and a vertically overscanned visible raster band. Caret,
selection, focus, and ordinary scrolling do not reshape the document. Crossing an overscan boundary
rerasterizes without reshaping. This is currently more advanced than Swift's accepted large-document
path, where viewport-bounded layout and shared revision-keyed presentation snapshots remain active
scalability targets.

### Native accessibility bridge

Rust translates validated stable-ID semantic trees into AccessKit. M3.1c adds deterministic semantic
fingerprints, shared immutable semantic snapshots, activation-aware translation, and unchanged-tree
suppression. Swift has more semantic widget types because it has more UI surfaces, while Rust has the
clearer concrete native bridge.

### CPU oracle and optional GPU proof path

The Rust CPU host retains both the ordinary working framebuffer and optional application static
layers. Animation frames can restore one dirty region from a retained base, draw only dynamic paint,
and skip unchanged accessibility translation. M4 adds an optional `wgpu` host consuming the same
immutable display-list layers, with headless scene-compilation tests and side-by-side runtime
selection. This strengthens rendering-boundary verification without claiming product-level GPU
integration parity.

## Where Swift remains ahead

M3.3c closes the reusable pane-session, arbitrary-depth menu, asynchronous completion-delivery,
search-history/options, scrollbar paging, and native-first incremental watcher gaps. M4 closes the
missing Rust GPU proof-backend gap and expands built-in palette coverage. Swift remains ahead in regex
and language-aware search, syntax services, richer command availability, docking and cross-window
movement, broader native platform adapters, and direct paired Moth integration. It also has
substantially broader phase-specific product regression coverage.

## Roadmap interpretation

The versions are not aligned linearly. Rust resembles Swift Phase 4D/early Phase 5A for visible
editor functionality, while selected performance work corresponds to later Swift host, cache, and
scalability phases.

```text
M3.1c -> complete incremental engine-performance groundwork
M3.1d -> first-level desktop dropdown menus and command-surface separation
M3.2a -> stable document identity, lifecycle decisions, and duplicate prevention
M3.2b -> UTF-8 file services, atomic saves, native dialogs, and close/conflict resolution
M3.2c -> recent files, storage snapshots, external-change delivery, and reload notices
M3.2d -> recursive workspace snapshots, Open Folder, expansion, refresh, and sidebar activation
M3.2e -> workspace operations, persistent sessions, shared-view identity, and watcher seams
M3.3a -> recursive split panes, synchronized shared text, pane-local state, and splitters
M3.3b -> advanced tabs, submenus, tab context, completion, find options, and scrollbar
M3.3c -> durable pane/view sessions, async providers, popup depth, search history, and native watchers
M4    -> optional wgpu renderer/host, clip parity, CPU/GPU fixtures, and four theme presets
M5    -> syntax spans, Sublime themes, undo/redo, multiple cursors, and IME
M6    -> direct Moth integration and platform hardening
```

M3.3c provides durable recursive panes, deep desktop popup behavior, asynchronous completion, search
history/options, and native-first incremental workspace refresh on top of the real file, workspace,
mutation, and session runtime. M4 adds the optional GPU proof path and four-palette visual matrix.
Richer language-aware editing, regex search, docking, broader platform adapters, production resource
retention, and direct Moth integration remain the major gaps.
