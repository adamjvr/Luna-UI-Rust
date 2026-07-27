# M7 Release Checklist

## Blocking Linux gate

- [ ] `cargo fmt --all -- --check`
- [ ] `python3 scripts/check-public-api.py`
- [ ] `cargo check --workspace --all-targets --all-features`
- [ ] strict workspace Clippy with warnings denied
- [ ] complete workspace tests
- [ ] rustdoc with warnings denied
- [ ] deterministic M7 qualification report passes
- [ ] Linux development bundle builds
- [ ] CPU editor and proof gallery pass the operator checklist
- [ ] Vulkan/`wgpu` editor and proof gallery pass the same checklist
- [ ] `git diff --check` is clean and `Cargo.lock` is committed

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
