# Architecture

## Purpose

Luna-UI-Rust preserves the Swift Luna UI contract while using Rust-native boundaries. Luna owns
reusable UI anatomy and deterministic runtime behavior. Applications such as Moth Text own editor
meaning, workflow, compatibility, settings policy, document storage policy, and product commands.

## Dependency direction

```text
luna-core
  ├── luna-input
  ├── luna-layout
  ├── luna-theme
  ├── luna-accessibility
  └── luna-render

luna-documents
  └── std path and collection contracts only

luna-document-services
  └── luna-documents

luna-workspaces
  └── std path, filesystem, and collection contracts only

luna-session
  └── std path, filesystem, and collection contracts only

luna-panes
  ├── luna-core
  └── luna-documents

luna-text
  └── unicode-segmentation

luna-text-cosmic
  ├── luna-text
  ├── luna-render
  └── cosmic-text

luna-render-wgpu
  ├── luna-core
  ├── luna-render
  ├── luna-theme
  └── wgpu

luna-host-core

luna-commands
  ├── luna-core
  └── luna-input

luna-ui
  ├── luna-core
  ├── luna-layout
  ├── luna-theme
  ├── luna-render
  ├── luna-accessibility
  ├── luna-documents
  ├── luna-panes
  ├── luna-text
  └── luna-text-cosmic

native leaf adapters
  ├── luna-accessibility-accesskit
  ├── luna-host-winit
  │     ├── winit
  │     ├── softbuffer
  │     └── AccessKit
  └── luna-host-wgpu
        ├── luna-host-winit contracts/input
        ├── luna-render-wgpu
        ├── winit + wgpu
        └── AccessKit

applications
  ├── luna-ui-rust-proof-gallery
  └── luna-ui-rust-editor-demo
        ├── luna-documents
        ├── luna-document-services
        ├── luna-workspaces
        ├── luna-session
        └── luna-panes
```

Dependencies point toward small product-neutral contracts. Platform integrations and stateful
third-party engines remain leaf adapters rather than foundations.

## Deterministic UI lane

Widget state mutation, command resolution, document edits, layout, paint-list generation, hit
testing, focus, and accessibility snapshot generation occur synchronously on one logical UI lane.
Background work may parse files, load resources, or compute results, but it crosses into the UI lane
through explicit messages or adapters. It never mutates widget state directly.

## Proof-gallery/editor split

The Swift Luna test application deliberately separates an animated proof gallery from its default
editor harness. M3 established that split, and M4 runs the same proof application through either
native renderer:

- the proof gallery continuously exercises reusable controls, responsive geometry, four theme
  presets, shaping, animation, clipping, DPI, hit testing, and accessibility through CPU or GPU
  presentation;
- the editor demo remains event-driven and exercises realistic shell composition, text editing,
  tabs, project navigation, command overlays, dirty state, and status updates.

Both applications consume the same `luna-ui` components. Neither application is itself a reusable
widget library, and neither is allowed to push demo-specific policy downward into Luna foundations.

## Scheduled logical updates

`NativeApplication::frame_interval` is optional. When absent, the host uses `ControlFlow::Wait` and
remains event-driven. When present, `about_to_wait` advances application-owned logical time at the
requested cadence through `ControlFlow::WaitUntil`. The update hook may request a redraw, but frame
building, rendering, AccessKit submission, and presentation still occur only in
`WindowEvent::RedrawRequested`.

This prevents proof animation from turning ordinary editor applications into polling loops.

## Document lifecycle boundary

`luna-documents` owns identity and lifecycle decisions without owning bytes, dialogs, watchers, or
editor views. Filesystem adapters must canonicalize platform paths before constructing a
`FileIdentity`; Luna does not infer symlink, case-folding, sandbox, or volume policy.

```text
filesystem/dialog/watcher adapter
    -> canonical FileIdentity + opaque StorageRevision
    -> DocumentRegistry
         -> stable DocumentId
         -> saved edit revision and dirty decision
         -> SaveRequirement
         -> CloseRequirement
         -> ExternalState
    -> application-owned EditableText view state
```

`DocumentRegistry` prevents duplicate file opens and reserves monotonic untitled names. A dirty
close produces `SaveOrDiscard`; it never silently chooses a product policy. Save produces a decision
(`None`, `SaveAs`, `WriteFile`, or `Unsupported`) that the application resolves through
`luna-document-services`. Caret, selection, scroll, split-pane state, and command policy remain in
the application layer.

