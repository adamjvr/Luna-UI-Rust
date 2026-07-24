# Luna-UI-Rust

**Luna-UI-Rust** is a clean Rust-native rewrite of Luna UI: the product-neutral UI, input,
layout, rendering, accessibility, command, text, and native-host foundation used to build
editor-class desktop applications such as Moth Text.

This is not a mechanical Swift-to-Rust translation. The rewrite preserves Luna's architectural
contracts while expressing them through Rust ownership, explicit errors, immutable frame snapshots,
small workspace crates, strict compiler tooling, and platform-specific leaf adapters.

## M3.1 status

M0 established the deterministic core, M1 added the native host, M2 added editor-grade text, and M3
added the twin proof-gallery/editor applications plus reusable editor anatomy. M3.1 makes the CPU
path incremental and completes the first functional editor-surface correction pass before real
files/workspaces and GPU rendering.

M3.1a through M3.1c are complete, validated, and committed. They provide retained host buffers,
typed invalidation, frame-stage metrics, shared immutable document snapshots, retained editor text
layout and raster bands, bounded chrome labels, retained proof-gallery static scenes, coalesced
pointer motion, deterministic accessibility fingerprints, and conditional AccessKit translation.

M3.1d adds real desktop dropdown menus and separates them from the command palette. M3.1d.1
corrects the native integration after runtime testing showed that menu-heading clicks could still be
consumed by transient command-palette handling:

- reusable `MenuDefinition`, `MenuCommand`, `MenuItem`, `DropdownMenuState`, and `DropdownMenu`
  contracts;
- one application-owned command catalog projected into File/Edit/Find/View/Help dropdowns and the
  searchable command palette;
- top-level menu-heading clicks receive priority over every open transient surface;
- opening a dropdown forcibly closes palette and find state before the frame is built;
- the command palette is opened only by Control-P and is not projected as a dropdown row;
- independent dropdown, command-palette, and find-panel state;
- anchored and viewport-clamped menu geometry;
- separators, shortcut labels, disabled commands, and checked commands;
- pointer opening, outside-click dismissal, hover selection, and menu-heading traversal;
- Up/Down/Home/End navigation, Left/Right menu switching, Enter/Space activation, and Escape
  dismissal;
- shared paint, hit-test, and accessibility geometry for every visible menu row;
- accessibility menu/menu-item semantics with disabled, checked, focused, and expanded state;
- precise overlay invalidation without document reshaping.

M3.2 is next and begins the largest functional catch-up toward Swift Luna UI: real document/file
lifecycle, shared views, project/workspace adapters, Save As, conflict state, and watcher boundaries.
See `docs/SWIFT_PARITY.md` for the current feature comparison.

## Build and validate

Install the pinned Rust toolchain through rustup, then run the complete quality gate:

```bash
cargo fmt --all
./scripts/validate.sh
```

Run the proof gallery in release mode:

```bash
cargo run --release -p luna-ui-rust-proof-gallery
```

The M3.1c gallery baseline must remain intact: ordinary animation hits retained layout, static-paint,
semantic, and label caches; restores only the animation lane; skips unchanged accessibility
translation; and produces no frame while pointer motion remains inside one semantic hover target.

Run the event-driven editor integration harness:

```bash
cargo run --release -p luna-ui-rust-editor-demo
```

Editor menu behavior:

- click **File**, **Edit**, **Find**, **View**, or **Help** to open its anchored dropdown;
- click another heading, or move across headings while a dropdown is open, to switch menus;
- use Up/Down/Home/End to select commands and Left/Right to switch menus;
- use Enter or Space to activate and Escape or an outside click to dismiss;
- disabled commands remain visible but cannot activate;
- checked commands expose current sidebar/theme state;
- **Control-P** opens the separate searchable command palette and never opens from an ordinary
  menu-heading click.

Editor shortcuts:

- **Control-P** — command palette;
- **Control-F** — find panel;
- **Control-H** — find/replace panel;
- **Control-S** — mark the active document saved;
- **Control-N** — create a new document;
- **Control-B** — toggle the sidebar;
- **Control-W** — close the active tab when more than one is open;
- **Control-A** — select all editor text;
- **Escape** — close the active menu/overlay, or exit when none is open.

The editor must preserve M3.1b performance behavior: no idle frames, no reshaping for caret,
selection, focus, or menu changes, overscanned raster reuse during ordinary scrolling, and bounded
shell/menu-label cache entries.

The M2 text demo and earlier proofs remain available:

```bash
cargo run -p luna-ui-rust-text-demo
cargo run -p luna-ui-rust-native-demo
cargo run -p luna-ui-rust-demo -- ./luna-ui-rust-m0.ppm
```

## Governing architecture

```text
native event / AccessKit action / scheduled logical update
    -> precise invalidation class
    -> application command catalog
         -> dropdown-menu projection
         -> command-palette projection
         -> keyboard/accessibility dispatch
    -> platform-neutral Luna application state
    -> retained text/layout/chrome or retained static scene
    -> dynamic display list + shared validated semantic snapshot
    -> retained working/static CPU framebuffers + conditional AccessKit translation
    -> size-gated softbuffer presentation
```

Widgets do not call graphics APIs, create native windows, own operating-system event loops, or embed
Moth-specific product policy. See `docs/ARCHITECTURE.md`, `docs/PORTING_MAP.md`,
`docs/M3_1A_HOST_PIPELINE.md`, `docs/M3_1B_TEXT_CACHE.md`,
`docs/M3_1C_GALLERY_ACCESSIBILITY.md`, `docs/M3_1D_DROPDOWN_MENUS.md`, and
`docs/SWIFT_PARITY.md`.

## License

Mozilla Public License 2.0. Source files include SPDX identifiers and modifications to MPL-covered
files remain available under the MPL terms.
