# Architecture

## Purpose

Luna-UI-Rust preserves the Swift Luna UI contract while using Rust-native boundaries. Luna owns
reusable UI anatomy and deterministic runtime behavior. Applications such as Moth Text own editor
meaning, workflow, compatibility, settings policy, and product commands.

## Dependency direction

```text
luna-core
  ├── luna-input
  ├── luna-layout
  ├── luna-theme
  ├── luna-accessibility
  └── luna-render

luna-host-core

luna-commands
  ├── luna-core
  └── luna-input

luna-ui
  ├── luna-core
  ├── luna-layout
  ├── luna-theme
  ├── luna-render
  └── luna-accessibility

native leaf adapters
  ├── luna-accessibility-accesskit
  └── luna-host-winit
        ├── winit
        ├── softbuffer
        └── AccessKit

future leaf adapters
  ├── luna-render-wgpu
  └── luna-text-cosmic
```

Dependencies point toward small product-neutral contracts. Native and third-party integrations are
leaf crates, never foundations.

## Deterministic UI lane

Widget state mutation, command resolution, layout, paint-list generation, hit testing, focus, and
accessibility snapshot generation occur synchronously on one logical UI lane. Background work may
parse files, load resources, or compute results, but it crosses into the UI lane through explicit
messages or adapters. It never mutates widget state directly.

## Immutable frame snapshots

A frame contains data, not callbacks into widget state. The renderer consumes a `DisplayList`; the
accessibility adapter consumes an `AccessibilityTree`. This permits:

- deterministic unit tests;
- CPU and GPU renderers using the same paint contract;
- recording and replay tooling;
- frame caching and future diffing;
- clear synchronization boundaries;
- host-specific accessibility without host-specific widgets.

## Geometry invariant

For any interactive widget:

```text
paint bounds == hit-test bounds == accessibility bounds
```

Composite widgets may have child bounds, but all three systems consume the same immutable layout
snapshot. Geometry must not be independently re-derived in renderer, host, or accessibility code.

M1 applies DPI conversion only at the native leaf boundary. Luna layout and hit testing remain in
integer logical pixels; rendering and AccessKit bounds scale from that shared logical snapshot.

## Native host lifecycle

The winit host creates windows only after `resumed`, creates the AccessKit adapter before making the
window visible, normalizes native input, requests frames through `FrameRuntime`, builds one immutable
`UiFrame`, scales and paints it through the CPU renderer, presents through softbuffer, and submits a
matching AccessKit update. Application errors are propagated to the caller rather than hidden.

## Commands

Commands are stable typed intent, not closures stored in widgets. `CommandRegistry` resolves
normalized keyboard events into immutable `CommandRequest` values. Applications execute the
behavior. This avoids hidden ownership, lifetime, and thread-affinity requirements in reusable UI.

## Safety policy

The workspace uses `unsafe_code = "forbid"`. Native adapters remain safe because the selected
upstream APIs expose safe constructors and presentation methods. If future FFI requires unsafe code,
isolate it in a tiny adapter crate, state the invariant in a `SAFETY:` comment, add boundary tests,
and never expose an unsafe requirement to ordinary widget or application code.
