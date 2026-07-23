# M2 Validation Report

## Verified baseline

The project owner compiled, formatted, linted, tested, ran, committed, and pushed M0 and the
corrected M1 native-host milestone on Pop!_OS with Rust 1.97.1. The M1 correction uses AccessKit's
`ActionRequest::target_node` field.

## M2 generation checks

The M2 overlay was checked for the following properties:

- all TOML files parse with Python's standard `tomllib` parser;
- every declared workspace member contains a manifest;
- every local path dependency resolves to a workspace package;
- the local workspace dependency graph is acyclic;
- external text/native dependencies are pinned to exact versions;
- all Rust files include an MPL-2.0 SPDX identifier;
- all crate roots include crate-level rustdoc;
- a source-aware delimiter scan reports balanced braces, brackets, and parentheses;
- project source contains no `unsafe` blocks or declarations;
- project source contains no `.unwrap()`, `.expect()`, `panic!`, `todo!`, or `unimplemented!` calls;
- generated source contains no container or developer-home path leakage;
- UTF-8 location conversion, scalar snapping, grapheme motion/deletion, selection replacement,
  vertical preferred-column motion, and scroll clamping have deterministic test fixtures;
- shaping tests cover caret geometry, hit testing, multiline selection, exact one-line visible
  ranges, and real horizontal extent for unwrapped long lines;
- renderer tests cover clipping, source-over alpha, transparent intermediate glyph images, and
  disjoint image clips;
- ordinary keyboard text is retained separately from logical keys, while IME commits remain a
  separate platform-neutral event;
- native adapter signatures were reviewed against the published winit 0.30.13, cosmic-text 0.19.0,
  unicode-segmentation 1.13.3, AccessKit 0.24.1, and accesskit_winit 0.33.2 APIs;
- the final repository-root archive contains no `.git`, `target`, Cargo registry, generated
  framebuffer, or outer wrapper directory;
- the M1-to-M2 patch applies to a fresh M1 baseline and reproduces the packaged M2 tree.

## Toolchain limitation for this overlay

The M2 generation container did not contain `rustc`, Cargo, rustfmt, or Clippy and could not resolve
the Cargo registry. Therefore this report does **not** claim a completed compiler run for new M2
code. After extracting the overlay into the committed corrected-M1 repository, run:

```bash
cargo fmt --all
./scripts/validate.sh
cargo run -p luna-ui-rust-text-demo
```

The first Cargo command may update the existing `Cargo.lock` with the new pinned text dependencies.
Any compiler, formatting, Clippy, test, rustdoc, rendering, input, shaping, scrolling, or
accessibility failure is an M2 blocker and should be corrected before M3 work begins.
