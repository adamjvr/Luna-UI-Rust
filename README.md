# Luna-UI-Rust

**Luna-UI-Rust** is a clean Rust-native rewrite of Luna UI: the product-neutral UI, input,
layout, rendering, accessibility, command, document, text, and native-host foundation used to build
editor-class desktop applications such as Moth Text.

This is not a mechanical Swift-to-Rust translation. The rewrite preserves Luna's architectural
contracts while expressing them through Rust ownership, explicit errors, immutable frame snapshots,
small workspace crates, strict compiler tooling, and platform-specific leaf adapters.

## M4 status

M0 established the deterministic core, M1 added the native host, M2 added editor-grade text, M3
added the twin proof-gallery/editor applications, M3.1 made the CPU path incremental, M3.2 completed
real file/workspace/session behavior, and M3.3 completed durable recursive panes and hardened desktop
interaction.

M4 adds an optional GPU path while retaining the CPU renderer as the default oracle and fallback:

- `luna-render-wgpu` compiles immutable display lists into ordered solid/image quad batches;
- nested logical clip stacks map to CPU clips and GPU scissors through one DPI rule;
- repeated raster images share a bounded per-frame BGRA atlas;
- `luna-host-wgpu` preserves the existing application, input, invalidation, and AccessKit contracts;
- surface resize/loss and device-loss paths rebuild native GPU resources without resetting app state;
- the proof gallery runs through either CPU/softbuffer or GPU/wgpu presentation;
- Luna Dark, Luna Light, Amber Monitor, and Green Terminal are shared built-in theme presets;
- the editor exposes the presets through **View > Color Scheme**.

See `docs/M4_GPU_RENDERING.md`, `docs/ROADMAP.md`, and `docs/SWIFT_PARITY.md`.

## Build and validate

Install the pinned Rust toolchain through rustup, then run the complete quality gate:

```bash
cargo fmt --all
./scripts/validate.sh
```

M4 includes the focused automated gate and CPU/GPU runtime checklist:

```bash
./scripts/test-m4.sh
```

Run the proof gallery with the deterministic CPU backend:

```bash
cargo run --release -p luna-ui-rust-proof-gallery
```

Run the identical application through the GPU backend:

```bash
LUNA_RENDER_BACKEND=wgpu cargo run --release -p luna-ui-rust-proof-gallery
```

The M3.1c gallery baseline must remain intact: ordinary animation hits retained layout, static-paint,
semantic, and label caches; restores only the animation lane; skips unchanged accessibility
translation; and produces no frame while pointer motion remains inside one semantic hover target.

Run the event-driven editor integration harness through the CPU backend:

```bash
cargo run --release -p luna-ui-rust-editor-demo
```

Run the same editor through the GPU backend:

```bash
LUNA_RENDER_BACKEND=wgpu cargo run --release -p luna-ui-rust-editor-demo
```

Editor menu behavior:

- click **File**, **Edit**, **Find**, **View**, or **Help** to open its anchored dropdown;
- click another heading, or move across headings while a dropdown is open, to switch menus;
- use Up/Down/Home/End at the current menu level; Right opens a submenu and Left closes it;
- use Enter or Space to activate and Escape or an outside click to dismiss;
- disabled commands remain visible but cannot activate;
- checked commands expose current sidebar and selected color scheme; Alt mnemonics open top-level menus;
- **Control-P** opens the separate searchable command palette and never opens from an ordinary
  menu-heading click.

Editor shortcuts:

- **Control-P** — command palette;
- **Control-F** — find panel;
- **Control-H** — find/replace panel;
- **Control-O** — open a UTF-8 text file through a native file chooser;
- **Control-S** — save to the current file or open Save As for untitled/virtual content;
- **Control-Shift-S** — Save As to a new destination;
- **Control-N** — create a new document;
- **Control-B** — toggle the sidebar;
- **Control-W** — close the active pane-local tab; the final document view retains dirty-close policy;
- **Control-\** — split the focused pane to the right;
- **Control-Shift-\** — split the focused pane downward;
- **Control-Alt-Left/Right** — focus the previous/next pane;
- **Control-Shift-W** — close the focused pane;
- **Control-Space** — open completion suggestions at the caret;
- **Control-A** — select all editor text;
- **Escape** — close the active menu/overlay, or exit when none is open.

