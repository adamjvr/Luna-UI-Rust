# Swift Luna UI Parity

This document records the functional relationship between Luna-UI-Rust and the original Swift
Luna UI implementation. It is a directional feature-inventory assessment, not a line-count or API
percentage guarantee.

## Current estimate after M3.3a

```text
foundational architecture       78–86% parity
core text/editing mechanics     65–75% parity
reusable editor UI              60–70% parity
file/workspace/pane integration 62–72% parity
total Swift functional surface  approximately 68–74%
```

Rust is no longer a framework skeleton. It has a native host, backend-neutral display lists, a CPU
renderer, real text shaping, editable Unicode geometry, accessibility, reusable editor anatomy,
typed invalidation, retained framebuffers, retained document layout/raster state, static gallery
retention, conditional semantic translation, functional first-level desktop dropdown menus,
a product-neutral document lifecycle model, real UTF-8 file operations, atomic optimistic saves,
native Linux dialog boundaries, a mutable recursive workspace runtime, persistent recent/workspace
session restoration, and a live recursive split-pane runtime with synchronized shared document text.
Swift remains broader and closer to a complete reusable editor platform.

## Capability matrix

| Area | Luna-UI-Rust after M3.3a | Swift Luna UI | Relative state |
|---|---|---|---|
| Core architecture | IDs, geometry, input, commands, layout, themes, rendering, accessibility, and host boundaries | Same broad spine with more downstream use | Near parity |
| CPU rendering | Display lists, images, alpha composition, retained host/static layers, dirty-region restore | CPU framebuffer and proof-frame caching | Near parity |
| Text shaping | cosmic-text fallback, bidi, grapheme and caret geometry | HarfBuzz/FreeType cluster geometry | Near parity |
| Text caching | Retained per-document logical layout and overscanned raster bands | Exact shaped rows; visible-row virtualization remains an active scalability target | Rust ahead in scroll-time raster reuse |
| Basic editing | Insertion, deletion, caret, selection, scrolling, hit testing, and editable semantics | Same foundation with deeper gesture/view integration | Mostly parity |
| Editor shell | Menu bar, pane-local tabs, recursive editor lanes, real workspace sidebar, and status bar | Richer docking, tab policy, and product integration | Swift moderately ahead |
| Command palette | Filtering, selection, activation, and accessibility | Unified command runtime, availability, and disabled-state behavior | Swift ahead |
| Find/replace | Reusable panel geometry and state | Literal/regex search, options, replace-current, and replace-all | Swift well ahead |
| Menus | Functional first-level dropdowns, checked/disabled rows, shortcuts, pointer/keyboard navigation, and accessibility | First-level menus plus submenus and deeper command-state integration | Swift moderately ahead |
| Context menus | Not yet implemented | Product-neutral context definitions and routing | Missing in Rust |
| Completion popup | Not yet implemented | Anchored popup, details, keyboard/pointer activation, and result payloads | Missing in Rust |
| Documents | Stable buffer/view identities, canonical files, lifecycle decisions, synchronized shared text revisions, independent pane state, relocation/detachment, and retained caches | Live shared buffers with deeper product integration and richer transaction history | Swift moderately ahead |
| File lifecycle | Strict UTF-8 Open, Save, Save As, atomic replacement, duplicate activation, dirty-close resolution, and optimistic conflict handling | Real UTF-8 lifecycle with broader host/product integration | Swift moderately ahead |
| Native dialogs | Product-neutral contract plus Zenity/KDialog Linux adapter and deterministic scripted adapter | Host-owned cross-platform Open, Save As, dirty-close, and product integration | Swift ahead in breadth/platform coverage |
| Projects/workspaces | Open Folder, recursive snapshots, create/rename/delete, collision policy, expansion, refresh, restoration, error projection, and file activation | Native watchers, incremental adapters, deeper product integration, and broader operations | Rust close in core runtime; Swift ahead in breadth |
| Split panes | Recursive horizontal/vertical pane trees, pane-local tabs, shared logical buffers, independent view state, draggable splitters, focus traversal, close/collapse, and accessibility | Richer pane movement, docking, persistence, and product integration | Rust core implemented; Swift ahead in breadth |
| Tab mechanics | Pane-local tabs, active-view routing, close buttons, and shared-document views | Pinned/preview tabs, overflow geometry, reordering, movement, and active-tab visibility | Swift ahead |
| Accessibility | Validated semantic trees and a concrete AccessKit bridge with fingerprinted update suppression | Broader semantic coverage because more widgets exist | Rust stronger bridge; Swift broader surface |
| Runtime scheduling | Typed invalidation, event-driven editor, retained animation lane | Persistent semantic scheduler and presentation deadlines | Different strengths |
| GPU renderer | Planned wgpu backend | Production GPU path remains unfinished | Neither complete |
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
LunaRender           -> luna-render
LunaAccessibility    -> luna-accessibility
native accessibility -> luna-accessibility-accesskit
LunaHostCore         -> luna-host-core
LunaHostSDL/Metal    -> luna-host-winit
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

### Incremental CPU frame pipeline

The Rust host now retains both the ordinary working framebuffer and optional application static
layers. Animation frames can restore one dirty region from a retained base, draw only dynamic paint,
and skip unchanged accessibility translation.

## Where Swift remains ahead

M3.3a closes the basic recursive-pane and independent-view presentation gap, but Swift still
implements nested submenus, context menus, completion popups, richer command availability, richer
find/replace behavior, native event watchers, incremental workspace adapters, pane persistence and
movement, pinned/preview/overflow tabs, and direct paired Moth integration. It
also has substantially broader phase-specific regression coverage.

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
M3.3b -> advanced tabs, nested menus, context menus, and completion
M4    -> wgpu renderer
M5    -> syntax spans, Sublime themes, undo/redo, multiple cursors, and IME
M6    -> direct Moth integration and platform hardening
```

M3.3a now provides live recursive split panes on top of the real file, workspace, mutation, and
session runtime. Advanced tab policy and movement, native watcher backends, incremental subtree
refresh, nested/context menus, completion, and direct Moth integration remain the major gaps.
