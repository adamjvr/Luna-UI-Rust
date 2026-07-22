# M0 Validation Report

Validation performed during scaffold generation:

- all TOML files parsed successfully with Python's standard `tomllib` parser;
- every workspace member contains a manifest;
- every local path dependency resolves to a workspace package;
- the local dependency graph is acyclic;
- all Rust files include an MPL-2.0 SPDX identifier;
- all crate roots include crate-level rustdoc;
- a delimiter sanity scan found balanced parentheses, brackets, and braces;
- scans found no `unsafe`, `.unwrap()`, `.expect()`, `panic!`, `todo!`, or `unimplemented!` use;
- scans found no accidental `/mnt/data` or `/home/adamjvr` path leakage;
- the generated patch applies cleanly to an empty Git worktree.

## Toolchain limitation

The generation environment did not contain `rustc`, Cargo, rustfmt, or Clippy, and outbound binary
toolchain installation was unavailable. Therefore this package does **not** claim a completed Rust
compiler run. On the first Rust-equipped machine, run:

```bash
./scripts/validate.sh
```

Any compiler, rustfmt, Clippy, test, or rustdoc failure is an M0 blocker and should be fixed before
adding the native-host milestone.
