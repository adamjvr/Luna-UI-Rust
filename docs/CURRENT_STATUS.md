# Current Status

**Milestone:** M3.2b file and dialog service boundaries

## Verified baseline

The project owner locally validated and committed M3.2a. Stable document IDs, monotonic untitled
naming, duplicate file identity, dirty-state evaluation, and protected close decisions are the
accepted baseline for this phase.

## Implemented in M3.2b

- new `luna-document-services` workspace crate;
- product-neutral synchronous `TextFileService` contract;
- strict UTF-8 loading with typed invalid-encoding failures;
- deterministic content revisions for optimistic write checks;
- canonical identities for existing and not-yet-created Save As destinations;
- same-directory temporary writes, existing-permission preservation, and atomic replacement;
- `WritePrecondition::Any`, `Missing`, and `Matches` policies;
- typed conflict details with expected and observed revisions;
- product-neutral `DocumentDialogService` contract;
- Open, Save As, dirty-close, and save-conflict dialog decisions;
- Linux native dialog adapter using Zenity with KDialog fallback;
- deterministic `MemoryTextFileService` and `ScriptedDialogService` adapters;
- editor-demo Open, Save, Save As, duplicate-open activation, and file-title reassignment;
- Save/Discard/Cancel dirty-close resolution;
- Overwrite/Reload/Cancel optimistic-conflict resolution;
- canceled or failed operations that preserve editor content and dirty state;
- real Control-O and Control-Shift-S shortcuts;
- enabled File-menu and command-palette projections for Open and Save As;
- unit coverage for UTF-8 loading, invalid bytes, atomic-write preconditions, dialog scripting,
  duplicate open, Save As, normal save, close decisions, and conflict reload.

## Runtime scope

The editor now performs real UTF-8 file I/O and native desktop dialogs. The current Linux dialog
adapter intentionally uses installed desktop helpers rather than linking a GUI toolkit into Luna.
Zenity is preferred and KDialog is the fallback. When neither helper is present, the application
reports an explicit dialog-unavailable status and does not alter document state.

M3.2b does not yet persist recent files, watch storage continuously, populate a folder workspace, or
share one buffer across independent pane views.

## Next milestone

M3.2c adds recent-file state, external-change observation and delivery, reload notifications,
missing-file handling, and further save-conflict hardening. M3.2d then introduces workspace/folder
adapters and real project-tree snapshots.
