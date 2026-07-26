# M3.3c Validation Report

## Baseline

M3.3c is reconstructed from committed M3.3b.6 at
`45150b72de3df8922c8cac9dfb8d88638d27c784`, including the M3.3b.1 through M3.3b.6 compiler,
Clippy, accessibility, node-ID, and submenu-test corrections.

## Change set

- validated recursive pane snapshots and keyboard tab movement in `luna-panes`;
- V2 document/view/pane session records with V1 read compatibility in `luna-session`;
- explicit dirty-state and file storage-baseline restoration;
- per-view caret, directional selection, and scroll persistence;
- recursive dropdown selection paths and pointer intent in `luna-ui`;
- reusable delayed cascading-menu state;
- asynchronous completion request, cancellation, response, and replacement-range contracts;
- bounded search history, wrap and selection-scoped find options;
- scrollbar track page classification;
- native-first Linux watcher delivery with polling fallback;
- coalesced full/subtree refresh planning and immutable snapshot reconciliation;
- editor-demo integration and expanded deterministic restart/corruption/service/runtime tests;
- new `docs/M3_3C_DESKTOP_HARDENING.md` and `scripts/test-m3-3c.sh`;
- updated architecture, roadmap, status, porting, parity, README, and validation documentation.

## Implemented invariants

- session decoding cannot construct duplicate documents/views/nodes or orphaned pane records;
- every persisted view is owned by exactly one pane tab and references one persisted document;
- shared documents restore once while pane-local views restore independently;
- pinned tabs form a leading partition and cannot be previews;
- dirty, untitled, and virtual tabs do not restore as replaceable previews;
- persisted file baselines retain modified/replaced/missing/recreated distinctions across restart;
- keyboard tab movement preserves view and document identities;
- dropdown selection, paint, pointer, keyboard, and accessibility share one recursive path/layout;
- completion responses activate only when request, view, and edit revision still match;
- every completion candidate owns an explicit valid UTF-8 replacement range;
- watcher events mutate workspace state only after UI-thread draining;
- watcher bursts are deterministic and excessive delivery degrades to a full rescan;
- subtree refresh retains unaffected stable IDs, expansion, and selection;
- polling and native watcher failure paths preserve a safe full-refresh fallback.

## Static validation performed in the delivery environment

- all TOML files parse;
- every workspace member and local path dependency exists;
- Rust lexical delimiter and splice scans pass;
- public-item documentation heuristic passes for all changed reusable crates;
- no unsafe block, unwrap, expect, panic, todo, or unimplemented call was introduced;
- changed Rust, script, and documentation files retain SPDX/license expectations where applicable;
- all shell scripts pass `bash -n`;
- no trailing whitespace or malformed patch whitespace is present;
- archive manifests and SHA-256 checks are generated after packaging;
- overlay reconstruction is compared byte-for-byte with the changed working tree.

## Compiler validation boundary

Rust 1.97.1 is not installed in the artifact-building container and outbound package retrieval is
unavailable there. Therefore this report does **not** claim that rustfmt, rustc, strict Clippy, the
Rust test runner, rustdoc, or the native application runtime were executed in that container.

The authoritative Pop!_OS gate is:

```bash
cargo fmt --all
./scripts/test-m3-3c.sh
```

Any formatting, compiler, Clippy, test, rustdoc, native watcher, session restart, accessibility,
proof-gallery, or performance regression blocks acceptance and M4 work.
## M3.3c.1 compiler correction

The first external pinned-toolchain run identified two calls to a nonexistent `EditableText::text`
method and one import-scope failure in the editor-demo test target. M3.3c.1 reads text through
`EditableText::document().text()` and imports `DropdownMenuState` explicitly.
