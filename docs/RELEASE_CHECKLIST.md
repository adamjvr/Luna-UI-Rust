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

- [ ] `scripts/test-m8-1.sh` passes through the safe child-shell runner
- [ ] `luna-clipboard` memory tests pass
- [ ] M7-to-M8.1 crate-contract difference is fully classified
- [ ] CPU editor Cut/Copy/Paste menu and Ctrl+X/C/V pass
- [ ] `wgpu` editor Cut/Copy/Paste menu and Ctrl+X/C/V pass
- [ ] copy from Luna into another desktop application passes
- [ ] paste from another desktop application into Luna passes
- [ ] multi-selection copy joins selections in document order
- [ ] multi-caret Paste inserts once per caret and is undoable
- [ ] extracted Linux package retains clipboard operation
