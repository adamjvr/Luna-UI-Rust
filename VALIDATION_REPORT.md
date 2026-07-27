# M7 Validation Report

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

## Compiler and runtime boundary

Rust 1.97.1 is unavailable in the artifact-building container. This report therefore does **not**
claim Cargo metadata, lockfile refresh, rustfmt, rustc, Clippy, tests, rustdoc, semver comparison,
qualification execution, Linux desktop/AppStream validation, CPU/GPU startup, accessibility, IME,
macOS hardware, or package launch were executed there.

Authoritative Linux/Pop!_OS validation:

```bash
./scripts/test-m7.sh
```

Advisory macOS validation:

```bash
./scripts/test-macos.sh
```

Any Linux formatting, API-contract, compilation, Clippy, test, rustdoc, deterministic-budget,
packaging, CPU/GPU, or operator-checklist regression blocks M7 acceptance. macOS remains advisory
until repeated real-hardware acceptance supports promotion.
