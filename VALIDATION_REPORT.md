# M3.2c Validation Report

## Baseline

M3.2c is based on the owner-validated and committed M3.2b.1 source tree.

## Change set

- `crates/luna-documents/src/lib.rs`
- `crates/luna-document-services/src/lib.rs`
- `apps/luna-ui-rust-editor-demo/src/main.rs`
- project status, architecture, roadmap, porting, parity, and phase documentation
- new `docs/M3_2C_RECENT_EXTERNAL_CHANGES.md`
- complete `scripts/test-m3-2c.sh` automated/runtime routine

## Implemented contracts

- opaque storage instances and revision-plus-instance snapshots;
- Modified, Replaced, Missing, and Recreated lifecycle states;
- standard-library and in-memory file observation;
- bounded canonical MRU recent-file state;
- recent-file menu and palette commands;
- UI-thread polling delivery through the native update cadence;
- status/accessibility external-state notices;
- explicit Reload from Disk and save-conflict continuity.

## Static validation performed here

- all TOML files parse;
- every workspace path dependency resolves;
- Rust source delimiter and lexical scans pass;
- no unsafe, unwrap, expect, panic, todo, or unimplemented calls were introduced;
- source files retain SPDX identifiers;
- documentation links target existing local files;
- overlay and complete-source reconstructions are compared byte-for-byte;
- archives exclude Git metadata, target output, backups, and commit-message artifacts.

Resulting source inventory: 20 workspace members, 40 Rust files, 17,883 Rust lines, and 137 declared
tests across 91 tracked source/documentation/configuration files.

## Local validation required

Run the complete pinned-toolchain gate:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Then run the editor and verify recent-file projection, unchanged polling, Modified, Replaced,
Missing, Recreated, Reload from Disk, save conflict, dropdown menus, and the independent Ctrl+P
palette. Any compiler, strict-Clippy, documentation, observation, accessibility, or runtime failure
blocks M3.2d.
