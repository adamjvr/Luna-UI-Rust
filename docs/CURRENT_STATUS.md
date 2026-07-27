# Current Status

**Milestone:** M7 public API stabilization and release qualification — implemented; local validation pending

## Baseline

M7 is based on committed and locally validated M6 at
`89da6a786357d84a1be4e32f46749fc3666b9f1c`. The user confirmed Linux CPU editor operation after the
M6.0.1 cache-field correction and committed the complete M6 phase. That baseline supplies explicit
native lifecycle and dirty-state contracts, macOS dialogs/session/FSEvents/package tooling,
application-owned downstream service composition, bounded platform support tiers, and the
**Different** theme.

## Implemented in M7

- checked-in `api/public-api.toml` inventory for every public library crate;
- stable, provisional, and internal API commitment tiers through `luna-qualification`;
- inherited repository/readme metadata and complete package descriptions for public crates;
- `luna_core::ErrorCode` and `CodedError` machine-readable public error contracts;
- coded errors across document, workspace, session, pane, render, host, text, editor, integration,
  command, accessibility, and qualification boundaries;
- deterministic structural budgets for editor replay, text cache behavior, pane/menu/workspace
  scale, CPU/GPU scene size, retained GPU capacities, and accessibility nodes;
- executable `luna-ui-rust-qualification` fixture producing deterministic JSON evidence;
- geometrically grown and explicitly capped retained GPU vertex/index buffers;
- atlas upload reuse, retained-resource statistics, and memory-pressure trimming;
- product-neutral packaged-resource discovery and a downstream resource-loading example;
- relocatable Linux development bundle and tarball with Desktop Entry and AppStream metadata;
- permanent `EDITOR_DEMO_COMMANDS.md` keyboard, pointer, runtime, and acceptance reference;
- public API, Linux packaging, accessibility audit, qualification, and release-checklist documents;
- M7 Linux blocking CI and expanded macOS advisory qualification.

## Architectural boundary

Compatibility tier and error-code policy live in small contracts rather than application logic.
Qualification uses deterministic structural evidence instead of unreliable shared-runner timing.
GPU retention remains entirely inside the GPU leaf backend. Resource discovery finds application-
owned files but does not choose product configuration. Linux packaging distributes the proof app and
operator documentation without declaring a stable end-user product release.

## Validation status

Static TOML, path, shell, Python, Markdown-link, SPDX, delimiter, whitespace, archive, overlay, and
patch reconstruction checks are performed in the delivery environment. That environment has no
Rust 1.97.1 toolchain or graphical desktop and therefore cannot claim rustfmt, rustc, strict Clippy,
tests, rustdoc, Cargo metadata, qualification execution, package validators, GPU presentation, IME,
accessibility, or macOS hardware success.

Authoritative Linux/Pop!_OS gate:

```bash
cargo fmt --all
./scripts/test-m7.sh
```

Advisory macOS gate:

```bash
./scripts/test-macos.sh
```

See [`M7_RELEASE_QUALIFICATION.md`](M7_RELEASE_QUALIFICATION.md),
[`EDITOR_DEMO_COMMANDS.md`](EDITOR_DEMO_COMMANDS.md), and
[`RELEASE_CHECKLIST.md`](RELEASE_CHECKLIST.md).

## Next milestone

M8 will consume the M7 evidence in a release-candidate cycle: retain API snapshots, qualify real
Linux packages over repeated long sessions, record repeated macOS hardware results, expand
product-neutral downstream examples, and resolve any API or resource-contract changes before a
versioned development release. Windows remains unofficial and non-blocking.
