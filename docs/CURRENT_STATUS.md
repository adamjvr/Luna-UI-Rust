# Current Status

**Milestone:** M3.2a document identity and lifecycle model

## Verified baseline

The project owner locally validated and ran the synchronized M3.1d.4 tree. File/Edit/Find/View/Help
now open real anchored dropdown menus, Control-P opens the independent command palette, and the full
menu correction is ready to remain the M3.1 baseline.

## Implemented in M3.2a

- new product-neutral `luna-documents` workspace crate;
- monotonic `DocumentId` allocation and stable UI/cache keys;
- monotonic Untitled-1, Untitled-2, ... naming that is not reused after close;
- adapter-canonicalized absolute `FileIdentity` values;
- duplicate-open prevention for canonical file identities;
- duplicate prevention for application-owned virtual document keys;
- saved edit revisions and dirty-state evaluation against current editor revisions;
- opaque storage revisions for future optimistic write-conflict checks;
- external InSync, Modified, and Missing states;
- explicit Save requirements: None, SaveAs, WriteFile, or Unsupported;
- explicit close requirements: Safe or SaveOrDiscard;
- Save As identity reassignment with duplicate-file protection;
- removal that releases file and virtual indexes;
- editor-demo migration from ad hoc string IDs/titles/saved revisions to `DocumentRegistry`;
- stable registry IDs projected into tabs, sidebar rows, text-layout caches, pointer routing, and
  accessibility;
- dirty-close blocking instead of silent text loss;
- status feedback for Save As, unsupported virtual writes, and pending close decisions;
- unit coverage for identity, dirty/save/close decisions, external changes, reassignment, duplicate
  prevention, and editor integration.

## Runtime scope

M3.2a establishes lifecycle decisions but performs no filesystem write and shows no native dialog.
Open and Save As remain disabled in the visible menus until M3.2b supplies testable adapter
interfaces and host implementations.

## Next milestone

M3.2b adds real UTF-8 open/save/save-as and close-resolution services behind product-neutral
filesystem and dialog contracts. The implementation should support mock adapters in tests before a
native winit/platform adapter is connected.
