# M3.2a Document Identity and Lifecycle

## Purpose

M3.2a introduces the product-neutral document model that real file, dialog, workspace, and watcher
adapters will consume in later M3.2 subphases. It intentionally does not perform filesystem I/O or
show native dialogs.

The central rule is:

```text
storage adapters own bytes and canonicalization
luna-documents owns identity and lifecycle decisions
editor applications own views, caret, selection, scroll, and product policy
```

## New crate: `luna-documents`

The crate has no platform or UI dependencies. It provides:

- `DocumentId`, a monotonic application-local identity suitable for tabs, caches, and semantics;
- `FileIdentity`, an absolute adapter-canonicalized path used for duplicate-open prevention;
- `StorageRevision`, an opaque adapter-provided content token;
- `StorageInstance` and `StorageSnapshot`, which pair content with a concrete storage object;
- `DocumentSource` for untitled, file-backed, and virtual/generated documents;
- `DocumentRecord`, which retains saved edit revision, storage snapshot, and external state;
- `DocumentRegistry`, which owns identity allocation, monotonic untitled numbering, and file/virtual
  indexes;
- `SaveRequirement`, which distinguishes no-op, Save As, direct write, and unsupported sources;
- `CloseRequirement`, which distinguishes safe close from Save/Discard/Cancel policy;
- `ExternalState`, which records in-sync, externally modified, replaced, missing, and recreated states.

## File identity boundary

`FileIdentity::from_canonical_path` accepts only absolute paths without `.` or `..` components. Luna
does not claim that lexical path comparison solves symlinks, filesystem case rules, sandbox
bookmarks, or platform aliases. The filesystem adapter must resolve those details first and then
supply the canonical identity.

This separation prevents platform assumptions from leaking into document state while still making
same-file duplicate prevention deterministic.

## Save lifecycle

Save is represented as a decision, not an implicit side effect:

```text
untitled document        -> SaveAs
virtual/generated source -> Unsupported
clean file document      -> None
dirty file document      -> WriteFile(identity, expected storage snapshot, external state)
```

A successful adapter write calls `mark_saved`, which advances the saved edit revision, installs the
new storage snapshot, and clears external conflict state.

## Close lifecycle

A close request evaluates the current editor revision against the saved baseline:

```text
clean document -> Safe
dirty document -> SaveOrDiscard
```

M3.2a does not invent product policy for the second case. M3.2b now resolves the requirement
through the separate `DocumentDialogService`, preserving the registry as a policy-neutral decision
model.

## External changes

M3.2c observation adapters call either:

- `observe_storage_snapshot`, which returns to `InSync` for an exact baseline match and otherwise
  distinguishes `Modified`, `Replaced`, or `Recreated`;
- `observe_missing_file`, which records `Missing`.

The next Save decision carries this external state so Moth or another application can choose reload,
overwrite, save-copy, or cancel policy without pushing that policy into Luna.

## Editor-demo integration

The editor demo now keeps editor view state in `DemoDocument` and lifecycle metadata in a
`DocumentRegistry`:

```text
DemoDocument
  -> DocumentId
  -> EditableText + caret/selection + scroll

DocumentRegistry
  -> title/source
  -> saved edit revision
  -> storage snapshot
  -> external state
```

Visible effects:

- tabs and sidebar rows use stable registry IDs;
- untitled documents are empty and clean when created;
- untitled names increase monotonically and are not reused after close;
- dirty close is blocked instead of silently discarding text;
- Save on untitled documents reports that Save As is required;
- virtual proof documents never pretend that a real write succeeded;
- the status bar identifies Untitled, File, or Virtual source state.

## Follow-up status

M3.2b implements strict UTF-8 reading, deterministic storage snapshots, atomic writes, native Open
and Save As dialogs, dirty-close choices, and save-conflict resolution through the separate
`luna-document-services` crate.

M3.2c adds bounded recent files, UI-thread observation delivery, missing/recreated notices,
and explicit Reload from Disk behavior. M3.2d adds the product-neutral workspace tree, Open Folder,
sidebar expansion, selection, refresh preservation, and workspace-file activation. M3.2e adds
controlled mutation, persistent recent/workspace session state, and shared document/view identity.

Still deferred:

- native event-based watcher backends and incremental subtree reconciliation;
- live split panes with independent editor-view state;
- reopening the previous complete tab/pane set.
