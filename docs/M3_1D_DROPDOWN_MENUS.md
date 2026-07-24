# M3.1d Dropdown Menus and Command-Surface Separation

M3.1d replaces the editor demo's temporary `menu heading -> command palette` shortcut with real
first-level desktop dropdown menus. M3.1d.1 corrects the native pointer integration after runtime
testing showed that the first delivery could still leave the command-palette surface in control of
menu-heading clicks. The command palette remains a separate searchable surface opened only by
Ctrl+P.

## M3.1d.1 correction contract

```text
primary pointer press
    -> resolve top-level menu heading first
    -> clear palette and find state
    -> open one anchored DropdownMenu
    -> assert exactly one transient surface
```

The command palette is not itself a dropdown row. This makes an ordinary File/Edit/Find/View/Help
click incapable of executing the `command-palette` command.

## Governing command contract

The application owns command meaning, availability, checked state, and execution. Luna owns reusable
presentation, geometry, interaction, and accessibility:

```text
application MenuDefinition values
    -> EditorShell top-level headings
    -> DropdownMenu rows
    -> enabled PaletteItem projection
    -> one execute_command path
```

A command may appear in a menu, the palette, a keyboard shortcut, and an accessibility action without
duplicating its behavior.

## Reusable menu types

`luna-ui` exposes:

- `MenuCommand` — stable command ID, title, shortcut, enabled state, and checked state;
- `MenuItem` — command or separator;
- `MenuDefinition` — stable top-level menu ID, title, and ordered items;
- `DropdownMenuState` — active menu and selected source-item index;
- `DropdownMenuLayout` and `DropdownMenuRowFrame` — immutable shared geometry;
- `DropdownMenu` — painting, hit testing, command resolution, and accessibility.

The menu widget does not execute commands and does not know Moth product policy.

## State separation

```text
DropdownMenuState
    active heading + selected row

CommandPaletteState
    query + filtered result selection

FindPanelState
    query/replacement focus + match selection
```

Opening a top-level menu closes both the command palette and find panel before constructing the
dropdown. Ctrl+P closes menus and the find panel before creating a fresh searchable palette
projection. The demo maintains a hard one-transient-surface invariant.

## Pointer behavior

- clicking a closed heading opens its dropdown;
- clicking the active heading closes it;
- clicking another heading switches immediately;
- moving across headings while a menu is open switches menus;
- moving over an enabled row changes keyboard selection;
- clicking an enabled row executes its command;
- disabled rows and separators do not execute;
- clicking inside non-command menu space keeps the menu open;
- clicking outside dismisses the menu without activating the underlying editor surface.

## Keyboard behavior

While a menu is open:

- Down/Up select the next/previous enabled command and wrap;
- Home/End select the first/last enabled command;
- Left/Right switch top-level menus and select the new menu's first enabled command;
- Enter or Space executes the selected command;
- Escape dismisses the dropdown;
- ordinary text and editor shortcuts do not leak through to the document.

When no menu or overlay is open, Escape retains the editor demo's existing exit behavior.

## Geometry and rendering

The dropdown is anchored below the active heading and clamped inside the current viewport. It uses a
compact fixed preferred width, bounded row heights, separators, a checked indicator, and theme-derived
panel, border, selection, accent, foreground, and muted-foreground colors.

Application-shaped labels are placed into the exact row rectangles from `DropdownMenuLayout` and use
stable `TextLabelCache` slots. Opening, closing, or navigating menus therefore does not reshape the
editor document.

## Accessibility

The editor shell exposes each top-level heading with `Expanded` or `Collapsed` value and focused state.
The open dropdown exposes:

- one `Menu` root;
- ordered `MenuItem` children;
- stable IDs derived from command IDs;
- shared row bounds;
- disabled state;
- focused selection;
- checked and shortcut value text.

Accessibility click actions resolve through the same command ID used by pointer, keyboard, and
palette activation.

## Invalidation

```text
open/close/switch menu -> TextOverlay
change selected row    -> PaintOverlay
execute command        -> command-specific class
accessibility action   -> Accessibility or command-specific class
```

Menu interaction must not produce `TextLayout` or `TextRaster` invalidation unless the executed
command actually edits or reconfigures document text.

## Runtime acceptance

Run:

```bash
cargo run --release -p luna-ui-rust-editor-demo
```

Confirm:

1. File/Edit/Find/View/Help open their own anchored dropdowns with no modal backdrop or query field.
2. Open the palette with Ctrl+P, then click File once: the palette must disappear and the File
   dropdown must appear on that same click.
3. Ctrl+P opens only the searchable command palette.
4. Menus and palette execute the same command IDs.
5. disabled Open, Save As, Undo, Redo, Cut, Copy, Paste, and About rows remain visible but inert;
6. Save and Close Tab availability tracks current application state;
7. Show Sidebar and Light Theme checked state tracks the application;
8. pointer and keyboard navigation behave as described above;
9. accessibility activation opens headings and executes enabled rows;
10. menu interaction creates no document layout/raster cache miss;
11. proof-gallery M3.1c behavior remains unchanged.
