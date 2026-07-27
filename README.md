# Luna-UI-Rust

**Luna-UI-Rust** is a clean Rust-native rewrite of Luna UI: the product-neutral UI, input,
layout, rendering, accessibility, command, document, text, and native-host foundation for
product-neutral desktop applications, including editor-class software.

This is not a mechanical Swift-to-Rust translation. The rewrite preserves Luna's architectural
contracts while expressing them through Rust ownership, explicit errors, immutable frame snapshots,
small workspace crates, strict compiler tooling, and platform-specific leaf adapters.

## M8 status

M0 established the deterministic core, M1 added the native host, M2 added editor-grade text, M3
added the twin proof-gallery/editor applications, M3.1 made the CPU path incremental, M3.2 completed
real file/workspace/session behavior, M3.3 completed durable recursive panes and hardened desktop
interaction, M4 added the optional `wgpu` backend, M5 added broader editor mechanics, and M6
completed the first macOS/downstream integration hardening pass. M6 has been locally accepted on
Pop!_OS and committed.

M7 establishes enforceable public API and release-qualification evidence:

- `api/public-api.toml` and `luna-qualification` classify every public library crate as stable,
  provisional, or internal;
- public boundary errors expose stable `ErrorCode` values without freezing human-readable text;
- `luna-ui-rust-qualification` exercises real editor replay, retained text, pane, workspace, CPU/GPU,
  and accessibility structures against deterministic budgets;
- the `wgpu` backend now retains scene buffers geometrically within explicit caps, skips unchanged
  atlas uploads, exposes resource statistics, and trims rebuildable resources on memory pressure;
- `ResourceLocator` and its downstream example define product-neutral development and packaged
  resource lookup;
- `scripts/package-linux.sh` creates a relocatable Linux development bundle and tarball;
- [`docs/EDITOR_DEMO_COMMANDS.md`](docs/EDITOR_DEMO_COMMANDS.md) permanently records the complete
  keyboard, mouse, runtime, and acceptance command set;
- Linux remains the blocking primary target, macOS remains the supported secondary/advisory target,
  and Windows remains unofficial and best-effort.

M7.0.1 is accepted on the blocking Linux/Pop!_OS lane. M8 retains that baseline, captures
reproducible release evidence, validates a separate downstream consumer, and prepares the planned
`0.2.0-rc.1` development candidate.

See `docs/M8_RELEASE_CANDIDATE.md`, `docs/M7_RELEASE_QUALIFICATION.md`,
`docs/PUBLIC_API_POLICY.md`, `docs/EDITOR_DEMO_COMMANDS.md`, `docs/LINUX_PACKAGING.md`,
`docs/ACCESSIBILITY_AUDIT.md`, `docs/RELEASE_CHECKLIST.md`, `docs/ROADMAP.md`, and
`docs/SWIFT_PARITY.md`.

## Build and validate

Install the pinned Rust toolchain through rustup, then run the complete quality gate:

```bash
cargo fmt --all
./scripts/validate.sh
```

M8 re-runs the complete M7 gate, verifies the retained baseline, and captures release evidence:

```bash
./scripts/test-m8.sh
```

The accepted M7 gate remains available directly:

```bash
./scripts/test-m7.sh
```

Build deterministic qualification evidence and the Linux development bundle:

```bash
cargo run --release -p luna-ui-rust-qualification -- --output /tmp/luna-m7-qualification.json
./scripts/package-linux.sh
```

On macOS, run the secondary-platform gate and optionally package the editor proof:

```bash
./scripts/test-macos.sh
./scripts/package-macos.sh
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

The complete editor operator reference is maintained in
[`docs/EDITOR_DEMO_COMMANDS.md`](docs/EDITOR_DEMO_COMMANDS.md).

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
- **Control-Z** — undo the active shared document transaction;
- **Control-Shift-Z** — redo the active shared document transaction;
- **Control-Shift-Up/Down** — add a cursor on the adjacent logical line;
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
- recent files and workspace tree state persist in XDG state storage on Linux and Application Support on macOS;
- edits in one pane synchronize the shared document text into sibling views;
- caret, selection, scroll, focus, and text-layout caches remain pane-local;
- closing a pane-local shared view does not close the shared document;
- closing a pane rehomes documents that would otherwise lose their final live view;
- clean workspace activations use a replaceable preview tab and editing promotes it;
- pinned tabs remain leading and visible while regular tabs overflow;
- tab dragging reorders locally or moves the existing view to another pane;
- secondary-clicking a tab opens Pin/Preview/Move/Close commands;
- syntax scopes are supplied through a product-neutral provider and styled through an imported
  Sublime color scheme;
- completion acceptance replaces the active identifier prefix as one undoable transaction;
- find/replace supports case and whole-word filters plus undoable current/all replacement;
- multiple cursors insert, replace, and delete simultaneously while retaining one primary cursor;
- IME pre-edit remains transient, anchors native candidate UI to the caret, and commits once;
- editable accessibility replacement/value requests enter the same transaction history;
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
    -> syntax/theme + transaction history + multiple-selection + IME mechanics
    -> platform-neutral lifecycle + downstream adapter composition
    -> platform-neutral Luna application state
    -> retained styled text/layout/chrome or retained static scene
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
`docs/M4_GPU_RENDERING.md`, `docs/M5_EDITOR_COMPONENT_PARITY.md`,
`docs/M6_MACOS_INTEGRATION.md`, `docs/M7_RELEASE_QUALIFICATION.md`,
`docs/EDITOR_DEMO_COMMANDS.md`, `docs/PUBLIC_API_POLICY.md`, `docs/LINUX_PACKAGING.md`,
`docs/MACOS_TESTING.md`, and `docs/SWIFT_PARITY.md`.

## License

Mozilla Public License 2.0. Source files include SPDX identifiers and modifications to MPL-covered
files remain available under the MPL terms.
