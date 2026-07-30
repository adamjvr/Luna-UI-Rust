# Changelog

All notable Luna-UI-Rust changes are recorded here. The project remains pre-1.0; compatibility
commitments are governed by `api/public-api.toml` and `docs/PUBLIC_API_POLICY.md`.

## 0.2.0-rc.1 — 2026-07-30

### Added

- M8.4 private Apple-Silicon environment, bundle, operator-status, and repeated-campaign evidence recorder;
- backend-pinned, ad-hoc signed CPU and Metal macOS application bundles with ZIP checksums;
- M8.4 Apple-Silicon automated gate and explicit three-run hardware acceptance protocol;

- M8.3 deterministic repeated long-session qualification with subsystem-specific structural reports;
- repeated source-tree and extracted-package self-test loops with normalized report comparison;
- M8.2 external downstream reference consumer with separate Cargo-workspace qualification;
- M8.2 source-tree and extracted-ZIP public-API self-tests;
- provisional product-neutral `luna-clipboard` text clipboard services;
- native and deterministic memory clipboard adapters;
- Cut, Copy, and Paste command integration for the editor demo;
- M8.1 crate-contract difference classification and gate;
- retained M7 public-API baseline metadata;
- release-evidence capture tooling;
- M8 release-candidate qualification gate;
- M8 release-candidate and ecosystem-validation specification.

### Changed

- M8.4 Apple-Silicon qualification is accepted after three unique CPU/Metal real-hardware evidence runs from one clean commit;

- the full M8.3 workload unit test is ignored in routine debug suites because the release-mode M8.3 gate is authoritative and debug cosmic-text shaping is intentionally expensive;

- M8.2 is accepted on the blocking Linux/Pop!_OS lane after CPU, `wgpu`, session-relaunch, and
  extracted-package operator passes;
- the provisional `SearchHistory::next()` API is renamed to `SearchHistory::next_newer()`; downstream callers must update the method name;
- the provisional `ParityError::Mismatch` payloads are boxed; downstream constructors must use `Box::new` and destructuring code must handle `Box<ParityResult>`;
- M7.0.1 is the accepted implementation baseline for M8;
- project status now distinguishes completed Linux acceptance from advisory macOS evidence;
- arbitrary-depth popup documentation matches the implemented M3.3c behavior.

### Fixed

- macOS packaging permits intentionally repeated required plist placeholders while still rejecting missing and unresolved template tokens;

- Zenity extra-button responses are recognized as exact lines on either standard output or standard error, so choosing **Discard** completes dirty-document close instead of being mistaken for Cancel;
- two-choice Zenity confirmations no longer add a duplicate extra button labeled Cancel.

## M7.0.1 — 2026-07-26

### Added

- explicit stable, provisional, and internal public-crate contracts;
- machine-readable `ErrorCode` and `CodedError` boundaries;
- deterministic release-qualification budgets and JSON reports;
- bounded retained GPU resources, atlas reuse, statistics, and memory-pressure trimming;
- product-neutral packaged-resource discovery and a downstream example;
- Linux development packaging with Desktop Entry, AppStream metadata, and operator docs;
- permanent editor operator and accessibility acceptance documentation.

### Fixed

- Python 3.9/3.10 public-API audit compatibility;
- separator-safe accessibility IDs for dotted menu command namespaces;
- Pop!_OS-compatible Desktop Entry metadata.

M0 through M6 established the deterministic core, native hosts, editor-grade text, real document and
workspace behavior, recursive panes and desktop interaction, the optional `wgpu` backend, broader
editor mechanics, and macOS/downstream integration hardening.
