# M3.3b Advanced Tabs and Desktop Popup Surfaces

## Purpose

M3.3a established recursive editor panes and independent `DocumentViewId` state. M3.3b completes
that visible desktop-editor layer with pane-local tab policy, overflow handling, drag movement,
submenus, context menus, completion suggestions, richer find/replace controls, and an interactive
vertical scrollbar.

The phase keeps product policy outside reusable Luna widgets. `luna-panes` owns tab ordering and
pane-local metadata. `luna-ui` owns geometry, painting, hit regions, and accessibility. The editor
demo supplies commands, completion candidates, find policy, and document-lifecycle decisions.

## Advanced pane-local tabs

Each `PaneLeaf` now records:

- ordered `DocumentViewId` tabs;
- a leading ordered pinned partition;
- at most one replaceable preview tab;
- the active tab;
- a regular-tab overflow offset.

Pinned tabs remain visible before the scrollable regular-tab viewport. Pinning moves a tab into the
leading partition and clears preview state. Unpinning moves it to the first regular position.

A clean workspace activation may create a preview tab. Opening another clean preview in the same
pane replaces the previous preview view. Editing, pinning, or explicitly choosing **Keep Preview
Open** promotes the preview into a persistent regular tab. Dirty preview content is never silently
replaced.

`PaneTree::reorder_view` and `PaneTree::move_view` preserve the pinned/regular partition. Moving the
only tab out of a pane collapses that pane through the same safe recursive reconciliation used by
pane close.

## Overflow and active-tab visibility

`EditorPaneSurface` divides each tab strip into:

```text
[pinned tabs][regular tab viewport][previous][next]
```

Regular tabs use a fixed display width and pinned tabs use a compact width. The pane model retains a
requested regular-tab offset, while the immutable surface layout computes an effective offset that
always keeps the active regular tab visible. Previous and next controls are exposed through the same
rectangles for paint, pointer hit testing, labels, and accessibility.

The overflow controls activate a newly revealed boundary tab so keyboard focus and visible tab state
remain synchronized.

## Drag reorder and cross-pane movement

A primary-button drag beginning on a pane tab records the source pane, source view, and pointer
origin. On release over any pane-local tab strip, the shared tab geometry resolves an insertion
index. The pane model either reorders the view locally or moves it to the target pane.

The operation preserves:

- `DocumentViewId` identity;
- shared `DocumentId` lifecycle state;
- dirty state;
- pinned or preview metadata;
- pane-local caret, selection, scroll, and retained layout cache.

No document bytes are copied or reopened during a tab move.

## Menus, submenus, and mnemonics

`MenuItem` now supports commands, separators, and nested `MenuDefinition` values. Dropdown layout
can project a parent panel and one child submenu panel with viewport-aware left/right placement.

Keyboard and pointer behavior includes:

- Right or pointer hover opens a selected submenu;
- Left closes the child submenu before changing top-level menus;
- Up, Down, Home, and End operate at the current menu level;
- Enter activates a command or opens a submenu;
- case-insensitive mnemonics select commands or open submenus;
- Alt plus a top-level mnemonic opens File, Edit, Find, View, or Help.

The File menu uses **Open Recent** as a submenu. View uses a **Tabs** submenu with pin, preview,
move-next/move-previous, and close commands. The tab context menu uses an exact **Move to Pane**
submenu generated from current pane identities.

## Tab context menu

Secondary-clicking a pane tab opens a reusable `DropdownMenu` anchored to that tab. The application
projects command state for the clicked tab and pane:

- Pin Tab;
- Unpin Tab;
- Keep Preview Open;
- Move to Pane;
- Close Tab.

The context menu has independent state from the top-level menu, command palette, find panel, and
completion popup. Only one transient surface may be visible at a time.

## Completion popup foundation

`CompletionPopup` is a product-neutral caret-anchored list surface with:

- stable candidate and semantic identities;
- label and detail text;
- below-caret placement with above-caret fallback;
- keyboard selection with wrapping;
- pointer and accessibility activation;
- an insertion payload returned to application policy.

The editor demo opens it with `Control-Space`, filters a deterministic candidate catalog by the
active identifier prefix, and replaces that prefix on Enter or Tab. The candidate catalog is only a
runtime proof; language servers and Moth-owned completion providers remain future adapters.

## Find, replace, and scrollbar behavior

The find panel now includes:

- match-case toggle;
- whole-identifier-word toggle;
- Replace Current;
- Replace All;
- accessible checkbox and action semantics.

The editor demo implements literal ASCII-insensitive or case-sensitive matching with identifier-word
boundaries. Replacement ranges are applied in reverse order for Replace All so earlier byte offsets
remain stable.

`TextView` reserves a vertical scrollbar track, derives thumb size and position from the exact text
viewport and maximum scroll range, maps pointer positions back to clamped scroll offsets, and shares
that geometry across paint and input.

## Accessibility

M3.3b adds semantics for:

- pinned and preview tab state;
- tab overflow previous/next buttons;
- nested menu rows and child panels;
- tab context-menu commands;
- completion list and selected item;
- find-option checkboxes and replace buttons.

Accessible actions route through the same application commands and popup item identities as pointer
and keyboard interaction.

## Test coverage

M3.3b adds deterministic tests for:

- pinned-first tab ordering;
- preview replacement and promotion;
- local reorder and cross-pane move;
- source-pane collapse after moving its only view;
- tab-scroll offset clamping;
- pinned tabs remaining outside regular overflow;
- active-tab visibility correction;
- tab drop-target indices;
- nested menu geometry, command resolution, and mnemonics;
- completion placement, semantics, and prefix replacement;
- case-sensitive and whole-word matching;
- Replace All excluding identifier substrings;
- scrollbar thumb geometry and pointer mapping;
- application command/menu/context integration.

## Deferred

- arbitrary-depth cascading submenus;
- delayed submenu-open timing and pointer-intent triangles;
- keyboard tab reordering and direct numbered-tab shortcuts;
- pinned-tab persistence and pane topology persistence;
- tab drag ghost imagery and cross-window transfer;
- language-server completion adapters and asynchronous result replacement;
- regex search, preserve-case replacement, and search history;
- horizontal scrollbar, minimap, and scrollbar markers;
- production native filesystem watcher backends.
