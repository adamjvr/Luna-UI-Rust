# Current Status

**Milestone:** M3.2d workspace and project-tree runtime

## Verified baseline

The project owner locally validated and committed M3.2c. Recent files, storage snapshots,
modified/replaced/missing/recreated detection, UI-thread file observation, and explicit Reload from
Disk are the accepted baseline.

## Implemented in M3.2d

- new product-neutral `luna-workspaces` crate;
- exact stable node IDs derived from absolute normalized path bytes;
- immutable recursive workspace snapshots and path indexes;
- deterministic directories-before-files ordering;
- explicit hidden-file, symlink, and maximum-depth policies;
- symlinks shown as non-followed leaves by default;
- Available, PermissionDenied, DepthLimit, and Unreadable node projection;
- mutable expansion, selection, ancestor reveal, and monotonic model generation;
- refresh reconciliation that preserves surviving expansion and selection;
- unchanged-snapshot suppression;
- standard-library recursive scanner;
- deterministic in-memory workspace adapter;
- native Open Folder through Zenity or KDialog;
- scripted folder-dialog results for tests;
- File-menu, command-palette, shortcut, pointer, and accessibility integration;
- real workspace rows in the editor sidebar;
- folder expansion/collapse and file activation through the existing UTF-8 Open path;
- duplicate workspace-file activation instead of duplicate tabs;
- one-second workspace refresh through the UI-thread update contract;
- automatic row appearance/disappearance after external create, rename, or delete;
- Close Workspace restoration of the Open Documents fallback;
- focused `scripts/test-m3-2d.sh` quality gate and runtime fixture generator.

## Runtime scope

The proof editor owns one workspace at a time. Opening another folder replaces the current workspace
model without closing documents. Saving into the workspace refreshes and reveals the new path.
Workspace scans are synchronous and delivered entirely on the native UI thread in this phase.
Identical snapshots request no frame; changed snapshots invalidate only widget layout.

Hidden dot entries are excluded. Symlinks are visible but never followed. Recursive scans stop at a
configurable depth and retain partial trees when child directories are unreadable.

## Next milestone

M3.2e adds workspace file/folder create, rename, and delete operations; operation confirmation and
conflict policy; persistent workspace/session state; and shared document buffers with independent
editor views. Native event watcher adapters and incremental subtree refresh remain later platform
hardening work.
