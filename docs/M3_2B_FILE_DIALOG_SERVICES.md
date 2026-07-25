# M3.2b File and Dialog Services

## Purpose

M3.2a deliberately stopped at document identity and lifecycle decisions. M3.2b executes those
decisions through testable byte-storage and user-choice boundaries while preserving the rule that
`luna-documents` owns neither filesystem operations nor product UI.

The phase adds `luna-document-services` and integrates it into the native editor demo.

## Service contracts

### TextFileService

`TextFileService` exposes four synchronous operations:

```text
load_utf8(path)
identity_for_save(path)
observe_file(path)
write_utf8_atomic(path, text, precondition)
```

A successful load returns:

- strict decoded UTF-8 text;
- an adapter-canonicalized `FileIdentity`;
- a `StorageSnapshot` containing a deterministic content revision and concrete storage instance.

Invalid UTF-8 is an explicit `InvalidUtf8` error. The adapter never substitutes replacement
characters because silent conversion would make later saves destructive.

### Write preconditions

Every write selects one explicit policy:

```text
Any                 replace/create after product confirmation
Missing             write only when no destination exists
Matches(snapshot)   write only when content and storage instance still match the baseline
```

Ordinary Save uses `Matches` whenever a baseline snapshot exists and `Missing` when no baseline is
available. Save As uses `Any` only after the native chooser has confirmed replacement. A mismatch
returns a typed `Conflict` containing expected and observed content revisions.

### Atomic replacement

`StdTextFileService` writes UTF-8 bytes to a uniquely named temporary file in the destination
directory, copies existing destination permissions when applicable, synchronizes the temporary file,
rechecks the optimistic precondition, renames it over the destination, and synchronizes the parent
directory on Unix. A failed operation attempts to remove the temporary file and leaves the registry
dirty.

The destination identity resolves an existing file, including symlinks, through canonicalization.
For a new destination, the parent directory is canonicalized and the selected file name is appended.

## Dialog contracts

`DocumentDialogService` owns modal user choices without depending on Luna widgets:

```text
choose_open_file          -> path or cancel
choose_save_file          -> path or cancel
confirm_dirty_close       -> Save / Discard / Cancel
resolve_save_conflict     -> Overwrite / Reload / Cancel
```

`SystemDialogService` is a Linux leaf adapter:

1. prefer Zenity when installed;
2. fall back to KDialog;
3. return `DialogErrorKind::Unavailable` when neither helper exists.

Commands are launched directly with argument arrays and never through a shell. The editor reports an
unavailable or failed dialog in the status bar and preserves document state.

## Editor orchestration

### Open

```text
Open command
    -> native file selection
    -> strict UTF-8 load
    -> canonical identity
    -> DocumentRegistry::register_file
         -> Opened: create editor view and activate it
         -> AlreadyOpen: activate existing view
```

### Save

```text
SaveRequirement::None
    -> no write

SaveRequirement::SaveAs / Unsupported
    -> Save As flow

SaveRequirement::WriteFile
    -> optimistic Matches(snapshot) write
         -> success: mark_saved
         -> conflict: Overwrite / Reload / Cancel
```

Virtual proof documents may now use Save As to become ordinary file-backed documents. They are not
silently written until a destination is selected.

### Dirty close

```text
CloseRequirement::Safe
    -> remove view and lifecycle record

CloseRequirement::SaveOrDiscard
    -> Save: save, then close only on success
    -> Discard: close without writing
    -> Cancel: preserve document and focus
```

### Conflict reload

Reload performs a strict fresh load, replaces the active `EditableText`, resets scroll and retained
text-layout state, updates the registry baseline snapshot, refreshes find matches, and leaves the
document clean.

## Deterministic adapters

`MemoryTextFileService` stores bytes in a shared in-memory map. It supports UTF-8 and arbitrary-byte
insertion, canonical test identities, snapshot checks, observation, and all write preconditions.

`ScriptedDialogService` queues open paths, save paths, dirty-close choices, and conflict choices.
Clones share the same queues, allowing the editor to own a trait object while tests retain a handle
for assertions and later scripted responses.

## Validation expectations

- Open loads UTF-8 text and refuses invalid bytes.
- Opening the same canonical file twice activates the existing tab.
- Save As writes content, assigns file identity, updates the title, and clears dirty state.
- Save uses the last storage snapshot as its optimistic precondition.
- An external change cannot be silently overwritten.
- Overwrite replaces storage only after an explicit conflict choice.
- Reload replaces editor content and clears dirty state.
- Cancel preserves both storage and editor content.
- Dirty close Save writes before removal.
- Dirty close Discard removes without writing.
- Dirty close Cancel leaves the tab open.
- File commands work through menus, shortcuts, the command palette, pointer activation, and
  accessibility.
- M3.1 retained text/menu/gallery behavior remains unchanged.

## Delivered by M3.2c/M3.2d and deferred later

M3.2c adds in-memory recent-file projection, storage observation, modified/replaced/missing/recreated
state, UI-thread polling delivery, status/accessibility notices, and Reload from Disk. M3.2d adds
Open Folder, product-neutral recursive workspace snapshots, real sidebar tree rows, expansion,
selection, refresh preservation, and duplicate-safe file activation.

Still deferred:

- persistent recent-file and workspace/session storage;
- native event watcher backends and incremental subtree delivery;
- workspace create, rename, and delete operations;
- shared buffers with independent pane views;
- cross-platform native dialog adapters beyond the current Linux helper implementation.
