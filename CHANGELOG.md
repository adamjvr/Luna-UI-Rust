# Changelog

All notable Luna-UI-Rust changes are recorded here. The project remains pre-1.0; compatibility
commitments are governed by `api/public-api.toml` and `docs/PUBLIC_API_POLICY.md`.

## Unreleased — M8

### Added

- provisional product-neutral `luna-clipboard` text clipboard services;
- native and deterministic memory clipboard adapters;
- Cut, Copy, and Paste command integration for the editor demo;
- M8.1 crate-contract difference classification and gate;
- retained M7 public-API baseline metadata;
- release-evidence capture tooling;
- M8 release-candidate qualification gate;
- M8 release-candidate and ecosystem-validation specification.

### Changed

- M7.0.1 is the accepted implementation baseline for M8;
- project status now distinguishes completed Linux acceptance from advisory macOS evidence;
- arbitrary-depth popup documentation matches the implemented M3.3c behavior.

## M7.0.1 — 2026-07-26

### Added

- explicit stable, provisional, and internal public-crate contracts;
- machine-readable `ErrorCode` and `CodedError` boundaries;
- deterministic release-qualification budgets and JSON reports;
- bounded retained GPU resources, atlas reuse, statistics, and memory-pressure trimming;
- packaged-resource discovery and Linux development packaging;
- permanent editor operator and accessibility acceptance documentation.

### Fixed

- Python 3.9/3.10 public-API audit compatibility;
- separator-safe accessibility IDs for dotted menu command namespaces;
- Pop!_OS-compatible Desktop Entry metadata.

M0 through M6 established the deterministic core, native hosts, editor-grade text, real document and
workspace behavior, recursive panes and desktop interaction, the optional `wgpu` backend, broader
editor mechanics, and macOS/downstream integration hardening.
