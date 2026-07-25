# Swift Luna UI Parity

This document records the functional relationship between Luna-UI-Rust and the original Swift
Luna UI implementation. It is a directional feature-inventory assessment, not a line-count or API
percentage guarantee.

## Current estimate after M3.2b

```text
foundational architecture       72–82% parity
core text/editing mechanics     65–75% parity
reusable editor UI              48–58% parity
file/workspace/pane integration 30–40% parity
total Swift functional surface  approximately 57–62%
```

Rust is no longer a framework skeleton. It has a native host, backend-neutral display lists, a CPU
renderer, real text shaping, editable Unicode geometry, accessibility, reusable editor anatomy,
typed invalidation, retained framebuffers, retained document layout/raster state, static gallery
retention, conditional semantic translation, functional first-level desktop dropdown menus,
a product-neutral document lifecycle model, real UTF-8 file operations, atomic optimistic saves,
and native Linux dialog boundaries. Swift remains broader and closer to a complete reusable editor
platform.

## Capability matrix

| Area | Luna-UI-Rust after M3.2b | Swift Luna UI | Relative state |
|---|---|---|---|
| Core architecture | IDs, geometry, input, commands, layout, themes, rendering, accessibility, and host boundaries | Same broad spine with more downstream use | Near parity |
| CPU rendering | Display lists, images, alpha composition, retained host/static layers, dirty-region restore | CPU framebuffer and proof-frame caching | Near parity |
| Text shaping | cosmic-text fallback, bidi, grapheme and caret geometry | HarfBuzz/FreeType cluster geometry | Near parity |
| Text caching | Retained per-document logical layout and overscanned raster bands | Exact shaped rows; visible-row virtualization remains an active scalability target | Rust ahead in scroll-time raster reuse |
| Basic editing | Insertion, deletion, caret, selection, scrolling, hit testing, and editable semantics | Same foundation with deeper gesture/view integration | Mostly parity |
| Editor shell | Menu-bar anatomy, tabs, sidebar, editor lane, and status bar | Richer shell interaction and workspace projection | Swift ahead |
| Command palette | Filtering, selection, activation, and accessibility | Unified command runtime, availability, and disabled-state behavior | Swift ahead |
| Find/replace | Reusable panel geometry and state | Literal/regex search, options, replace-current, and replace-all | Swift well ahead |
| Menus | Functional first-level dropdowns, checked/disabled rows, shortcuts, pointer/keyboard navigation, and accessibility | First-level menus plus submenus and deeper command-state integration | Swift moderately ahead |
| Context menus | Not yet implemented | Product-neutral context definitions and routing | Missing in Rust |
| Completion popup | Not yet implemented | Anchored popup, details, keyboard/pointer activation, and result payloads | Missing in Rust |
| Documents | Stable IDs, canonical file identity, duplicate prevention, untitled lifecycle, dirty/save/close decisions, external state, and retained text caches | Shared buffers, independent views, complete adapters, and product integration | Swift ahead |
| File lifecycle | Strict UTF-8 Open, Save, Save As, atomic replacement, duplicate activation, dirty-close resolution, and optimistic conflict handling | Real UTF-8 lifecycle with broader host/product integration | Swift moderately ahead |
| Native dialogs | Product-neutral contract plus Zenity/KDialog Linux adapter and deterministic scripted adapter | Host-owned cross-platform Open, Save As, dirty-close, and product integration | Swift ahead in breadth/platform coverage |
| Projects/workspaces | Demonstration sidebar rows | Workspace/project adapters and tree snapshots | Missing in Rust |
| Split panes | Planned | Recursive pane trees, divider dragging, focus traversal, and pane-bound views | Missing in Rust |
| Tab mechanics | Basic document tabs | Pinned tabs, overflow geometry, and active-tab visibility | Swift ahead |
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

M3.2b closes much of the basic file/dialog lifecycle gap, but Swift still implements nested
submenus, context menus, completion popups, richer command availability, richer find/replace
behavior, continuous external-change delivery, recent files, workspace/project adapters, recursive
split panes, independent pane presentation state, pinned/overflow tabs, and direct paired Moth
integration. It also has
substantially broader phase-specific regression coverage.

## Roadmap interpretation

The versions are not aligned linearly. Rust resembles Swift Phase 4D/early Phase 5A for visible
editor functionality, while selected performance work corresponds to later Swift host, cache, and
scalability phases.

```text
M3.1c -> complete incremental engine-performance groundwork
M3.1d -> first-level desktop dropdown menus and command-surface separation
M3.2a -> stable document identity, lifecycle decisions, and duplicate prevention
M3.2b -> UTF-8 file services, atomic saves, native dialogs, and close/conflict resolution
M3.2c+ -> recent files, external-change delivery, workspace adapters, and independent views
M3.3  -> split panes, advanced tabs, nested menus, context menus, and completion
M4    -> wgpu renderer
M5    -> syntax spans, Sublime themes, undo/redo, multiple cursors, and IME
M6    -> direct Moth integration and platform hardening
```

The largest visible parity gain will still occur in later M3.2 and M3.3 work. M3.2b now provides
real file persistence and dialog behavior; workspace trees, watcher delivery, shared buffers, and
panes remain the major application-integration gaps.
