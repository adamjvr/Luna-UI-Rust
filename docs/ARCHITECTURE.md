# Architecture

## Purpose

Luna-UI-Rust preserves the Swift Luna UI contract while using Rust-native boundaries. Luna owns
reusable UI anatomy and deterministic runtime behavior. Applications such as Moth Text own editor
meaning, workflow, compatibility, settings policy, and product commands.

## Dependency direction

```text
luna-core
  ├── luna-input
  ├── luna-theme
  ├── luna-accessibility
  └── luna-render

luna-host-core

luna-ui
  ├── luna-core
  ├── luna-theme
  ├── luna-render
  └── luna-accessibility

native adapters (future)
  ├── luna-host-winit
  ├── luna-render-wgpu
  ├── luna-text-cosmic
  └── luna-accessibility-accesskit
```

Dependencies point toward small product-neutral contracts. Native and third-party integrations are
leaf crates, not foundations.

## Deterministic UI lane

Widget state mutation, layout, paint-list generation, hit testing, focus, and accessibility snapshot
generation occur synchronously on one logical UI lane. Background work may parse files, load
resources, or compute results, but it crosses into the UI lane through explicit messages/adapters.
It never mutates widget state directly.

## Immutable frame snapshots

A frame contains data, not callbacks into widget state. The renderer consumes a `DisplayList`; the
accessibility adapter consumes an `AccessibilityTree`. This permits:

- deterministic unit tests;
- CPU and GPU renderers using the same paint contract;
- recording/replay tooling;
- frame caching;
- clear synchronization boundaries;
- host-specific accessibility without host-specific widgets.

## Geometry invariant

For any interactive widget:

```text
paint bounds == hit-test bounds == accessibility bounds
```

Composite widgets may have child bounds, but all three systems must consume the same computed
layout snapshot. Geometry must not be independently re-derived in renderer or accessibility code.

## Safety policy

The workspace begins with `unsafe_code = "forbid"`. Native and GPU crates should remain safe where
upstream APIs permit. If future FFI requires unsafe code, isolate it in a tiny adapter crate, state
the invariant in a `SAFETY:` comment, add boundary tests, and never expose an unsafe requirement to
ordinary widget/application code.
