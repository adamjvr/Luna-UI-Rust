# M7.0.1 Acceptance and M8 Baseline Report

## Baseline

M7 is developed from committed and locally validated M6 at
`89da6a786357d84a1be4e32f46749fc3666b9f1c`.

## Change set

- added a checked-in public-library stability inventory and metadata audit;
- added stable machine-readable error codes across public failure boundaries;
- added deterministic structural release budgets and an executable qualification report;
- added bounded retained GPU buffers, atlas-upload reuse, resource statistics, and memory-pressure
  trimming;
- added product-neutral packaged-resource resolution and a downstream example;
- added Linux development packaging with Desktop Entry, AppStream metadata, and operator docs;
- made the complete editor keyboard/mouse/runtime command set permanent documentation;
- added API policy, accessibility audit, release checklist, packaging, and M7 qualification docs;
- promoted CI and platform scripts to the M7 qualification gate.
- M7.0.1 removes the Python 3.11-only `tomllib` assumption, encodes dotted menu command IDs into
  valid accessibility-node segments, and uses Desktop Entry metadata accepted by the primary
  Pop!_OS validator.

## Implemented invariants

- every Cargo library package appears in both API contract inventories;
- stable error identity is independent of mutable human-readable diagnostic text;
- release budgets use deterministic counts/capacities rather than noisy wall-clock thresholds;
- GPU scene buffers grow geometrically but never exceed explicit policy limits;
- unchanged atlas content is not uploaded repeatedly;
- memory-pressure delivery trims rebuildable GPU resources without discarding application state;
- resource paths are normalized, relative, traversal-free, and searched in explicit priority order;
- Linux packages remain relocatable development bundles and do not claim a stable distro release;
- Linux is primary, macOS secondary/advisory, and Windows best-effort/non-blocking.

## Static validation performed in the delivery environment

- all 30 TOML files parse and all 29 workspace members are present;
- every workspace member and local path dependency exists;
- Python scripts compile;
- shell scripts pass `bash -n`;
- Rust lexical delimiter and duplicate-splice scans pass;
- changed Rust and shell files retain SPDX identifiers;
- local Markdown links resolve;
- `git diff --check` reports no malformed whitespace;
- the Linux packaging script completed a structural archive/checksum exercise with a placeholder executable;
- generated ZIP and patch payloads reconstruct the exact M7 delivery tree;
- SHA-256 manifests are generated after final archive creation.

## Acceptance boundary

M7.0.1 is accepted on the blocking Linux/Pop!_OS lane. The authoritative local gate completed
formatting, API-contract validation, all-target compilation, strict Clippy, complete tests, rustdoc,
deterministic qualification, resource-loading execution, Linux package construction, and the CPU/
Vulkan operator checks used for acceptance.

The retained M8 baseline is commit `e696df0cedaeda7ac5c0892cf8f709f8325eff8b`. M8 must re-run the
complete M7 gate before recording new evidence:

```bash
./scripts/test-m8.sh
```

Advisory macOS validation remains:

```bash
./scripts/test-macos.sh
```

Any Linux formatting, API-contract, compilation, Clippy, test, rustdoc, deterministic-budget,
packaging, CPU/GPU, or operator-checklist regression blocks an M8 candidate. macOS remains advisory
until repeated real-hardware acceptance supports promotion.

## M8.0 Known Follow-Up

- Edit > Copy and Edit > Paste remain disabled in both the CPU and `wgpu`
  editor-demo backends.
- The matching behavior across renderers indicates a shared command-enablement
  or platform-clipboard integration gap rather than a renderer defect.
- This is accepted for the M8.0 baseline commit but must be resolved before
  the `0.2.0-rc.1` release candidate.
