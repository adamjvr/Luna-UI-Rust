# M3.2e Validation Report

## Baseline

M3.2e is based on the owner-validated and committed M3.2d source tree.

## Change set

- new `crates/luna-session` persistent session-state crate;
- workspace create-file, create-folder, rename, and recursive-delete service contracts;
- collision, replacement, deletion, and dirty-document dialog choices;
- open-document and recent-file relocation after workspace rename;
- dirty-file detachment to Untitled or protected close during deletion;
- persistent recent-file and workspace tree restoration;
- shared document-buffer/view identity seam;
- native watcher event and incremental refresh-scope boundaries;
- editor File-menu, command-palette, runtime, session, and mutation integration;
- updated architecture, status, roadmap, porting, parity, and prior-phase documentation;
- new `docs/M3_2E_WORKSPACE_OPERATIONS_SESSION.md`;
- new `scripts/test-m3-2e.sh` automated and runtime routine.

## Implemented contracts

- non-destructive mutation by default with typed collision errors;
- regular-file-only replacement after explicit confirmation;
- protected workspace root and non-following symlink mutation behavior;
- relocation of every open file identity beneath a renamed path;
- dirty-state preservation across relocation;
- pre-delete resolution of all affected dirty documents;
- atomic versioned session persistence in the user state directory;
- exact Unix path-byte encoding and deterministic memory-session tests;
- restoration of recent order, workspace root, expansion, and selection;
- multiple view IDs referring to one document-buffer ID;
- watcher events drained on the UI thread with full/subtree refresh scopes.

## Static validation performed here

- all TOML files parse;
- all 22 workspace members exist and have unique package names;
- every workspace path dependency resolves;
- every workspace package appears in `Cargo.lock`;
- Rust source lexical and delimiter scans pass;
- no unsafe block, unwrap, expect, panic, todo, or unimplemented call was introduced;
- source files retain SPDX identifiers;
- shell syntax validation passes for `scripts/test-m3-2e.sh`;
- documentation links target existing local files;
- source and documentation files contain no trailing whitespace;
- overlay and complete-source reconstructions are compared byte-for-byte;
- archives exclude Git metadata, target output, backups, and commit-message artifacts.

The final source inventory contains:

- 22 workspace members;
- 42 Rust source files;
- 22,539 Rust source lines;
- 187 declared tests;
- 99 tracked source, documentation, script, and configuration files.

## Local validation required

Run the focused pinned-toolchain gate:

```bash
./scripts/test-m3-2e.sh
```

The script runs formatting checks, workspace checking, strict Clippy, all tests, and rustdoc before
creating `/tmp/luna-m3-2e-workspace` runtime fixtures.

Then verify create file/folder, collision cancellation/replacement, file and directory rename,
recursive deletion, dirty Keep Open/Discard/Cancel outcomes, recent-file relocation, session
restoration after restarting the editor, dropdown-menu and Ctrl+P separation, accessibility, and the
proof-gallery regression. Any compiler, strict-Clippy, test, documentation, dialog, filesystem,
session, accessibility, or runtime failure blocks M3.3a.
