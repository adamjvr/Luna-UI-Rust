# Luna-UI-Rust

**Luna-UI-Rust** is a clean Rust-native rewrite of Luna UI: the product-neutral UI, input,
layout, rendering, accessibility, command, text, and native-host foundation used to build
editor-class desktop applications such as Moth Text.

This is not a mechanical Swift-to-Rust translation. The rewrite preserves Luna's architectural
contracts while expressing them through Rust ownership, explicit errors, immutable frame snapshots,
small workspace crates, strict compiler tooling, and platform-specific leaf adapters.

## M3 status

M0 established the deterministic core, M1 added the native host, and M2 added editor-grade text.
M3 ports the Swift project's deliberate split between a proof gallery and an editor integration
harness while moving their reusable anatomy into `luna-ui`:

- reusable buttons, toggles, progress indicators, cards, and immutable shaped labels;
- a responsive proof-gallery card system with deterministic logical animation time;
- editor shell geometry for menus, tabs, sidebar/project rows, editor lane, and status bar;
- reusable command-palette and find/replace overlays;
- expanded semantic roles for menus, tabs, trees, dialogs, status, toggles, and progress;
- host-scheduled application updates using winit control flow without rendering outside
  `RedrawRequested`;
- a dedicated native proof gallery for visual, interaction, DPI, theme, shaping, and accessibility
  regression work;
- a dedicated native editor demo with editable documents, dirty tabs, project navigation,
  command palette, find panel, theme/sidebar commands, and shared editor text geometry.

The two applications intentionally serve different purposes. Animation and stress proofs stay out of
the event-driven editor path, while both applications consume the same reusable Luna widgets.

## Build and validate

Install the pinned Rust toolchain through rustup, then run the complete quality gate:

```bash
cargo fmt --all
./scripts/validate.sh
```

Run the M3 proof gallery:

```bash
cargo run -p luna-ui-rust-proof-gallery
```

The gallery supports window resizing, button/toggle activation, light/dark theme switching through
the Theme card, multilingual shaping, continuous deterministic animation, and AccessKit actions.
Press **Escape** or close the window to exit.

Run the M3 editor integration harness:

```bash
cargo run -p luna-ui-rust-editor-demo
```

Editor shortcuts:

- **Control-P** — command palette;
- **Control-F** — find/replace panel;
- **Control-S** — mark the active document saved;
- **Control-N** — create a new document;
- **Control-B** — toggle the sidebar;
- **Control-W** — close the active tab when more than one is open;
- **Control-A** — select all editor text;
- **Escape** — close an overlay, or exit when no overlay is open.

The M2 text demo and earlier proofs remain available:

```bash
cargo run -p luna-ui-rust-text-demo
cargo run -p luna-ui-rust-native-demo
cargo run -p luna-ui-rust-demo -- ./luna-ui-rust-m0.ppm
```

## Governing architecture

```text
native event / AccessKit action / scheduled logical update
    -> platform-neutral Luna application state
    -> reusable shell, control, overlay, gallery, or text widgets
    -> immutable display list + validated semantic tree
    -> CPU renderer / AccessKit adapter
    -> softbuffer presentation
```

Widgets do not call graphics APIs, create native windows, own operating-system event loops, or embed
Moth-specific product policy. See `docs/ARCHITECTURE.md`, `docs/PORTING_MAP.md`, and
`docs/M3_UPGRADE.md`.

## License

Mozilla Public License 2.0. Source files include SPDX identifiers and modifications to MPL-covered
files remain available under the MPL terms.
