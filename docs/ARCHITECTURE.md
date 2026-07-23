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

## Text model boundary

`luna-text` owns durable semantic positions and editing behavior. A position is a logical line plus a
UTF-8 byte column. The public conversion boundary clamps arbitrary coordinates to valid scalar
boundaries, while user-visible movement and deletion advance by Unicode extended grapheme clusters.
The current `String`-backed editor state is intentionally replaceable by a rope or piece table
without changing those public coordinates.

`luna-text-cosmic` owns mutable font discovery and glyph caches. It consumes an immutable
`TextDocument` and produces an immutable `TextLayoutSnapshot` containing shared BGRA8 glyph pixels,
caret stops, hit geometry, selection geometry, visible ranges, and content extents. Widgets and
render backends never retain a cosmic-text borrow.

## Immutable frame snapshots

A frame contains data, not callbacks into widget state. The renderer consumes a `DisplayList`; the
accessibility adapter consumes an `AccessibilityTree`. Large immutable raster images use shared
storage so cloning frame commands does not copy every glyph pixel. This permits deterministic tests,
CPU/GPU parity, recording/replay, caching, and clear synchronization boundaries.

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
window visible, normalizes native input, schedules optional logical updates, requests frames through
`FrameRuntime`, builds one immutable `UiFrame`, scales and paints it through the CPU renderer,
presents through softbuffer, and submits a matching AccessKit update. Application errors propagate
to the caller rather than being hidden.

## Safety policy

The workspace uses `unsafe_code = "forbid"`. If future FFI or GPU integration requires unsafe code,
isolate it in a tiny adapter crate, state the invariant in a `SAFETY:` comment, add boundary tests,
and never expose an unsafe requirement to ordinary widget or application code.
