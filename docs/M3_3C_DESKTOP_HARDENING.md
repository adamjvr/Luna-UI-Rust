# M3.3c Desktop Interaction Hardening

## Purpose

M3.3c closes the remaining desktop-runtime gaps before Luna begins the M4 GPU backend. It preserves
M3.3b's product-neutral crate boundaries while making pane sessions durable, completion delivery
asynchronous, popup traversal recursive, workspace watching native-first, and common editor actions
available from the keyboard.

## Durable editor sessions

`luna-session` now writes the versioned `LUNA_EDITOR_SESSION_V2` format. V1 recent-file and workspace
sessions remain readable. V2 stores:

- each shared document buffer once;
- file, untitled, and virtual source identity;
- dirty state and file storage revision/instance baselines;
- each independent `DocumentViewId` presentation;
- caret, directional selection, and pane-local scroll state;
- recursive horizontal and vertical pane topology and split ratios;
- focused pane, pane-local tab order, active views, pinned partitions, preview state, and overflow
  offsets.

The decoder rejects duplicate identities, orphaned pane records, invalid split ratios, missing view
references, duplicate tab ownership, impossible active/focused identities, invalid pinned partitions,
multiple previews, negative scroll positions, incomplete selections, and storage snapshots on
non-file documents. Clean file-backed preview tabs remain previews; dirty, untitled, or virtual tabs
restore as ordinary durable tabs.

Persisted file storage baselines are compared with current storage after restart. Luna therefore
retains the distinction between in-place modification, atomic replacement, missing files, and
recreation instead of silently treating the restart-time file as the editor's saved baseline.

## Pane keyboard operations

`luna-panes` exposes deterministic active-tab movement:

- move left or right inside the current pane;
- move to the previous or next pane in depth-first order;
- preserve `DocumentViewId`, shared `DocumentId`, pinned/regular partitions, active visibility, and
  safe source-pane collapse.

The editor demo routes these operations through Control-Shift-Left/Right and
Control-Alt-Shift-Left/Right.

## Arbitrary-depth popups

The dropdown runtime now represents selection as a recursive path instead of one optional child
index. Keyboard, pointer, layout, display, and accessibility projections support menus at arbitrary
depth. Parent-row geometry feeds a pointer-intent corridor that keeps an open child menu stable while
the pointer travels diagonally toward it. A four-panel Help/Diagnostics/Runtime/Session path is
included in the editor demo as a live proof.

`luna-ui::CascadingMenuState` additionally provides product-neutral logical hover delays and reusable
panel-path geometry for applications that need explicit delayed switching policy.

## Asynchronous completion

`luna-ui` now defines:

- monotonic completion request IDs;
- immutable view/revision/caret/prefix request context;
- explicit UTF-8 replacement ranges per candidate;
- cloneable response delivery suitable for worker threads;
- cancellation;
- UI-thread draining with stale request, stale view, and stale revision rejection;
- deterministic scripted-provider tests.

The editor demo uses a real delayed worker-thread provider in production and a zero-delay equivalent
under tests. A loading popup remains visible until the logical UI update cadence drains a valid
response.

## Search and scrollbar hardening

The find panel adds bounded most-recently-used search history, wrap-around navigation, and a frozen
Find in Selection scope. Existing match-case, whole-word, Replace Current, and Replace All behavior
continues to use stable UTF-8 ranges. Scrollbar track presses classify as page-up, thumb drag, or
page-down using the same geometry used for paint and accessibility.

## Native-first workspace delivery

`LinuxWorkspaceWatchService` attempts recursive `inotifywait` delivery first and falls back to a
safe standard-library metadata poller when the utility is unavailable or exits. Events are
coalesced deterministically, excessive bursts become a full-rescan request, and the smallest common
directory is selected for subtree refresh. Reconciliation preserves stable path-derived node IDs,
expansion, selection, and unaffected subtrees. All model mutation remains on the application UI
thread.

## Validation boundary

The portable source delivery includes deterministic model tests, runtime fixtures, and
`scripts/test-m3-3c.sh`. The authoritative Pop!_OS gate remains:

```bash
cargo fmt --all
./scripts/test-m3-3c.sh
```

M4 must not begin until formatting, compiler checking, strict Clippy, all tests, rustdoc, native
runtime behavior, accessibility, session restart, watcher fallback, and proof-gallery regressions
pass locally.
