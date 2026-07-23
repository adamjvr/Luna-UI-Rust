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

future leaf adapter
  └── luna-render-wgpu
```

Dependencies point toward small product-neutral contracts. Platform integrations and stateful
third-party engines remain leaf adapters rather than foundations.

## Deterministic UI lane

Widget state mutation, command resolution, document edits, layout, paint-list generation, hit
testing, focus, and accessibility snapshot generation occur synchronously on one logical UI lane.
Background work may parse files, load resources, or compute results, but it crosses into the UI lane
through explicit messages or adapters. It never mutates widget state directly.

## Text model boundary

`luna-text` owns durable semantic positions and editing behavior. A position is a logical line plus a
UTF-8 byte column. The public conversion boundary clamps arbitrary coordinates to valid scalar
boundaries, while user-visible movement and deletion advance by Unicode extended grapheme clusters.
The current `String`-backed editor state is intentionally replaceable by a rope or piece table
without changing those public coordinates.

`luna-text-cosmic` owns mutable font discovery and glyph caches. It consumes an immutable
`TextDocument` and produces an immutable `TextLayoutSnapshot` containing shared BGRA8 glyph pixels,
caret stops, hit geometry, selection geometry, visible ranges, and content extents. Neither widgets
nor render backends retain a cosmic-text borrow.

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

For text, the same shaped snapshot supplies glyph placement, caret stops, selection rectangles,
pointer mapping, viewport ranges, and semantic line bounds. Hosts and renderers may transform the
snapshot for DPI, but they may not independently reshape or reinterpret the document.

## Alpha and image contract

Raster images are tightly packed straight-alpha BGRA8. CPU source-over composition preserves
straight color in transparent intermediate glyph images, preventing antialiased text from being
alpha-multiplied twice. Display-list image clips are explicit and a disjoint clip draws nothing.

## Native host lifecycle

The winit host creates windows only after `resumed`, creates the AccessKit adapter before making the
window visible, normalizes native input, requests frames through `FrameRuntime`, builds one immutable
`UiFrame`, scales and paints it through the CPU renderer, presents through softbuffer, and submits a
matching AccessKit update. Application errors propagate to the caller rather than being hidden.

## Safety policy

The workspace uses `unsafe_code = "forbid"`. If future FFI or GPU integration requires unsafe code,
isolate it in a tiny adapter crate, state the invariant in a `SAFETY:` comment, add boundary tests,
and never expose an unsafe requirement to ordinary widget or application code.
