# M3.3b Validation Report

## Baseline

M3.3b is based on the owner-validated and committed M3.3a.2 source tree.

## Change set

- advanced pane-tab metadata and operations in `luna-panes`;
- pinned, preview, reordered, moved, and overflowed tab projection in `EditorPaneSurface`;
- parent/child submenu definitions, geometry, keyboard routing, pointer routing, and mnemonics;
- tab context-menu integration;
- reusable caret-anchored `CompletionPopup`;
- match-case, whole-word, replace-current, and replace-all find behavior;
- interactive vertical scrollbar geometry and input mapping;
- editor integration for preview replacement/promotion, drag reorder, cross-pane movement, overflow,
  menus, context menus, completion, find options, replacement, scrollbar input, and accessibility;
- new `docs/M3_3B_ADVANCED_TABS_POPUPS.md`;
- new `scripts/test-m3-3b.sh` automated and runtime routine;
- updated architecture, status, roadmap, porting, parity, contribution, README, and prior-phase docs.

## Implemented contracts

- pane-local pinned tabs form a deterministic leading partition;
- pinned tabs cannot become transient previews;
- each pane has at most one replaceable clean preview;
- dirty previews promote before replacement;
- moving or reordering a tab preserves its `DocumentViewId` and document lifecycle identity;
- moving the final tab out of a pane safely collapses that pane;
- regular-tab overflow never hides the active tab;
- paint, hit testing, labels, drag targets, and accessibility use one immutable tab-strip layout;
- dropdown state separates top-level and child-submenu selection;
- menu commands, context commands, palette commands, keyboard, pointer, and accessibility share IDs;
- completion widgets return application-owned insertion payloads without language policy;
- find replacement applies stable byte ranges and preserves document/view synchronization;
- scrollbar paint and pointer mapping derive from the exact text viewport and maximum scroll range;
- only one editor transient surface is visible at a time.

## Static validation performed here

- all TOML files parse;
- every workspace member and local path dependency resolves;
- every workspace package is represented in `Cargo.lock`;
- Rust source lexical and delimiter scans pass;
- no unsafe block, unwrap, expect, panic, todo, or unimplemented call was introduced;
- source files retain SPDX identifiers;
- shell syntax validation passes for `scripts/test-m3-3b.sh`;
- documentation links target existing local files;
- source and documentation files contain no trailing whitespace;
- overlay and complete-source reconstructions are compared byte-for-byte;
- archives exclude Git metadata, target output, backups, temporary files, and commit-message artifacts.

Final source inventory before packaging:

- 23 workspace members;
- 44 Rust source files;
- 28,261 Rust source lines;
- 217 declared tests;
- 106 tracked source, documentation, script, and configuration files.

## Local validation required

Run the pinned-toolchain gate:

```bash
./scripts/test-m3-3b.sh
```

Then verify pinned and preview tabs, active-tab overflow visibility, local drag reorder, cross-pane
movement, submenu and mnemonic routing, tab context commands, completion acceptance, find options,
replace-current/all, scrollbar dragging, accessibility, existing file/workspace/session behavior,
and the proof-gallery regression. Any compiler, strict-Clippy, test, rustdoc, native runtime,
accessibility, or regression failure blocks M3.3c.