## File and dialog service boundary

`luna-document-services` translates lifecycle decisions into bytes, storage observations, and user choices without moving
storage policy into `luna-documents` or widget policy into `luna-ui`.

```text
DocumentRegistry SaveRequirement / CloseRequirement
    -> application orchestration
         -> DocumentDialogService
              -> open path / save path
              -> Save / Discard / Cancel
              -> Overwrite / Reload / Cancel
         -> TextFileService
              -> strict UTF-8 read
              -> canonical FileIdentity
              -> deterministic StorageRevision
              -> optimistic WritePrecondition
              -> same-directory atomic replacement
    -> registry mark_saved / assign_file / remove
    -> editor view and cache invalidation
```

The standard adapter hashes file bytes into an opaque revision and compares that revision immediately
before replacement. A mismatch is a typed conflict, not an implicit overwrite. Save As uses a native
dialog that confirms replacement before the file service receives `WritePrecondition::Any`.
Ordinary Save uses `WritePrecondition::Matches` whenever the registry has a baseline storage snapshot.

Native dialogs are a leaf concern. `SystemDialogService` shells out directly to Zenity or KDialog on
Linux and passes arguments without a command shell. Tests use `MemoryTextFileService` and
`ScriptedDialogService`, so open/save/close/conflict behavior remains deterministic and does not
create windows or touch the host filesystem.

## Workspace and mutation boundary

`luna-workspaces` separates immutable observation from controlled filesystem mutation. The snapshot
model never performs byte I/O and the editor shell never calls `std::fs` directly.

```text
native folder dialog
    -> application-selected root path
    -> WorkspaceService::scan(root, options)
         -> exact stable WorkspaceNodeId values
         -> immutable WorkspaceSnapshot
         -> directory/file/symlink kind
         -> availability state and ordered children
    -> WorkspaceModel
         -> expansion / selection / reveal
         -> refresh reconciliation
    -> editor-shell SidebarItem projection

application mutation command
    -> DocumentDialogService product choice
    -> WorkspaceMutationService
         -> create file / create directory
         -> rename within current parent
         -> recursive delete
         -> explicit WorkspaceCollisionPolicy
    -> document/recent identity reconciliation
    -> fresh immutable workspace snapshot
```

The standard adapter canonicalizes scan roots, does not follow symlinks, sorts directories before
files, and preserves partial trees when child directories are unreadable. Mutation paths use
`symlink_metadata` so a symlink entry is renamed or deleted rather than its target. Replacement is
restricted to confirmed regular-file collisions; directories and symlinks are never replaced by
collision policy.

A workspace rename relocates every open file-backed document below the renamed path while retaining
its saved edit revision. Deletion resolves all affected dirty buffers before storage changes. A
buffer may be detached to a new Untitled identity, discarded and closed, or protect the complete
operation through Cancel.

The editor currently rescans once per second through `NativeApplication::update`. Snapshot comparison
and model mutation remain on the UI lane. `WorkspaceWatchService`, `WorkspaceWatchEvent`, and
`WorkspaceRefreshScope` establish the later native-watcher and subtree-reconciliation boundary;
background discovery still may not mutate editor state directly.

## Session persistence boundary

`luna-session` persists application restoration state without depending on UI, document, or
workspace-model crates.

```text
application state transition
    -> SessionState
         -> recent canonical paths and titles
         -> workspace root
         -> expanded directory paths
         -> selected workspace path
    -> SessionStore
         -> MemorySessionStore for deterministic tests
         -> StdSessionStore versioned atomic file
```

The standard Linux path follows `XDG_STATE_HOME` with a `$HOME/.local/state` fallback. Paths and
titles are encoded in a versioned text format; exact Unix path bytes are preserved. Session failure
is visible but non-fatal. Closing a workspace persists an empty workspace field, preventing an
unwanted restore on the next launch.

## Shared document/view boundary

A `DocumentId` identifies one buffer, file identity, dirty baseline, and storage observation.
`DocumentViewId` identifies one independent presentation of that buffer. `DocumentViewRegistry`
allows multiple views to reference the same document without duplicating lifecycle state.

Caret, selection, scrolling, folding, pane focus, and presentation caches remain view-owned. M3.3a
consumes this identity seam with synchronized per-view text snapshots. M3.3b adds pane-local pinned,
preview, order, and overflow metadata without moving lifecycle state out of `DocumentId`. An edit
commits one shared text revision to the canonical document and updates sibling views while
preserving/clamping their local caret and selection; scroll remains independent.


