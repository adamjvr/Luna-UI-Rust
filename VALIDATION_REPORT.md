# M1 Validation Report

## Verified M0 baseline

The project owner compiled and validated M0 on Pop!_OS with Rust 1.97.1. Formatting, Clippy with
warnings denied, the complete workspace build, nine unit tests, documentation tests, and the
headless PPM demo all passed before M1 began.

## M1 generation checks

The M1 overlay was checked for the following properties:

- all TOML files parse with Python's standard `tomllib` parser;
- every workspace member contains a manifest;
- every local path dependency resolves to a workspace package;
- the local dependency graph is acyclic;
- external native dependencies are pinned to exact versions;
- all Rust files include an MPL-2.0 SPDX identifier;
- all crate roots include crate-level rustdoc;
- delimiters are balanced by a source-aware sanity scan;
- scans find no `unsafe` blocks or declarations in project source;
- scans find no `.unwrap()`, `.expect()`, `panic!`, `todo!`, or `unimplemented!` calls;
- scans find no generated container or developer-home path leakage;
- native adapter signatures were checked against the published winit 0.30.13, softbuffer 0.4.8,
  AccessKit 0.24.1, and accesskit_winit 0.33.2 APIs;
- layout includes regression fixtures for fixed/flexible ordering, clipped gaps, fractional
  remainders, cross-axis alignment, stacks, and split minimums;
- the final repo-root archive contains no `.git`, `target`, or generated framebuffer output.

## Toolchain limitation for this overlay

The M1 generation container did not contain `rustc`, Cargo, rustfmt, or Clippy, and it could not
resolve the Cargo registry. Therefore this report does **not** claim a completed compiler run for
new M1 crates. After extracting the overlay into the committed M0 repository, run:

```bash
cargo fmt --all
./scripts/validate.sh
cargo run -p luna-ui-rust-native-demo
```

Any compiler, formatting, Clippy, test, rustdoc, window-lifecycle, or accessibility failure is an M1
blocker and should be corrected before M2 text work begins.
