# M8 Release-Candidate Checklist

## Accepted M7.0.1 Linux baseline

- [x] `cargo fmt --all -- --check`
- [x] `python3 scripts/check-public-api.py`
- [x] `cargo check --workspace --all-targets --all-features`
- [x] strict workspace Clippy with warnings denied
- [x] complete workspace tests
- [x] rustdoc with warnings denied
- [x] deterministic M7 qualification report passes
- [x] Linux development bundle builds
- [x] CPU editor and proof gallery pass the operator checklist
- [x] Vulkan/`wgpu` editor and proof gallery pass the same checklist
- [x] `git diff --check` is clean and `Cargo.lock` is committed

## Advisory compatibility evidence

- [ ] `cargo-semver-checks` reviewed against the selected baseline when available
- [ ] public API manifest and crate-contract inventory agree
- [ ] every public error implements `CodedError`
- [ ] downstream resource-loading example builds

## macOS secondary acceptance

- [ ] advisory CI passes
- [ ] Apple-Silicon CPU and Metal applications launch
- [ ] `.app` bundle validates and launches
- [ ] Retina/external-display scaling and resize/full-screen pass
- [ ] Standard Additions dialogs and Application Support sessions pass
- [ ] FSEvents, sleep/wake, and memory pressure pass
- [ ] dead keys, emoji, and a multi-stage CJK IME pass
- [ ] VoiceOver audit passes or exceptions are recorded

Windows is not a release blocker and receives no official package or support commitment.

## M8 retained evidence

- [ ] accepted M7 API snapshot retained
- [ ] current API snapshot compared with the accepted baseline
- [ ] qualification JSON retained with commit and environment metadata
- [ ] Linux bundle checksum retained
- [ ] repeated CPU/GPU long-session evidence recorded
- [ ] external downstream consumer builds and packages
- [ ] provisional API changes classified with migration notes
- [ ] `0.2.0-rc.1` known limitations recorded

## M8.1a clipboard acceptance

- [x] `scripts/test-m8-1.sh` passes through the safe child-shell runner
- [x] `luna-clipboard` memory tests pass
- [x] M7-to-M8.1 crate-contract difference is fully classified
- [x] CPU editor Cut/Copy/Paste menu and Ctrl+X/C/V pass
- [x] `wgpu` editor Cut/Copy/Paste menu and Ctrl+X/C/V pass
- [x] copy from Luna into another desktop application passes
- [x] paste from another desktop application into Luna passes
- [x] multi-selection copy joins selections in document order
- [x] multi-caret Paste inserts once per caret and is undoable
- [x] extracted Linux package retains clipboard operation

## M8.1b symbol-level API acceptance

- [x] pinned `cargo-public-api 0.52.0`, `cargo-semver-checks 0.49.0`, and `nightly-2026-07-10` are recorded
- [x] accepted M7 and exact-current-commit public API snapshots are retained
- [x] both snapshot manifests and SHA-256 checksum sets pass
- [x] every public-symbol difference is explicitly classified
- [x] compatible provisional `luna-clipboard` addition is reviewed
- [x] intentional provisional `luna-editor` boxed-payload migration is reviewed
- [x] intentional provisional `luna-ui` search-history method rename is reviewed
- [x] no stable public crate changed
- [x] `cargo-semver-checks` passed across all ten stable crates
- [x] symbol diff, per-crate diffs, semver report, logs, and acceptance record are retained
