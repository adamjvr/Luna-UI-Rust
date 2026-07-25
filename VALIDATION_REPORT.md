# M3.3a Validation Report

## Baseline

M3.3a is based on the owner-validated and committed M3.2e.2 source tree.

## Change set

- new `crates/luna-panes` recursive pane-model crate;
- shared document-view lookup and stable numeric view identity access;
- synchronized shared-text snapshots in `luna-text`;
- reusable `luna-ui::EditorPaneSurface`;
- optional legacy window-wide tab-strip projection in `EditorShell`;
- editor integration for recursive splits, pane-local tabs, independent view state, splitters,
  focus traversal, close/collapse behavior, and accessibility;
- new `docs/M3_3A_SPLIT_PANES.md`;
- new `scripts/test-m3-3a.sh` automated and runtime routine;
- updated architecture, status, roadmap, porting, parity, contribution, and prior-phase docs.

## Implemented contracts

- one lifecycle/save/storage identity per `DocumentId`;
- one independent presentation identity per `DocumentViewId`;
- synchronized shared text revisions without concurrent mutable aliasing;
- recursive binary pane topology with stable identities;
- deterministic leaf focus order and wrapping;
- pane-local tab membership and activation;
- safe parent collapse after removing an empty leaf;
- final-pane protection;
- clamped splitter ratios and minimum pane geometry;
- shared paint/hit/accessibility rectangles;
- pane-aware close that preserves documents represented in sibling views;
- rehoming for documents whose only view belonged to a closed pane.

## Static validation performed here

- all TOML files parse;
- every workspace member and local path dependency resolves;
- every workspace package is represented in `Cargo.lock`;
- Rust source lexical and delimiter scans pass;
- no unsafe block, unwrap, expect, panic, todo, or unimplemented call was introduced;
- source files retain SPDX identifiers;
- shell syntax validation passes for `scripts/test-m3-3a.sh`;
- documentation links target existing local files;
- source and documentation files contain no trailing whitespace;
- overlay and complete-source reconstructions are compared byte-for-byte;
- archives exclude Git metadata, target output, backups, and commit-message artifacts.

The completed tree contains 23 workspace members, 44 Rust source files, 24,889 Rust lines, 200 declared tests, and 104 source/documentation/configuration files.

## Local validation required

Run the pinned-toolchain gate:

```bash
./scripts/test-m3-3a.sh
```

The script runs formatting, workspace checking, strict Clippy, all tests, and rustdoc before creating
`/tmp/luna-m3-3a-workspace`.

Then verify horizontal and vertical recursive splits, shared edits, pane-local caret/selection/scroll,
splitter drag limits, focus traversal, pane-local close, close-pane rehoming, accessibility, existing
file/workspace/session behavior, and the proof-gallery regression. Any compiler, strict-Clippy, test,
rustdoc, native runtime, accessibility, or regression failure blocks M3.3b.