## Recursive pane boundary

`luna-panes` owns product-neutral topology and deterministic geometry, not document bytes or editor
commands.

```text
DocumentId lifecycle buffer
    -> one or more DocumentViewId records
    -> PaneTree
         -> Leaf: ordered pane-local views + active view
                  + pinned partition + preview view + tab offset
         -> Split: axis + ratio + two recursive children
    -> PaneLayoutSnapshot
         -> leaf/tab/editor rectangles
         -> splitter rectangles and containers
    -> luna-ui::EditorPaneSurface
         -> shared paint / hit / label / accessibility geometry
```

The application owns the synchronization transaction between the active view and canonical buffer.
Closing a leaf collapses its parent into the surviving sibling; the final pane is protected. Pane
focus, tab activation, tab dragging, overflow scrolling, pointer splitter dragging, and accessibility
actions all resolve through the same stable identities on the UI lane.

## Desktop popup boundary

`luna-ui` provides product-neutral geometry and semantics for dropdown submenus, context-menu
instances, completion lists, and find actions. Applications supply command definitions, popup state,
completion payloads, and search policy.

```text
application command/candidate/search state
    -> immutable popup definition
    -> shared panel/row/caret geometry
    -> paint + pointer + keyboard + accessibility routing
    -> application command or payload result
```

Only one editor transient surface is presented at a time. The editor demo enforces mutual exclusion
between the top-level dropdown, tab context menu, command palette, completion popup, and find panel.
Current dropdown presentation supports a parent panel plus one child submenu; arbitrary-depth
cascades remain a later extension.

## Text model boundary

`luna-text` owns durable semantic positions and editing behavior. A position is a logical line plus a
UTF-8 byte column. The public conversion boundary clamps arbitrary coordinates to valid scalar
boundaries, while user-visible movement and deletion advance by Unicode extended grapheme clusters.
The editable state still rebuilds snapshots after changes, but immutable `TextDocument` clones share
UTF-8 and line-index storage through `Arc`. The storage engine remains replaceable by a rope or piece
table without changing public coordinates.

`luna-text-cosmic` owns mutable font discovery, glyph caches, and retained per-document layout state.
A `TextLayoutCache` separates complete logical geometry from partial glyph pixels:

```text
document revision + width + typography
    -> retained cosmic-text buffer
    -> complete content size and caret/hit/selection geometry
    -> viewport-height raster band with vertical overscan
```

Caret, selection, focus, and overlays do not participate in logical cache keys. Foreground color
invalidates glyph pixels only. The immutable `TextLayoutSnapshot` records `raster_bounds` so widgets
paint a partial image in full document coordinates while hit testing, scrolling, selection, and
accessibility continue to use complete geometry. Widgets and render backends never retain a
cosmic-text borrow.

## Editor chrome retention

`TextLabelCache` is application-owned because shaping mutates the shared font system. Each stable
chrome slot retains at most one immutable text snapshot. Static menus and sidebar rows become cache
hits, while dynamic tab/status values replace their slot instead of growing an unbounded text-key
cache. Bounds and alignment remain widget properties and do not invalidate shaping.

## Command-surface projection

Applications own command meaning and current availability. Luna owns reusable presentation and
interaction. M3.1d uses one application command catalog to derive every command surface:

```text
application command catalog
    -> top-level dropdown definitions
    -> searchable command-palette items
    -> keyboard shortcut dispatch
    -> pointer and accessibility activation
    -> one application command executor
```

`DropdownMenuState` and `CommandPaletteState` remain independent. A menu-heading click opens an
anchored desktop dropdown; Ctrl+P opens a modal searchable palette. They may project the same command
ID without sharing query, selection, focus, geometry, or dismissal state. Disabled commands remain
visible in menus but are omitted from the current palette projection. Checked state is application
state projected into immutable menu definitions.

Dropdown row geometry drives paint, pointer hit testing, and accessibility. Menu open/close and row
navigation invalidate overlay paint only; they never participate in document-layout or glyph-raster
cache keys. Moth-specific command policy remains above Luna.

## Immutable frame snapshots

A frame contains data, not callbacks into widget state. Either renderer consumes a dynamic
`DisplayList` and may also consume a shared `RetainedDisplayList`. The accessibility adapter consumes
a validated `AccessibilityTree` shared through `Arc`. Large immutable raster images, static paint
lists, and semantic trees therefore cross frame boundaries without copying every glyph pixel, paint
command, node, or string.

