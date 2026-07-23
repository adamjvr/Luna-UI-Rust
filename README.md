# Luna-UI-Rust

**Luna-UI-Rust** is a clean Rust-native rewrite of Luna UI: the product-neutral UI, input,
layout, rendering, accessibility, command, text, and host foundation used to build editor-class
desktop applications such as Moth Text.

This is not a mechanical Swift-to-Rust translation. The rewrite preserves Luna's architectural
contracts while expressing them with Rust ownership, explicit errors, immutable frame snapshots,
small workspace crates, and strict compiler tooling.

## M2 status

M0 established the dependency-light deterministic core. M1 added the native desktop host. M2 adds
the first editor-grade text lane while keeping font-system state inside a narrow adapter:

- stable validated node IDs and saturating integer geometry;
- deterministic row, column, stack, and two-pane split layout snapshots;
- platform-neutral pointer, keyboard-layout text, IME commits, scroll, and focus events;
- typed command IDs, metadata, key bindings, and dispatch requests;
- immutable backend-neutral display lists;
- safe straight-alpha BGRA8 CPU rendering with clipped raster-image commands and fractional DPI;
- validated platform-neutral accessibility trees and an AccessKit bridge;
- deterministic line-plus-UTF-8 document positions and anchor/focus ranges;
- Unicode extended-grapheme movement and deletion;
- compact editable text state with selection replacement and vertical preferred-column motion;
- cosmic-text advanced shaping, fallback, bidi, ligatures, caret positions, hit geometry, and cached
  glyph rasterization;
- a reusable `TextView` whose pixels, caret, selection, hit testing, scrolling, and accessibility
  derive from one immutable shaped snapshot;
- headless, native workspace, and native editable-text proof applications;
- unit tests, strict lints, formatting policy, rustdoc checks, and CI.

The reusable document model remains independent of cosmic-text, winit, softbuffer, and AccessKit.
Those dependencies remain pinned in leaf adapter crates.

## Build and validate

Install the pinned toolchain through rustup, then run the complete quality gate:

```bash
cargo fmt --all
./scripts/validate.sh
```

Run the M2 editable-text proof:

```bash
cargo run -p luna-ui-rust-text-demo
```

Inside the text demo:

- type through winit keyboard-layout text and IME commit paths;
- use arrows, Shift-arrows, Home, End, Backspace, Delete, and Enter;
- use **Control-A** to select the document;
- click or drag to place the caret and create a selection;
- use the mouse wheel, Shift-wheel, or Page Up/Down to scroll vertically or horizontally;
- resize the window to exercise reshaping, clipping, DPI conversion, and caret reveal;
- press **Escape** or close the window to exit.

The earlier proofs remain available:

```bash
cargo run -p luna-ui-rust-native-demo
cargo run -p luna-ui-rust-demo -- ./luna-ui-rust-m0.ppm
```

## Governing architecture

```text
native event / AccessKit action
    -> platform-neutral Luna input or command request
    -> UTF-8/grapheme-safe application state update
    -> cosmic-text adapter creates immutable shaped snapshot
    -> TextView derives paint + hit testing + accessibility from that snapshot
    -> display list + semantic tree
    -> CPU renderer / AccessKit adapter
    -> softbuffer presentation
```

Widgets do not call graphics APIs, create native windows, own operating-system event loops, or
embed Moth-specific policy. See `docs/ARCHITECTURE.md`, `docs/PORTING_MAP.md`, and
`docs/M2_UPGRADE.md`.

## License

Mozilla Public License 2.0. Source files include SPDX identifiers and modifications to MPL-covered
files remain available under the MPL terms.
