# M3.3a Recursive Split-Pane Runtime

## Purpose

M3.2e established independent `DocumentViewId` records but still presented one live editor view at a
time. M3.3a consumes that boundary with a real recursive pane tree while preserving one lifecycle,
file identity, dirty baseline, storage observation, and save policy per `DocumentId`.

The phase is deliberately product-neutral. Luna owns pane topology, geometry, tab chrome, hit
regions, and accessibility projection. The editor application owns command policy and synchronizes
view snapshots with its canonical document buffer.

## Pane topology

The new `luna-panes` crate models a binary recursive tree:

```text
PaneNode
  ├── Leaf(PaneLeaf)
  │     ├── stable PaneId
  │     ├── ordered DocumentViewId tabs
  │     └── active DocumentViewId
  └── Split(PaneSplit)
        ├── stable PaneId
        ├── Horizontal or Vertical axis
        ├── clamped split ratio
        ├── first PaneNode
        └── second PaneNode
```

A horizontal split produces side-by-side panes and a vertical splitter. A vertical split produces
stacked panes and a horizontal splitter. Leaves and splitters are traversed deterministically in
depth-first order.

Closing an empty leaf collapses its parent into the surviving sibling. The final pane cannot be
closed, so the tree never enters an invalid empty state.

## Shared document and independent view state

A `DocumentId` remains the shared logical buffer and lifecycle identity. Each pane tab owns a unique
`DocumentViewId` and application-side view state:

- caret;
- selection;
- horizontal and vertical scroll;
- focus;
- retained text-layout cache key;
- semantic text node identity.

After an edit, the active view commits its text and revision to the canonical `DemoDocument`. Other
views of the same `DocumentId` receive that shared text revision through
`EditableText::synchronize_document`, which preserves and clamps their independent caret and
selection state. Scroll remains pane-local.

This is synchronized shared logical-buffer state, not concurrent mutable aliasing. All mutation
still occurs on the native UI lane.

## Pane-local tab ownership

The legacy window-wide tab strip can now be disabled in `EditorShellState`. The new reusable
`EditorPaneSurface` projects one tab strip per leaf pane and shares geometry across:

- paint;
- pointer hit testing;
- close-button targeting;
- text-label placement;
- accessibility nodes.

Opening or creating a document places a new view in the focused pane. Activating a document prefers
an existing view in the focused pane, then an existing view in another pane, and finally creates a
new view in the focused pane.

`Control-W` closes the active pane-local view. When another view of the same document exists, the
shared document remains open. Closing the final view of a document invokes the existing dirty-close
policy. `Control-Shift-W` closes the focused pane and rehomes documents that would otherwise lose
their final live view.

## Split layout and resizing

`PaneTree::layout` produces one immutable `PaneLayoutSnapshot` containing leaf and splitter
rectangles. Split ratios are stored in thousandths and clamped from ten through ninety percent.
Layout additionally enforces a minimum pane extent when enough space exists.

Pointer dragging updates a split ratio through the splitter's complete container bounds. The same
snapshot drives rendering, hit testing, and accessibility, preventing divergent splitter geometry.

## Focus and command routing

The editor routes text, find, save, close, and document commands through the focused pane's active
view. Focus traversal follows deterministic depth-first leaf order and wraps:

- `Control-Alt-Right` or `Control-Alt-Down` — next pane;
- `Control-Alt-Left` or `Control-Alt-Up` — previous pane.

Split commands are:

- `Control-\` — split the focused pane to the right;
- `Control-Shift-\` — split the focused pane downward;
- `Control-Shift-W` — close the focused pane.

Menus and the command palette expose the same command IDs.

## Accessibility

`EditorPaneSurface` exposes:

- one root editor-panes group;
- one group per leaf pane with focused/inactive value;
- one tab list per pane;
- tab nodes with Saved or Modified value;
- first-class close-button nodes;
- one editor-content group per pane parenting the active text semantic tree;
- splitter groups labeled by orientation and marked draggable.

Accessibility focus on a pane tab activates that view. Clicking a semantic close button follows the
same pane-aware close path as a pointer click. Focusing editor content changes pane focus without
inventing a second document identity.

## Cache and invalidation behavior

Text layout caches are keyed by `DocumentViewId`, allowing each pane to retain its own viewport band
and scroll state. Shared text revisions invalidate sibling logical layout snapshots only after text
changes; caret, selection, pane focus, tab selection, and splitter highlighting remain overlay or
widget-layout invalidations as appropriate.

The proof gallery and earlier editor file/workspace behavior remain regression requirements.

## Test coverage

M3.3a adds tests for:

- horizontal and vertical recursive splits;
- stable depth-first focus traversal and wrapping;
- pane collapse after closing the final local tab;
- protection of the last remaining pane;
- split-ratio pointer updates and clamping;
- minimum pane geometry and tab-strip allocation;
- shared text revisions with independent caret and scroll;
- pane-local close preserving a shared document;
- closing a pane while rehoming uniquely represented documents;
- pane tab, close button, editor, and splitter hit geometry;
- pane accessibility action routing;
- shared-text synchronization clamping stale caret and selection positions.

## Follow-up status

M3.3b now implements tab drag reordering, pinned and preview tabs, overflow scrolling, cross-pane tab
movement, one-level child submenus, tab context menus, completion-popup foundations, richer literal
find/replace, and an interactive vertical scrollbar.

Still deferred:

- directional geometric focus rather than depth-first traversal;
- keyboard splitter resizing and keyboard tab movement;
- pane topology and tab metadata persistence in session state;
- arbitrary-depth cascading menus and asynchronous completion providers;
- native watcher backends and incremental workspace subtree replacement.
