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

luna-text
  └── unicode-segmentation

luna-text-cosmic
  ├── luna-text
  ├── luna-render
  └── cosmic-text

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
  ├── luna-text
  └── luna-text-cosmic

native leaf adapters
  ├── luna-accessibility-accesskit
  └── luna-host-winit
        ├── winit
        ├── softbuffer
        └── AccessKit

applications
  ├── luna-ui-rust-proof-gallery
  └── luna-ui-rust-editor-demo
        └── luna-documents
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
editor harness. M3 preserves that split:

- the proof gallery continuously exercises reusable controls, responsive geometry, themes, shaping,
  animation, clipping, DPI, hit testing, and accessibility;
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
(`None`, `SaveAs`, `WriteFile`, or `Unsupported`) that later adapters execute. Caret, selection,
scroll, split-pane state, and command policy remain in the application layer.

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

A frame contains data, not callbacks into widget state. The renderer consumes a dynamic
`DisplayList` and may also consume a shared `RetainedDisplayList`. The accessibility adapter consumes
a validated `AccessibilityTree` shared through `Arc`. Large immutable raster images, static paint
lists, and semantic trees therefore cross frame boundaries without copying every glyph pixel, paint
command, node, or string.

A retained paint layer carries an application-owned revision and logical dirty region:

```text
static display list revision
    -> host-retained static framebuffer
    -> full restore after revision/size/scale change
    -> clipped dirty-region restore on dynamic-only frames
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

The winit host creates windows only after `resumed`, creates the AccessKit adapter before making the
window visible, normalizes native input, schedules optional logical updates, and requests frames
through typed invalidations. It builds one immutable `UiFrame`, reuses the ordinary working
framebuffer, optionally retains a separately rasterized static framebuffer, converts directly into
softbuffer storage, and resizes the surface only after physical-size changes.

For a retained frame, the host rerasterizes static paint only after revision, physical-size, or scale
changes. Otherwise it restores the declared dirty rectangle and executes only dynamic commands.

`AccessibilityTree` computes a deterministic fixed-endian field fingerprint during validation.
The host tracks AccessKit activation and translates semantics only after fingerprint or scale
changes. Initial activation always receives a complete current tree, and deactivation clears the
translated-snapshot state. Stage
timings and lifetime/cache counters make build, rendering, presentation, retained-scene, and semantic
cost visible. Application errors propagate to the caller rather than being hidden.

## Swift parity boundary

The architectural spine is near parity with Swift Luna UI, but feature breadth is not. Rust currently
has stronger retained editor-raster and concrete AccessKit paths, while Swift remains substantially
ahead in nested submenus, context menus, completion, real files/workspaces, split panes, advanced
tabs, and paired Moth integration. [`SWIFT_PARITY.md`](SWIFT_PARITY.md) is the governing inventory;
performance milestones must not be described as equivalent to editor-product feature parity.

## Safety policy

The workspace uses `unsafe_code = "forbid"`. If future FFI or GPU integration requires unsafe code,
isolate it in a tiny adapter crate, state the invariant in a `SAFETY:` comment, add boundary tests,
and never expose an unsafe requirement to ordinary widget or application code.
