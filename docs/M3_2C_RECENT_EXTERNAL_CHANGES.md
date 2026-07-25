# M3.2c Recent Files and External-Change Delivery

## Purpose

M3.2b made Open, Save, Save As, dirty close, and optimistic save conflicts real. M3.2c adds the
state that keeps those operations trustworthy after a document remains open: a bounded recent-file
list and continuous observation of the file backing each open document.

The implementation remains product-neutral. Luna does not persist a user profile, choose an
application-specific notification design, or mutate editor state from a worker thread.

## Recent-file model

`luna-documents` now provides `RecentFileList` and `RecentFileEntry`.

The list:

- stores canonical `FileIdentity` values rather than unverified strings;
- is ordered most-recently-used;
- moves duplicate identities to the front;
- has an explicit capacity;
- supports removal and clearing;
- remains in memory for this phase.

The editor demo records successful Open, duplicate Open activation, Save, and Save As operations.
The File menu and command palette project up to eight commands named `Open Recent: <file>` plus
`Clear Recent Files`. Selecting a missing recent file reports the failure and removes the identity
when it can still be canonicalized safely.

Persistence belongs to a later application/session adapter because Luna does not own user-profile
locations or product privacy policy.

## Storage snapshots

A content revision cannot distinguish an in-place modification from atomic replacement with the
same bytes. M3.2c therefore expands the document baseline to `StorageSnapshot`:

```text
StorageSnapshot
    content StorageRevision
    concrete StorageInstance
```

On Unix, `StdTextFileService` derives the storage instance from device and inode metadata. The
in-memory test adapter uses deterministic monotonic instance IDs. Other platform adapters may
provide an equivalent opaque identifier.

Loaded and written files now return complete snapshots. Successful load, Save, Save As, and Reload
replace the document baseline and clear external state.

## External-state distinctions

`DocumentRecord` now distinguishes:

- `InSync` — revision and instance match the saved baseline;
- `Modified` — the same storage object has different content;
- `Replaced` — the canonical path refers to a different storage object;
- `Missing` — the path no longer exists;
- `Recreated` — a file appeared after the path was observed missing.

The state is sticky until storage returns to the exact baseline or a successful load/save establishes
a new baseline. A recreated file therefore remains explicitly recreated instead of being silently
collapsed into an ordinary modification.

## UI-thread delivery

The editor requests a 750 ms logical update cadence through `NativeApplication::frame_interval`.
`NativeApplication::update` performs observation and mutates the document registry on the native UI
thread. There is no background watcher thread, cross-thread editor mutation, or platform event type
inside the document crates.

```text
winit WaitUntil
    -> NativeApplication::update(elapsed)
    -> TextFileService::observe_file(path)
    -> DocumentRecord external-state transition
    -> accessibility/status invalidation only when state changes
```

No redraw is requested when every observation is unchanged.

## User-facing behavior

When the active file changes externally, the status bar and its accessibility semantics announce the
condition. `Reload from Disk` becomes enabled in the File menu and command palette for Modified,
Replaced, and Recreated files.

Reload:

- performs a strict UTF-8 read;
- replaces editor text and scroll state;
- discards local edits only after explicit command activation;
- records the new storage snapshot;
- clears external state;
- invalidates retained text layout for that document.

For a missing file, Save remains available. The existing conflict flow can recreate the original path
through Overwrite, while Save As can choose another destination. A clean file with external state is
also considered to require Save/conflict policy rather than incorrectly reporting “already saved.”

## Test coverage

The deterministic adapters cover:

- in-place modification preserving a storage instance;
- atomic replacement changing the storage instance;
- missing observation;
- recreation after missing;
- MRU ordering, deduplication, capacity, removal, and clearing;
- recent-menu projection and duplicate activation;
- editor polling transitions across Modified, Replaced, Missing, and Recreated;
- explicit Reload restoring InSync state.

## Deferred

M3.2d introduces workspace/folder adapters and real project-tree snapshots. Persistent recent-file
storage, native event-based watcher backends, and session restoration remain application/platform
adapter work after the polling contract is proven.
