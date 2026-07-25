# Current Status

**Milestone:** M3.2c recent files and external-change delivery

## Verified baseline

The project owner locally validated and committed M3.2b.1. Strict UTF-8 Open, atomic Save and Save
As, native Linux dialogs, protected dirty close, duplicate activation, and optimistic conflict
resolution are the accepted baseline.

## Implemented in M3.2c

- opaque `StorageInstance` and combined `StorageSnapshot` lifecycle values;
- Modified, Replaced, Missing, and Recreated external-state distinctions;
- Unix device/inode storage instances and deterministic in-memory instances;
- `TextFileService::observe_file` for present/missing snapshots;
- bounded MRU `RecentFileList` with canonical identities;
- recent-file File-menu and command-palette projections;
- duplicate recent-file activation and clear-recent command;
- 750 ms observation through `NativeApplication::update` on the UI thread;
- no redraw or accessibility update when observations are unchanged;
- active-document status and accessibility notices for external state;
- dynamically enabled Reload from Disk command;
- explicit reload that replaces editor text, resets scroll, updates the baseline, and clears state;
- Save/conflict policy for clean documents whose storage changed or disappeared;
- deterministic tests for in-place modification, replacement, missing, recreation, MRU behavior,
  recent command routing, polling transitions, and reload.

## Runtime scope

The editor continuously polls open file-backed documents without a worker thread. This is the first
safe delivery contract: storage adapters observe bytes and metadata, while editor mutation stays on
the native UI thread. Unix file instances distinguish an in-place write from atomic replacement.

Recent files are intentionally in memory. Luna does not yet choose a product profile directory,
persistence format, retention policy, or privacy behavior.

## Next milestone

M3.2d introduces workspace/folder adapters, recursive project-tree snapshots, expansion state, and
sidebar activation from real filesystem content. Persistent recents and native event watcher
backends remain later application/platform adapters.
