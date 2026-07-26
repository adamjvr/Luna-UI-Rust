# M6 Validation Report

## Baseline

M6 is developed from committed and locally validated M5 at
`b52685b7e528466b6257f1f48521746132764352`.

## Change set

- added native lifecycle, close-request, and unsaved-state host contracts;
- mirrored dirty state into the macOS document-edited indicator;
- persisted editor sessions on suspend, memory warning, and native close;
- added macOS Application Support session paths and AppleScript dialogs;
- added `notify` 8.2.0 FSEvents delivery with polling fallback;
- added `luna-integration` for product-neutral downstream adapter composition;
- added stable platform-support tiers while retaining Windows as best-effort;
- added the **Different** late-1990s/early-2000s translucent desktop theme;
- added macOS application-bundle creation, plist validation, ad-hoc signing, and verification;
- added `docs/M6_MACOS_INTEGRATION.md`, `scripts/test-m6.sh`, and expanded macOS validation;
- updated README, architecture, current status, roadmap, porting map, Swift parity, and practices.

## Implemented invariants

- lifecycle events are delivered identically by CPU and GPU hosts;
- native close policy remains application-owned and can veto or accept termination;
- the macOS edited indicator reflects application state without owning document policy;
- reconstructible caches may be dropped on memory warning while durable state is persisted;
- AppleScript, Linux helper, scripted, and memory dialogs implement the same neutral trait;
- native watcher callbacks cross a channel and never mutate UI state;
- watcher failures fall back to polling and request a safe rescan;
- downstream adapters remain concrete application-owned values rather than globals;
- `Different` is selected by stable ID and participates in normal theme/session/command projection;
- Linux is primary, macOS secondary/advisory, and Windows best-effort/non-blocking.

## Static validation performed in the delivery environment

- all TOML files parse;
- every workspace member and local path dependency exists;
- Rust lexical delimiter and duplicate-splice scans pass;
- shell scripts pass `bash -n`;
- `git diff --check` reports no malformed whitespace;
- changed Rust and shell files retain SPDX identifiers;
- local Markdown links resolve;
- generated archives pass `unzip -t`;
- the repo-root overlay is reconstructed over the exact M5 full-source baseline and compared byte-for-byte;
- SHA-256 manifests are generated after final archive creation.

## Compiler and runtime boundary

Rust 1.97.1 is unavailable in the artifact-building container. This report therefore does **not**
claim Cargo resolution, lockfile refresh for `notify`, rustfmt, rustc, strict Clippy, tests, rustdoc,
AppleScript dialogs, FSEvents, VoiceOver, Metal startup, graphical presentation, plist validation, or
codesign were executed there.

Authoritative Linux/Pop!_OS validation:

```bash
./scripts/test-m6.sh
```

Advisory macOS validation:

```bash
./scripts/test-macos.sh
```

Any Linux formatting, compilation, Clippy, test, rustdoc, CPU/GPU, theme, lifecycle, dialog, watcher,
session, or integration regression blocks M6 acceptance. macOS failures remain advisory but must be
recorded for M7 release qualification.
