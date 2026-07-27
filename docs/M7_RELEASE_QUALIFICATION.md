# M7 Public API Stabilization and Release Qualification

M7 changes Luna from a sequence of accepted implementation phases into a framework with explicit
release evidence. It does not declare 1.0 stability. It establishes the tools and policies required
to make future compatibility claims honestly.

## Implemented qualification layers

1. `api/public-api.toml` and `luna-qualification` define the public crate inventory and stability
   tiers.
2. `luna_core::ErrorCode` and `CodedError` give public boundary failures machine-readable identity
   without making human diagnostics rigid.
3. `luna-ui-rust-qualification` runs deterministic editor, text-cache, pane, workspace, CPU, GPU,
   and accessibility fixtures and evaluates checked-in structural budgets.
4. `luna-render-wgpu` retains vertex/index buffers geometrically within explicit caps, reuses
   unchanged atlas content, exposes high-water diagnostics, and trims resources on memory pressure.
5. `ResourceLocator` documents and implements development, macOS bundle, executable-relative, XDG-
   style, `/usr/local/share`, and `/usr/share` discovery without a global resource singleton; the
   checked-in example is compiled and executed by the M7 gate.
6. `scripts/package-linux.sh` creates an unpacked development bundle and tarball with Desktop Entry,
   AppStream metadata, license, README, and the editor command manual.
7. The editor operator and acceptance command set is permanently maintained in
   `EDITOR_DEMO_COMMANDS.md`.
8. The public-API audit uses only Python 3.9+ standard-library features and does not require
   Python 3.11 `tomllib` or a separately installed `tomli` package.
9. Menu accessibility identifiers encode arbitrary command and submenu IDs into separator-safe node
   segments, preserving stable semantic trees for dotted command namespaces.

## Deterministic budgets

Shared CI timing is noisy, so blocking performance gates use counts and capacities: replay
operations, cache misses, pane/splitter counts, menu rows, workspace nodes, display commands, GPU
batches and atlas bytes, retained buffer caps, and accessibility nodes. Wall-clock timings remain
valuable diagnostics but are not accepted as deterministic pass/fail evidence.

## Release boundary

Linux is the blocking primary platform. macOS remains a supported secondary target with advisory CI
until repeated Apple-Silicon CPU/Metal, IME, VoiceOver, dialogs, FSEvents, lifecycle, and packaged
launch acceptance is recorded. Windows remains unofficial, best-effort, and non-blocking.
