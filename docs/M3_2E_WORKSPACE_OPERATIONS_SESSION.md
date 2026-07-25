# M3.2e Workspace Operations and Session Runtime

## Purpose

M3.2d made workspace trees real but read-only. M3.2e adds controlled filesystem mutation and
persistent editor-session state without pushing product policy into rendering widgets or the
workspace snapshot model.

The phase also establishes two later editor-runtime seams:

- multiple independent editor views may share one document-buffer identity;
- native filesystem watchers may deliver path-level events to the UI thread and request full or
  subtree refreshes.

The editor demo still uses one view per open document and one-second safe polling. Split panes and a
native watcher backend remain later work, but their identity and delivery boundaries now exist.

## Workspace mutation contracts

`luna-workspaces` now separates three responsibilities:

```text
WorkspaceService          recursive immutable scans
WorkspaceMutationService  create / rename / delete operations
WorkspaceRuntimeService   combined application-facing boundary
```

Supported mutations are:

- create an empty regular file;
- create one directory;
- rename one file, directory, or symlink within its current parent;
- delete one file, symlink, or recursive directory tree.

Names are rejected when they are empty, `.` or `..`, contain a path separator, contain NUL, or
represent more than one path component.

## Collision policy

Mutation calls use explicit `WorkspaceCollisionPolicy` values:

- `FailIfExists` is the default for every operation;
- `ReplaceFile` may replace an existing regular file only.

Directories and symbolic links are never replaced through collision policy. The editor first tries
the non-destructive operation, then requests explicit user confirmation before retrying a permitted
regular-file replacement.

The standard-library adapter rejects symlink replacement and uses `symlink_metadata` for rename and
delete so a link entry is mutated rather than its target.

## Dirty-document behavior

A workspace rename updates every open file-backed document below the renamed path. The registry
changes canonical identity, title, and storage snapshot while preserving the saved edit revision.
A dirty buffer therefore stays dirty after its file or ancestor directory is renamed.

Workspace deletion resolves all affected open documents before touching storage:

```text
clean document  -> close after successful deletion

dirty document  -> Keep Open
                   Discard & Close
                   Cancel complete deletion
```

`Keep Open` detaches the buffer into a new monotonic Untitled identity. Its saved edit revision is
preserved, so local unsaved edits remain dirty and Save invokes Save As. `Discard & Close` closes the
view only after storage deletion succeeds. `Cancel` prevents the complete operation.

Recent-file identities beneath renamed paths are relocated in place. Deleted identities are removed
from recent history.

## Persistent session state

The new `luna-session` crate owns a small versioned persistence contract:

- `SessionStore` loads and atomically saves complete state;
- `StdSessionStore` uses the per-user XDG state directory;
- `MemorySessionStore` supports deterministic tests.

The default Linux path is:

```text
${XDG_STATE_HOME:-$HOME/.local/state}/luna-ui-rust/editor-session-v1.txt
```

Persisted state includes:

- recent files in MRU order;
- the last open workspace root;
- expanded workspace directory paths;
- the selected workspace path.

Paths and titles are hexadecimal encoded in a versioned text format. On Unix, exact `OsStr` bytes
are preserved rather than forcing paths through lossy UTF-8 conversion.

Session writes use a same-directory temporary file and rename. The editor persists state after
meaningful recent-file, workspace, expansion, selection, and mutation changes. Startup restores the
workspace only when it can still be scanned. A stale workspace produces a visible notice without
preventing the editor from launching.

Closing the workspace persists an empty workspace field, so it does not reopen on the next launch.

## Shared document/view identity

`luna-documents` now includes `DocumentViewId`, `DocumentViewRecord`, and
`DocumentViewRegistry`. Multiple view records may reference the same `DocumentId` buffer identity.
Caret, selection, scroll, folding, and pane placement remain independent application-owned view
state.

The current editor demo still creates one view per document. M3.3 split panes can add another view
without duplicating file identity, dirty state, or storage observation.

## Watcher and incremental refresh boundaries

`WorkspaceWatchService` defines native-watcher delivery without allowing a background backend to
mutate UI state. It provides:

- activation for one canonical workspace root;
- draining path-level `WorkspaceWatchEvent` values on the caller's UI thread;
- Created, Modified, Removed, Renamed, and RescanRequired classifications.

`MemoryWorkspaceWatchService` provides deterministic delivery tests. `WorkspaceRefreshScope`
represents either a complete refresh or a future directory-subtree refresh.

The editor continues to use its known-correct one-second full snapshot polling in M3.2e. A native
inotify/FSEvents/ReadDirectoryChanges backend and subtree reconciliation can replace the source of
refresh hints later without changing the UI-thread ownership rule.

## Dialog boundaries

`DocumentDialogService` now additionally owns:

- workspace leaf-name prompts;
- regular-file replacement confirmation;
- file/directory deletion confirmation;
- dirty affected-document Keep Open / Discard & Close / Cancel resolution.

Zenity and KDialog adapters implement the native dialogs. `ScriptedDialogService` exposes queues for
all choices so editor integration remains deterministic.

## Editor command integration

While a workspace is open, File and the command palette expose:

- New File in Workspace…
- New Folder…
- Rename Workspace Entry…
- Delete Workspace Entry…

Rename and Delete are disabled for the workspace root. Creation targets the selected directory, or
the selected file's parent directory.

Every successful mutation refreshes the workspace, preserves surviving tree state, selects the
resulting path when appropriate, and persists the new session state.

## Test coverage

M3.2e adds coverage for:

- leaf-name validation and collision errors;
- standard-library and in-memory create/rename/delete round trips;
- recursive directory rename and deletion;
- replacement limited to regular files;
- persisted session encode/decode and memory round trips;
- recent-file relocation and restoration;
- document relocation preserving dirty state;
- dirty file detachment to Untitled;
- multiple view identities sharing one document buffer;
- expansion and selection restoration from paths;
- watcher event root enforcement and draining;
- scripted mutation dialogs;
- editor create, rename, and delete command integration;
- dirty deletion Keep Open behavior;
- dynamic workspace mutation command projection;
- session restoration of recents, workspace root, expansion, and selection.

## Deferred

- native inotify/FSEvents/ReadDirectoryChanges watcher implementations;
- incremental immutable-subtree reconciliation;
- cross-process session locking and merge policy;
- reopening the previous document-tab set;
- split panes that instantiate multiple live views of one shared document buffer;
- undoable filesystem operations and trash/recycle-bin integration.
