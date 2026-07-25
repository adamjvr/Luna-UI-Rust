# Luna-UI-Rust

**Luna-UI-Rust** is a clean Rust-native rewrite of Luna UI: the product-neutral UI, input,
layout, rendering, accessibility, command, document, text, and native-host foundation used to build
editor-class desktop applications such as Moth Text.

This is not a mechanical Swift-to-Rust translation. The rewrite preserves Luna's architectural
contracts while expressing them through Rust ownership, explicit errors, immutable frame snapshots,
small workspace crates, strict compiler tooling, and platform-specific leaf adapters.

## M3.2 status

M0 established the deterministic core, M1 added the native host, M2 added editor-grade text, M3
added the twin proof-gallery/editor applications, and M3.1 made the CPU path incremental while
adding real first-level desktop dropdown menus.

M3.1a through M3.1d are complete and locally validated. File/Edit/Find/View/Help open anchored
dropdowns, while Control-P opens the independent searchable command palette.

M3.2a begins the real document/workspace phase with a new product-neutral `luna-documents` crate:

- monotonic stable `DocumentId` values for tabs, caches, hit routing, and accessibility;
- monotonic untitled naming that is not reused after close;
- adapter-canonicalized `FileIdentity` values and duplicate-open prevention;
- saved edit revisions and explicit dirty-state evaluation;
- opaque storage revisions plus in-sync, externally modified, and missing-file state;
- explicit Save decisions: no-op, Save As, direct file write, or unsupported source;
- explicit close decisions: safe close or Save/Discard/Cancel required;
- Save As reassignment with duplicate-file protection;
- editor-demo integration that blocks dirty close and never simulates a successful file write.

M3.2b is next: real UTF-8 open/save/save-as and dirty-close services behind mockable filesystem and
dialog boundaries. Workspace trees, watchers, recent files, and independent shared-buffer views
follow in later M3.2 subphases. See `docs/M3_2A_DOCUMENT_LIFECYCLE.md` and
`docs/SWIFT_PARITY.md`.

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
- **Control-S** — evaluate Save; untitled documents report that Save As is required;
- **Control-N** — create a new document;
- **Control-B** — toggle the sidebar;
- **Control-W** — close a clean tab; dirty tabs require a future Save/Discard/Cancel dialog;
- **Control-A** — select all editor text;
- **Escape** — close the active menu/overlay, or exit when none is open.

M3.2a document behavior:

- New File creates an empty, clean, monotonically named Untitled document;
- Save on an untitled document reports that Save As is required rather than faking persistence;
- dirty tab close is blocked until a future Save/Discard/Cancel service resolves it;
- clean close releases both editor-view state and registry identity;
- the sidebar projects the same stable document IDs used by tabs and caches.

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
    -> product-neutral document registry and lifecycle decision
    -> platform-neutral Luna application state
    -> retained text/layout/chrome or retained static scene
    -> dynamic display list + shared validated semantic snapshot
    -> retained working/static CPU framebuffers + conditional AccessKit translation
    -> size-gated softbuffer presentation
```

Widgets do not call graphics APIs, create native windows, own operating-system event loops, or embed
Moth-specific product policy. See `docs/ARCHITECTURE.md`, `docs/PORTING_MAP.md`,
`docs/M3_1A_HOST_PIPELINE.md`, `docs/M3_1B_TEXT_CACHE.md`,
`docs/M3_1C_GALLERY_ACCESSIBILITY.md`, `docs/M3_1D_DROPDOWN_MENUS.md`,
`docs/M3_2A_DOCUMENT_LIFECYCLE.md`, and `docs/SWIFT_PARITY.md`.

## License

Mozilla Public License 2.0. Source files include SPDX identifiers and modifications to MPL-covered
files remain available under the MPL terms.