Current document, workspace, pane, popup, and rendering behavior:

- New File creates an empty, clean, monotonically named Untitled document;
- Open loads strict UTF-8 text and activates an existing tab when the canonical file is already open;
- Save writes through an optimistic storage-snapshot check;
- Save As assigns the selected canonical file identity and updates the tab title;
- dirty close offers Save, Discard, and Cancel;
- a storage race offers Overwrite, Reload, and Cancel;
- canceled dialogs and failed writes preserve editor content and dirty state;
- clean close releases both editor-view state and registry identity;
- successful Open/Save/Save As updates an eight-entry recent-file list persisted in session state;
- external in-place edits, replacements, deletion, and recreation are distinguished;
- unchanged observations produce no redraw;
- Reload from Disk explicitly adopts observed content and clears external state.
- Open Folder restores a real workspace tree with stable expansion and selection;
- workspace create, folder create, rename, and delete use explicit product-neutral mutation policy;
- dirty files affected by deletion may remain open as Untitled, close after discard, or cancel;
- recent files and workspace tree state persist across launches in the XDG state directory;
- edits in one pane synchronize the shared document text into sibling views;
- caret, selection, scroll, focus, and text-layout caches remain pane-local;
- closing a pane-local shared view does not close the shared document;
- closing a pane rehomes documents that would otherwise lose their final live view;
- clean workspace activations use a replaceable preview tab and editing promotes it;
- pinned tabs remain leading and visible while regular tabs overflow;
- tab dragging reorders locally or moves the existing view to another pane;
- secondary-clicking a tab opens Pin/Preview/Move/Close commands;
- completion acceptance replaces the active identifier prefix;
- find/replace supports case and whole-word filters plus current/all replacement;
- the vertical scrollbar shares exact paint and pointer geometry.

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
    -> UTF-8 file/dialog service + storage observation boundary
    -> MRU recent-file projection + external-state transition
    -> workspace scan/mutation policy + persistent session state
    -> recursive pane topology + shared document/view projection
    -> platform-neutral Luna application state
    -> retained text/layout/chrome or retained static scene
    -> dynamic display list + shared validated semantic snapshot
    -> CPU retained framebuffers or ordered GPU quad/scissor/atlas compilation
    -> conditional AccessKit translation
    -> softbuffer or wgpu presentation
```

Widgets do not call graphics APIs, create native windows, own operating-system event loops, or embed
Moth-specific product policy. See `docs/ARCHITECTURE.md`, `docs/PORTING_MAP.md`,
`docs/M3_1A_HOST_PIPELINE.md`, `docs/M3_1B_TEXT_CACHE.md`,
`docs/M3_1C_GALLERY_ACCESSIBILITY.md`, `docs/M3_1D_DROPDOWN_MENUS.md`,
`docs/M3_2A_DOCUMENT_LIFECYCLE.md`, `docs/M3_2B_FILE_DIALOG_SERVICES.md`,
`docs/M3_2C_RECENT_EXTERNAL_CHANGES.md`, `docs/M3_2D_WORKSPACE_TREE.md`,
`docs/M3_2E_WORKSPACE_OPERATIONS_SESSION.md`, `docs/M3_3A_SPLIT_PANES.md`,
`docs/M3_3B_ADVANCED_TABS_POPUPS.md`, `docs/M3_3C_DESKTOP_HARDENING.md`,
`docs/M4_GPU_RENDERING.md`, and `docs/SWIFT_PARITY.md`.

## License

Mozilla Public License 2.0. Source files include SPDX identifiers and modifications to MPL-covered
files remain available under the MPL terms.
