# M3.2d Validation Report

## Baseline

M3.2d is based on the owner-validated and committed M3.2c source tree.

## Change set

- new `crates/luna-workspaces` product-neutral workspace model and scan adapters;
- `DocumentDialogService` Open Folder contract and native/scripted implementations;
- editor Open Folder, Refresh Workspace, Close Workspace, real tree rows, and file activation;
- dynamic workspace menu, command-palette, pointer, keyboard, and accessibility projection;
- one-second UI-thread workspace refresh with unchanged-state suppression;
- updated architecture, status, roadmap, porting, parity, and prior-phase follow-up documentation;
- new `docs/M3_2D_WORKSPACE_TREE.md`;
- new `scripts/test-m3-2d.sh` automated and runtime routine.

## Implemented contracts

- exact stable workspace-node identities derived from normalized absolute paths;
- immutable recursive snapshots with path indexes and deterministic fingerprints;
- directories-first stable sorting;
- explicit hidden-entry, symlink, and depth policies;
- permission, depth-limit, and unreadable-node projection;
- expansion, selection, reveal, and refresh reconciliation;
- standard-library and deterministic in-memory workspace services;
- native and scripted Open Folder selection;
- duplicate-safe workspace file activation through existing UTF-8 document services;
- unchanged workspace refreshes that request no frame.

## Static validation performed here

- all TOML files parse;
- all 21 workspace members exist and have unique package names;
- every workspace path dependency resolves;
- every workspace package appears in `Cargo.lock`;
- changed Rust source delimiter and lexical scans pass;
- no unsafe block, unwrap, expect, panic, todo, or unimplemented call was introduced;
- source files retain SPDX identifiers;
- shell syntax validation passes for `scripts/test-m3-2d.sh`;
- documentation links target existing local files;
- source and documentation files contain no trailing whitespace;
- overlay and complete-source reconstructions are compared byte-for-byte;
- archives exclude Git metadata, target output, backups, and commit-message artifacts.

Resulting source inventory: 21 workspace members, 41 Rust files, 20,030 Rust lines, and 163 declared
tests across 95 tracked source, documentation, and configuration files.

## Local validation required

Run the focused pinned-toolchain gate:

```bash
./scripts/test-m3-2d.sh
```

The script runs formatting checks, workspace checking, strict Clippy, all tests, and rustdoc before
creating `/tmp/luna-m3-2d-workspace` runtime fixtures.

Then run the editor and verify Open Folder, sorted real tree rows, expansion and collapse, duplicate
file activation, hidden-file policy, non-followed symlinks, unavailable-node projection, automatic
refresh after create/rename/delete, manual unchanged refresh, Close Workspace fallback, dropdown
menus, the independent Ctrl+P palette, accessibility, and the proof-gallery regression. Any compiler,
strict-Clippy, test, documentation, dialog, workspace, accessibility, or runtime failure blocks
M3.2e.
