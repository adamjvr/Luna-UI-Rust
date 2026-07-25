# M3.2d Workspace and Project-Tree Runtime

## Purpose

M3.2a through M3.2c made individual document identity, persistence, recent-file state, and external
change delivery real. M3.2d adds the missing folder-level runtime: an application can choose a
workspace root, scan it through a product-neutral service, preserve tree interaction state across
refreshes, and open files directly from real sidebar rows.

The phase deliberately stops before filesystem mutation policy. Create, rename, delete, persistent
workspace sessions, and shared-buffer pane views remain M3.2e work.

## `luna-workspaces`

The new `luna-workspaces` crate owns five related contracts:

1. exact stable node identities derived from absolute normalized filesystem paths;
2. immutable recursive `WorkspaceSnapshot` values;
3. `WorkspaceModel` expansion, selection, reveal, and refresh state;
4. `WorkspaceService` scan boundaries;
5. standard-library and deterministic in-memory adapters.

It does not own native dialogs, editor tabs, text decoding, file mutation, or product command policy.

## Stable identities

`WorkspaceNodeId` encodes the exact platform path bytes into a deterministic UI-safe key. The same
normalized path therefore retains the same node identity across rescans, even when sibling ordering
or unrelated files change.

Stable identity is used by:

- sidebar rows;
- pointer hit routing;
- accessibility nodes;
- expansion state;
- selection state;
- refresh reconciliation.

A file that is renamed receives a new identity because its path changed. Unrelated surviving nodes
keep their prior identities.

## Recursive snapshots

A `WorkspaceSnapshot` stores:

```text
canonical root
root node ID
stable node map
absolute-path index
deterministic snapshot fingerprint
```

Each `WorkspaceNode` stores its absolute path, root-relative path, display title, object kind,
availability state, and ordered child IDs.

Node kinds are:

- Directory
- File
- Symlink

Symlinks are never followed in this phase. They may be excluded or shown as non-openable leaves.
This prevents cycles and avoids silently escaping the selected workspace root.

## Visibility and scan policy

`WorkspaceScanOptions` makes policy explicit:

- dot-prefixed entries may be included or excluded;
- symbolic links may be excluded or shown as leaves;
- recursion has a configurable maximum depth.

The editor demo defaults to:

```text
hidden files: excluded
symlinks: shown as leaves
maximum depth: 64
```

Children are sorted deterministically with directories first, regular files second, and symlinks
last. Names are compared case-insensitively first and then by exact title and stable identity.

## Error projection

A failed child scan does not invalidate the complete workspace. Nodes carry one of these states:

- Available
- PermissionDenied
- DepthLimit
- Unreadable(message)

Unavailable directories remain visible as collapsed leaves. The sidebar appends a concise state to
the visible title, while activation places the detailed path/error in the status bar. Failure to
scan the root itself remains a typed `WorkspaceError` because no valid workspace can be constructed.

## Expansion, selection, and reveal

`WorkspaceModel` owns mutable interaction state over an immutable snapshot.

The root is expanded by default. Directory activation toggles expansion only when the node is
available. Opening or activating a workspace file reveals it by expanding all surviving directory
ancestors and selecting its stable row.

Refresh preserves:

- expansion for surviving available directories;
- selection for a surviving node;
- root expansion.

Selection is cleared when its node disappears. A meaningful snapshot change increments a monotonic
generation. An identical snapshot produces no change and no frame request.

## Native Open Folder boundary

`DocumentDialogService` now includes `choose_open_folder`. `SystemDialogService` implements it with:

- Zenity `--file-selection --directory`; or
- KDialog `--getexistingdirectory`.

`ScriptedDialogService` gains a deterministic queued folder result for integration tests. The editor
continues to own command policy while the dialog adapter owns only native selection.

## Editor integration

The editor adds these File commands:

- Open Folder… (`Ctrl+Shift+O`)
- Refresh Workspace (`Ctrl+Shift+R`, only while a workspace is open)
- Close Workspace (only while a workspace is open)

When no workspace is open, the sidebar retains the Open Documents fallback. When a workspace is
open, the sidebar projects the real visible rows from `WorkspaceModel`.

Sidebar activation behavior is:

```text
available directory -> select and toggle expansion
available file      -> Open through the existing UTF-8 document service
symlink             -> select and report non-followed policy
unavailable node    -> select and report status/path
```

Opening a file already represented by a document activates the existing tab. Saving a new file into
the workspace silently refreshes the tree and reveals the resulting row without replacing the Save
status notice.

## Refresh delivery

The editor performs a one-second recursive refresh through `NativeApplication::update`. The scan and
model mutation both occur on the native UI thread in this proof phase. Identical snapshots produce
no invalidation. A changed snapshot requests `WidgetLayout` while preserving text-layout caches.

This synchronous polling is intentionally simple and deterministic. A future platform watcher may
produce snapshots off-thread, but delivery into editor state must still occur through the native UI
thread contract.

## Accessibility

Workspace rows reuse the editor shell's tree/tree-item semantics and the same geometry used for
paint and pointer hit testing. Folder rows expose expanded state. Selected rows expose selection,
and error-state suffixes are included in accessible labels.

## Test coverage

The phase adds coverage for:

- exact stable path identities;
- invalid relative identity rejection;
- directories-before-files sorting;
- hidden-entry policy;
- symlink exclusion/show-as-leaf policy;
- expansion and visible-row flattening;
- ancestor reveal;
- refresh preservation and unchanged suppression;
- removed-selection cleanup;
- permission and depth-limit projection;
- standard-library recursive scanning;
- non-followed real symlink directories on Unix;
- native/scripted Open Folder routing;
- editor Open Folder projection;
- folder expansion and file activation;
- duplicate workspace-file activation;
- refresh adding rows while preserving expansion;
- symlink activation policy;
- Close Workspace fallback;
- dynamic menu and command-palette projection.

## Deferred to M3.2e

- create file/folder operations;
- rename and delete operations;
- operation confirmation and conflict policy;
- persistent workspace/session restoration;
- shared document buffers with multiple independent editor views;
- native event watcher backends and incremental subtree updates.
