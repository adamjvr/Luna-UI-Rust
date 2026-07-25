# Current Status

**Milestone:** M3.3b advanced tabs and desktop popup surfaces

## Verified baseline

The project owner locally validated and committed M3.3a.2. Recursive split panes, synchronized
shared documents, independent pane-view state, splitter interaction, pane-aware close behavior,
strict compiler/Clippy fixes, the editor demo, and the proof gallery are the accepted baseline.

## Implemented in M3.3b

- pinned pane-local tabs with deterministic leading order;
- one clean replaceable preview tab per pane;
- automatic preview promotion after editing and explicit Keep Preview Open;
- local tab drag reordering;
- cross-pane tab movement without reopening or copying documents;
- source-pane collapse when its final tab is moved away;
- pane-local regular-tab overflow offsets;
- compact pinned tabs outside the regular overflow viewport;
- previous/next overflow controls and active-tab visibility correction;
- reusable submenu definitions, child-panel geometry, keyboard traversal, and mnemonics;
- Open Recent and Tabs submenus in the desktop menu bar;
- tab context menu with exact Move to Pane choices;
- reusable caret-anchored completion popup;
- completion keyboard, pointer, and accessibility activation;
- match-case and whole-word find options;
- Replace Current and Replace All commands;
- vertical scrollbar track, thumb, hit testing, and pointer mapping;
- expanded accessibility for popup lists, submenus, tab state, find options, and overflow buttons;
- focused `scripts/test-m3-3b.sh` quality gate and runtime fixture generator.

## Runtime scope

`PaneTree` owns tab order, pinned/preview metadata, overflow offsets, and movement policy. The editor
application still owns document lifecycle decisions and keeps one canonical `DocumentId` buffer
synchronized across pane-local `DocumentViewId` presentations.

The completion popup is a deterministic runtime proof with an application-supplied candidate list.
It does not claim a language-server protocol, asynchronous result delivery, or Moth-owned completion
provider. Find remains literal rather than regex-based.

Dropdown menus currently support a top-level panel plus one child submenu panel. The command model
can contain deeper definitions, but arbitrary-depth cascading presentation, hover delays, and pointer
intent handling remain deferred.

The existing one-second workspace polling remains the safe runtime source for tree refreshes. M3.3b
does not claim a production native watcher backend or incremental subtree replacement.

## Local validation required

Run:

```bash
./scripts/test-m3-3b.sh
```

Then verify pinned/preview policy, overflow controls, drag reorder, cross-pane movement, nested menus,
Alt mnemonics, tab context commands, completion insertion, find options and replacement, scrollbar
dragging, existing file/workspace/session behavior, accessibility actions, and the proof gallery.

## Next milestone

M3.3c will harden desktop interaction and persistence: pane/tab session restoration, keyboard tab
movement, arbitrary-depth popup routing, asynchronous completion-provider boundaries, richer search
history/options, and production-oriented native watcher delivery before the GPU backend begins.
