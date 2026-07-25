# Current Status

**Milestone:** M3.2e workspace operations and persistent session runtime

## Verified baseline

The project owner locally validated and committed M3.2d. Real Open Folder behavior, recursive
workspace snapshots, stable node identities, expansion and selection preservation, accessibility,
and UI-thread refresh are the accepted baseline.

## Implemented in M3.2e

- product-neutral create-file, create-folder, rename, and recursive-delete contracts;
- explicit `FailIfExists` and regular-file-only `ReplaceFile` collision policies;
- standard-library and deterministic in-memory mutation adapters;
- leaf-name validation that rejects empty, reserved, multi-component, separator, and NUL input;
- native Zenity/KDialog prompts and confirmations with scripted test equivalents;
- workspace-root protection for rename and delete;
- open-document identity relocation when a file or ancestor directory is renamed;
- dirty-state preservation through file relocation;
- dirty-delete Keep Open, Discard & Close, or Cancel policy;
- Keep Open detachment into a new monotonic Untitled identity without losing local edits;
- recent-file relocation/removal after workspace mutation;
- new `luna-session` crate with versioned atomic session storage;
- persistent recent files, workspace root, expanded paths, and selected path;
- XDG state-directory resolution with exact Unix path-byte preservation;
- startup workspace/session restoration with visible stale-session failures;
- product-neutral `DocumentViewRegistry` seam for multiple views sharing one document buffer;
- native watcher event and full/subtree refresh-scope boundaries;
- focused `scripts/test-m3-2e.sh` quality gate and runtime fixture generator.

## Runtime scope

The proof editor still owns one live editor view per open document and uses its deterministic
one-second complete workspace rescan. M3.2e establishes the buffer/view identity and watcher-event
contracts without pretending that split panes or a native inotify backend already exist.

Filesystem operations are destructive only after explicit application policy. Regular files may be
replaced after confirmation; directories and symlinks are never collision-replaced. Recursive
delete confirms first and resolves every affected dirty document before touching storage.

The standard session file is:

```text
${XDG_STATE_HOME:-$HOME/.local/state}/luna-ui-rust/editor-session-v1.txt
```

## Next milestone

M3.3a begins the expanded editor shell with recursive split panes, draggable splitters, shared
document buffers, independent caret/selection/scroll state per view, pane focus traversal, and
pane-aware tab ownership. Native watcher backends and incremental subtree reconciliation remain a
later platform-hardening step behind the seams introduced in M3.2e.
