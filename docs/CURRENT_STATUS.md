# Current Status

**Milestone:** M3.1d.1 menu-routing correction

## Verified baseline

- M0 through M3.1b were compiler-validated, run, committed, and pushed by the project owner.
- M3.1c gallery, accessibility, and input optimization passed the complete local quality/runtime
  gate and was committed by the project owner.
- M3.1c remains the retained-gallery, input-coalescing, and conditional-accessibility baseline.

## M3.1d.1 runtime correction

Native testing showed that the first M3.1d integration did not enforce menu-heading priority strongly
enough: transient overlay handling could consume the same pointer path, leaving the command-palette
interface visible instead of constructing the anchored dropdown. M3.1d.1 corrects the integration:

- primary clicks on File/Edit/Find/View/Help are resolved before palette, find-panel, or editor input;
- opening a menu clears palette and find state and asserts that exactly one transient surface remains;
- Control-P is the only command-palette opening path in the demo;
- `Command Palette…` is no longer projected into a dropdown menu;
- the dropdown uses a narrower anchored panel and drop shadow, with no modal backdrop or query field;
- pointer-path tests begin with an open palette, click File, and prove that the palette is replaced by
  an anchored dropdown below the File heading.

## Implemented in M3.1d

- `luna-ui` now exposes reusable product-neutral menu definitions, command rows, separators,
  dropdown state, row/layout snapshots, and an anchored dropdown widget.
- The editor demo owns one command catalog and projects it into both top-level dropdown menus and the
  searchable command palette.
- Menu headings no longer route to `open_palette`; File, Edit, Find, View, and Help each open their
  own dropdown.
- Dropdown state, command-palette state, and find-panel state are independent. Opening one command
  surface never silently substitutes another.
- Dropdown geometry is anchored below its heading, clamped to the current viewport, and shared by
  paint, pointer hit testing, and accessibility.
- Commands support separators, shortcut descriptions, disabled states, and checked states.
- Keyboard interaction supports Up, Down, Home, End, Left, Right, Enter, Space, and Escape.
- Pointer interaction supports heading clicks, heading-to-heading traversal while open, row hover,
  command activation, disabled rows, and outside-click dismissal.
- Accessibility exposes the active top-level heading as expanded, the dropdown as a menu, and every
  command as a menu item with focused, disabled, checked, and shortcut state.
- Menu open/close and row navigation use overlay invalidation and do not invalidate document text
  layout or raster state.
- Ctrl+P remains the normal searchable command palette. It consumes the same enabled command IDs as
  the dropdowns but has its own query, selection, backdrop, and dismissal behavior.
- Command keyboard shortcuts, menu rows, palette rows, pointer activation, and accessibility actions
  converge on `execute_command` rather than duplicating command behavior.

## Swift parity checkpoint

The detailed comparison is in [`SWIFT_PARITY.md`](SWIFT_PARITY.md). M3.1d narrows the visible menu
and command-surface gap: Rust now has functional first-level dropdowns with checked/disabled state,
normal desktop navigation, and a distinct command palette. Swift remains ahead in submenus, context
menus, completion, richer command availability, real files/workspaces, split panes, advanced tabs,
and paired Moth integration.

The directional estimate after M3.1d is approximately 70–80% foundational-architecture parity,
65–75% core text/editing parity, 45–55% reusable editor-UI parity, 15–25%
file/workspace/pane integration parity, and approximately 50–55% of Swift Luna UI's complete
functional surface.

## Active validation gate

Run from the repository root:

```bash
cargo fmt --all
./scripts/validate.sh
cargo run --release -p luna-ui-rust-editor-demo
cargo run --release -p luna-ui-rust-proof-gallery
```

Editor runtime confirmation must show:

- each top-level heading opens its own dropdown rather than the command palette;
- clicking the same heading closes the dropdown;
- clicking or moving to another heading while open switches menus;
- disabled rows remain visible and cannot activate;
- checked sidebar/theme commands report current state;
- Up/Down/Home/End navigate enabled commands;
- Left/Right switch top-level menus;
- Enter and Space activate the selected command;
- Escape and outside clicks dismiss the dropdown;
- Ctrl+P opens the separate searchable command palette;
- menu and palette command activation produce the same application result;
- opening or navigating menus causes no text-layout or raster miss;
- editor idle and M3.1b text-cache behavior remain unchanged;
- proof-gallery retained-scene, coalescing, and accessibility behavior remain unchanged.

## Next milestone

M3.2 adds real document and workspace runtime boundaries: UTF-8 open/save lifecycle, untitled files,
Save As, external-change and save-conflict state, shared buffers with independent views,
project/workspace adapters, and watcher integration.

M3.3 remains responsible for recursive panes, advanced tabs, context menus, completion popups,
submenus, and broader editor-shell parity.
