# Luna-UI-Rust

**Luna-UI-Rust** is a clean Rust-native rewrite of Luna UI: the product-neutral UI, input,
layout, rendering, accessibility, command, and host foundation used to build editor-class desktop
applications such as Moth Text.

This is not a mechanical Swift-to-Rust translation. The rewrite preserves Luna's architectural
contracts while expressing them with Rust ownership, explicit errors, immutable frame snapshots,
small workspace crates, and strict compiler tooling.

## M1 status

M0 established the dependency-light deterministic core. M1 adds the first native desktop path while
keeping operating-system and third-party types in leaf adapters:

- stable validated node IDs and saturating integer geometry;
- deterministic row, column, stack, and two-pane split layout snapshots;
- platform-neutral pointer, keyboard, text, scroll, and focus events;
- typed command IDs, metadata, key bindings, and dispatch requests;
- strongly typed theme colors;
- immutable backend-neutral display lists;
- safe BGRA8 CPU rendering with fractional-DPI scaling;
- validated platform-neutral accessibility trees;
- stable AccessKit node-ID translation and full-tree updates;
- deterministic frame invalidation/runtime state;
- a product-neutral `Widget` contract and composite workspace fixture;
- a winit `ApplicationHandler` host with softbuffer presentation;
- a headless PPM demo and a live native desktop demo;
- unit tests, strict lints, formatting policy, rustdoc checks, and CI.

The reusable Luna core remains free of winit, softbuffer, and AccessKit. Those dependencies are
pinned only in native adapter crates.

## Build and validate

Install the pinned toolchain through rustup, then run the complete quality gate:

```bash
./scripts/validate.sh
```

Run the live M1 desktop proof:

```bash
cargo run -p luna-ui-rust-native-demo
```

Inside the native demo:

- press **Control-P** to dispatch a typed command and toggle the sidebar treatment;
- click the sidebar command button to exercise shared hit-test geometry;
- inspect the window with a platform accessibility tool to exercise the AccessKit tree;
- press **Escape** or close the window to exit.

The original headless proof remains available:

```bash
cargo run -p luna-ui-rust-demo -- ./luna-ui-rust-m0.ppm
```

## Governing architecture

```text
native winit event / AccessKit action
    -> platform-neutral Luna input or command request
    -> deterministic application state update
    -> shared immutable layout snapshot
    -> display list + hit testing + accessibility tree
    -> CPU renderer / AccessKit adapter
    -> softbuffer presentation
```

Widgets do not call graphics APIs, create native windows, own operating-system event loops, or
embed Moth-specific policy. See `docs/ARCHITECTURE.md` and `docs/PORTING_MAP.md`.

## License

Mozilla Public License 2.0. Source files include SPDX identifiers and modifications to MPL-covered
files remain available under the MPL terms.