A retained paint layer carries an application-owned revision and logical dirty region:

```text
static display list revision
    -> CPU host-retained static framebuffer
         -> full restore after revision/size/scale change
         -> clipped dirty-region restore on dynamic-only frames
    -> GPU retained immutable display-list layer
         -> ordered quad/scissor/atlas compilation with dynamic paint
    -> dynamic display list
```

The revision must change whenever static paint changes. Dynamic commands must remain inside the
declared dirty region. This explicit contract permits deterministic tests, CPU/GPU parity,
recording/replay, caching, and clear synchronization boundaries without making the host understand
application widgets.

## Geometry invariant

For any interactive widget:

```text
paint geometry == hit-test geometry == accessibility geometry
```

Editor shell rows, proof cards, overlay fields, and text surfaces expose immutable layout snapshots.
Application code shapes labels into those exact rectangles instead of independently reconstructing
chrome geometry.

## Native host lifecycle

Both native hosts create windows only after `resumed`, create the AccessKit adapter before making the
window visible, normalize native input through `WinitInputTranslator`, schedule optional logical
updates, and request frames through typed invalidations. Each builds the same immutable `UiFrame`.

The CPU host reuses its working framebuffer, optionally retains a separately rasterized static
framebuffer, restores declared dirty regions, converts directly into softbuffer storage, and resizes
only after physical-size changes. The GPU host preserves retained and dynamic display lists as
separate painter-ordered layers, compiles them into solid/image quad batches, uploads one bounded
per-frame BGRA atlas, applies physical scissor rectangles, and presents through a `wgpu` surface.

Surface resize, suboptimal, outdated, timeout, occlusion, and loss outcomes remain host concerns.
Surface loss or a device-loss callback rebuilds GPU resources while leaving application state,
semantic IDs, and command state intact. `luna-host-wgpu` does not become a second application runtime;
it is a leaf presentation adapter for the same `NativeApplication` contract.

`AccessibilityTree` computes a deterministic fixed-endian field fingerprint during validation. Both
hosts track AccessKit activation and translate semantics only after fingerprint or scale changes.
Initial activation always receives a complete current tree, and deactivation clears translated
snapshot state. CPU and GPU stage timings make build, rendering, presentation, retained-scene,
atlas/batch, recovery, and semantic cost visible. Application errors propagate to the caller rather
than being hidden.

## Swift parity boundary

The architectural spine is near parity with Swift Luna UI, but feature breadth is not. Rust currently
has stronger retained editor-raster, concrete AccessKit, and optional cross-platform `wgpu` proof
paths. M3.3c closes recursive popup, asynchronous provider, pane-session, and native-first watcher
gaps. M4 adds backend-neutral clip commands, the GPU renderer/host leaf adapters, CPU/GPU comparison,
and a four-preset theme matrix. Swift remains ahead in richer language services, platform breadth,
docking/cross-window behavior, and paired Moth integration. [`SWIFT_PARITY.md`](SWIFT_PARITY.md) is
the governing inventory; rendering milestones must not be described as equivalent to editor-product
feature parity.

## Safety policy

The workspace uses `unsafe_code = "forbid"`. The M4 `wgpu` renderer and host remain entirely within
safe Rust. If a future FFI integration requires unsafe code, isolate it in a tiny adapter crate,
state the invariant in a `SAFETY:` comment, add boundary tests, and never expose an unsafe requirement
to ordinary widget or application code.


## M3.3c persistence and delivery pipeline

```text
session V2 decode
    -> document/source/storage-baseline validation
    -> shared DocumentId reconstruction
    -> independent DocumentViewId caret/selection/scroll reconstruction
    -> recursive PaneTree restore and preview normalization

native inotifywait or polling fallback
    -> path-level event coalescing
    -> full or smallest-safe-subtree refresh scope
    -> immutable WorkspaceSnapshot reconciliation
    -> UI-thread WorkspaceModel refresh

completion request
    -> monotonic request ID + view/revision context
    -> provider-owned worker delivery
    -> UI-thread channel drain
    -> stale ID/view/revision rejection
    -> explicit candidate replacement range
```

The persisted wire format remains isolated in `luna-session`; pane validity remains isolated in
`luna-panes`; native watcher process and polling details remain isolated in `luna-workspaces`; and
applications continue to own policy and all UI-thread mutation.
