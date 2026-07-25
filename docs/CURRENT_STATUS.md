# Current Status

**Milestone:** M3.3a recursive split-pane runtime

## Verified baseline

The project owner locally validated and committed M3.2e.2. Controlled workspace mutation,
persistent recent/workspace state, dirty-document deletion policy, strict Clippy fixes, the editor
demo, and the proof gallery are the accepted baseline.

## Implemented in M3.3a

- new product-neutral `luna-panes` crate;
- recursive binary pane trees with stable leaf and split identities;
- horizontal and vertical split commands;
- pane-local ordered tabs and active-view ownership;
- multiple `DocumentViewId` values sharing one `DocumentId` lifecycle buffer;
- synchronized shared text revisions with independent caret, selection, and scroll;
- per-view retained text-layout cache identities;
- deterministic depth-first pane focus traversal with wrapping;
- draggable splitters with clamped ratios and minimum pane extents;
- pane collapse after closing the final local view;
- protection against closing the final editor pane;
- document rehoming when a closed pane held a document's only live view;
- reusable `EditorPaneSurface` paint, hit-test, label, and accessibility geometry;
- pane groups, tab lists, tab nodes, close buttons, editor groups, and splitter semantics;
- pane-aware Open, New, Save, Find, Close, workspace rename/delete, pointer, and keyboard routing;
- focused `scripts/test-m3-3a.sh` quality gate and runtime fixture generator.

## Runtime scope

One `DocumentId` still owns file identity, storage observation, dirty baseline, and save/close
policy. Each pane tab owns an independent `DocumentViewId` plus caret, selection, scroll, focus,
semantic identity, and retained layout cache.

The editor synchronizes immutable text snapshots and edit revisions after each edit. This provides a
shared logical buffer without concurrent mutable aliases and preserves the single UI-lane rule.
Pane topology is not yet persisted in session state.

The existing one-second workspace polling remains the safe runtime source for tree refreshes.
M3.3a does not claim a native watcher backend or incremental subtree replacement.

## Local validation required

Run:

```bash
./scripts/test-m3-3a.sh
```

Then verify recursive splits, shared edits, independent caret/selection/scroll, splitter dragging,
pane focus traversal, pane-local tabs, close/collapse/rehoming behavior, accessibility actions,
menus, command palette, file/workspace regressions, and the proof gallery.

## Next milestone

M3.3b adds advanced tab behavior and richer desktop command surfaces: tab reordering and movement,
pinned/preview/overflow policy, nested menus, context menus, and completion-popup foundations.
