# Swift Luna UI Parity

This document records the functional relationship between Luna-UI-Rust and the original Swift
Luna UI implementation. It is a directional feature-inventory assessment, not a line-count or API
percentage guarantee.

## Current estimate after M3.3b

```text
foundational architecture       79–87% parity
core text/editing mechanics     68–77% parity
reusable editor UI              68–78% parity
file/workspace/pane integration 68–77% parity
total Swift functional surface  approximately 72–79%
```

Rust is no longer a framework skeleton. It has a native host, backend-neutral display lists, a CPU
renderer, real text shaping, editable Unicode geometry, accessibility, reusable editor anatomy,
typed invalidation, retained framebuffers, retained document layout/raster state, static gallery
retention, conditional semantic translation, functional first-level desktop dropdown menus,
a product-neutral document lifecycle model, real UTF-8 file operations, atomic optimistic saves,
native Linux dialog boundaries, a mutable recursive workspace runtime, persistent recent/workspace
session restoration, a live recursive split-pane runtime, advanced tab policy and movement, child
submenus, tab context menus, completion-popup foundations, richer find/replace, and an interactive
scrollbar. Swift remains broader and closer to a complete reusable editor platform.

## Capability matrix

| Area | Luna-UI-Rust after M3.3b | Swift Luna UI | Relative state |
|---|---|---|---|
| Core architecture | IDs, geometry, input, commands, layout, themes, rendering, accessibility, and host boundaries | Same broad spine with more downstream use | Near parity |
| CPU rendering | Display lists, images, alpha composition, retained host/static layers, dirty-region restore | CPU framebuffer and proof-frame caching | Near parity |
| Text shaping | cosmic-text fallback, bidi, grapheme and caret geometry | HarfBuzz/FreeType cluster geometry | Near parity |
| Text caching | Retained per-document logical layout and overscanned raster bands | Exact shaped rows; visible-row virtualization remains an active scalability target | Rust ahead in scroll-time raster reuse |
| Basic editing | Insertion, deletion, caret, selection, scrolling, hit testing, and editable semantics | Same foundation with deeper gesture/view integration | Mostly parity |
| Editor shell | Menu bar, pane-local tabs, recursive editor lanes, real workspace sidebar, and status bar | Richer docking, tab policy, and product integration | Swift moderately ahead |
| Command palette | Filtering, selection, activation, and accessibility | Unified command runtime, availability, and disabled-state behavior | Swift ahead |
| Find/replace | Literal search with case/whole-word options, replace-current/all, shared geometry, and accessibility | Regex, search history, broader options, and deeper product integration | Swift moderately ahead |
| Menus | Anchored parent/child panels, checked/disabled rows, shortcuts, mnemonics, pointer/keyboard navigation, and accessibility | Arbitrary-depth cascades, hover intent, and deeper command-state integration | Swift moderately ahead |
| Context menus | Tab-anchored context state reusing dropdown commands, geometry, pointer, keyboard, and accessibility | Broader product-neutral context definitions and routing | Rust foundation implemented; Swift broader |
| Completion popup | Caret-anchored list, details, keyboard/pointer/accessibility activation, and insertion payload | Async providers, replacement ranges, documentation, and product integration | Rust foundation implemented; Swift broader |
| Documents | Stable buffer/view identities, canonical files, lifecycle decisions, synchronized shared text revisions, independent pane state, relocation/detachment, and retained caches | Live shared buffers with deeper product integration and richer transaction history | Swift moderately ahead |
| File lifecycle | Strict UTF-8 Open, Save, Save As, atomic replacement, duplicate activation, dirty-close resolution, and optimistic conflict handling | Real UTF-8 lifecycle with broader host/product integration | Swift moderately ahead |
| Native dialogs | Product-neutral contract plus Zenity/KDialog Linux adapter and deterministic scripted adapter | Host-owned cross-platform Open, Save As, dirty-close, and product integration | Swift ahead in breadth/platform coverage |
| Projects/workspaces | Open Folder, recursive snapshots, create/rename/delete, collision policy, expansion, refresh, restoration, error projection, and file activation | Native watchers, incremental adapters, deeper product integration, and broader operations | Rust close in core runtime; Swift ahead in breadth |
| Split panes | Recursive horizontal/vertical pane trees, pane-local tabs, shared logical buffers, independent view state, draggable splitters, focus traversal, close/collapse, and accessibility | Richer pane movement, docking, persistence, and product integration | Rust core implemented; Swift ahead in breadth |
| Tab mechanics | Pinned/preview tabs, overflow geometry, active visibility, drag reordering, cross-pane movement, and close controls | Richer persistence, keyboard movement, docking, and cross-window transfer | Rust core implemented; Swift ahead in breadth |
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

M3.3b closes the basic advanced-tab, child-submenu, tab-context-menu, completion-popup, literal
find-option, and vertical-scrollbar gaps. Swift still implements arbitrary-depth popup behavior,
asynchronous completion providers, regex/search history, richer command availability, native event
watchers, incremental workspace adapters, pane/tab persistence, docking, and direct paired Moth
integration. It also has substantially broader phase-specific regression coverage.

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
M3.3c -> desktop persistence, async provider seams, popup depth, and native watchers
M4    -> wgpu renderer
M5    -> syntax spans, Sublime themes, undo/redo, multiple cursors, and IME
M6    -> direct Moth integration and platform hardening
```

M3.3b now provides live recursive panes plus the core desktop tab and popup surfaces on top of the
real file, workspace, mutation, and session runtime. Pane/tab persistence, arbitrary-depth popup
behavior, asynchronous completion providers, native watcher backends, incremental subtree refresh,
and direct Moth integration remain the major gaps.
