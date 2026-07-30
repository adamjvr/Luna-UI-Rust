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

- [x] `cargo-semver-checks` reviewed against the selected baseline when available
- [x] public API manifest and crate-contract inventory agree
- [x] every public error implements `CodedError`
- [x] downstream resource-loading example builds

## macOS secondary acceptance

- [ ] advisory GitHub-hosted CI passes (non-blocking; not required for `0.2.0-rc.1`)
- [x] Apple-Silicon CPU and Metal applications launch
- [x] `.app` bundle validates and launches
- [x] Retina/external-display scaling and resize/full-screen pass
- [x] Standard Additions dialogs and Application Support sessions pass
- [x] FSEvents, sleep/wake, and memory pressure pass
- [x] dead keys, emoji, and a multi-stage CJK IME pass
- [x] VoiceOver audit passes or exceptions are recorded

The accepted M8.4 real-hardware campaign remains the authoritative macOS evidence for this candidate.

Windows is not a release blocker and receives no official package or support commitment.

## M8 retained evidence

- [x] accepted M7 API snapshot retained
- [x] current API snapshot compared with the accepted baseline
- [x] qualification JSON retained with commit and environment metadata
- [x] Linux bundle checksum retained
- [x] repeated CPU/GPU long-session evidence recorded
- [x] external downstream consumer builds and packages
- [x] provisional API changes classified with migration notes
- [x] `0.2.0-rc.1` known limitations recorded

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

## M8.2 external downstream consumer acceptance

- [x] consumer Cargo metadata proves a separate workspace outside the root dependency graph
- [x] root workspace format, API-contract, check, strict Clippy, tests, rustdoc, and release demo builds pass
- [x] external consumer format, check, strict Clippy, tests, rustdoc, and release build pass
- [x] source-tree `--self-test` finishes with `m8_2_self_test=passed`
- [x] CPU consumer editing, controls, workspace reload, theme cycling, and accessibility pass
- [x] `wgpu` consumer repeats the CPU operator pass
- [x] session text, caret/selection/scroll, and workspace state survive close/relaunch
- [x] Linux downstream ZIP and SHA-256 checksum are produced under ignored `dist/m8.2`
- [x] extracted package discovers executable-relative resources without `LUNA_RESOURCE_ROOT`
- [x] extracted package launches and self-tests from an unrelated working directory

## M8.3 repeated long-session acceptance

- [x] `scripts/test-m8-3.sh` re-runs the complete accepted M8.2 gate
- [x] private M8.3 qualification binary passes strict workspace Clippy, tests, and rustdoc
- [x] document open/edit/save/close workload completes the configured cycle count
- [x] large-document workload demonstrates retained layout and raster reuse
- [x] pane/tab workload preserves ownership, partitions, snapshots, and collapse invariants
- [x] workspace mutation bursts coalesce and refresh without escaping the active root
- [x] session save/load/resave loops preserve complete validated state
- [x] CPU/GPU scene analysis stays below deterministic command, batch, atlas, and framebuffer limits
- [x] lifecycle workload covers resume, suspend, theme, resize/DPI, and memory-pressure transitions
- [x] source and extracted package self-tests pass repeatedly from isolated state directories
- [x] source and extracted `welcome.txt` resources remain identical across every cycle
- [x] two normalized M8.3 JSON reports are byte-for-byte deterministic
- [x] CPU editor and consumer complete the long-session operator protocol
- [x] `wgpu` editor and consumer repeat the long-session operator protocol
- [x] extracted consumer completes both backend passes without `LUNA_RESOURCE_ROOT`

## M8.3 automated long-session acceptance

- [x] all seven deterministic workloads pass twice at 64 cycles
- [x] four source-tree and four extracted-package self-tests pass
- [x] normalized reports are byte-for-byte deterministic
- [x] source and extracted reference consumer launch through CPU and Vulkan/`wgpu`
- [x] full editor dirty-close Discard passes through CPU after M8.3.1 repair
- [x] full editor dirty-close Discard passes through Vulkan/`wgpu` after M8.3.1 repair

## M8.3.1 native dirty-close repair

- [x] `scripts/test-m8-3-1.sh` passes
- [x] Zenity primary, secondary-on-stdout, secondary-on-stderr, diagnostic, and CRLF tests pass
- [x] two-choice confirmations do not create a duplicate Cancel extra button
- [x] dirty untitled tab closes after choosing Discard without opening Save As
- [x] dirty file-backed tab closes after choosing Discard without changing storage

## M8.4 Apple-Silicon evidence campaign

- [x] `scripts/test-m8-4-macos.sh` passes on Apple-Silicon arm64 hardware
- [x] CPU and Metal editor/proof-gallery source launches pass
- [x] CPU and Metal backend-pinned `.app` bundles validate, sign, archive, and launch
- [x] Retina geometry and repeated resize/full-screen pass
- [x] external-display geometry passes or is documented as not applicable
- [x] AppleScript Open, Save As, Open Folder, dirty-close, and conflict dialogs pass
- [x] Application Support session location and relaunch restoration pass
- [x] FSEvents workspace create/modify/rename/delete delivery passes
- [x] sleep/wake and memory-pressure recovery pass
- [x] dead keys, emoji, and one multi-stage CJK IME pass
- [x] VoiceOver focus/actions pass or an advisory exception is documented
- [x] native document-edited indicator follows dirty/saved state
- [x] dirty-close Discard closes without saving
- [x] at least three unique evidence records reference one clean commit
- [x] `campaign-summary.json` finishes with `m8_4_campaign=passed`

## M8.5 development release candidate

- [x] workspace and inherited package version set to `0.2.0-rc.1`
- [x] changelog names the `0.2.0-rc.1` candidate
- [x] known limitations and deferred post-M8 features are recorded
- [ ] retained M7-to-candidate public API comparison is revalidated
- [ ] complete blocking Linux `scripts/test-m8.sh` gate passes
- [ ] final CPU editor and proof-gallery smoke pass completes
- [ ] final Vulkan/`wgpu` editor and proof-gallery smoke pass completes
- [ ] Linux development bundle and SHA-256 checksum are produced
- [ ] source tag `v0.2.0-rc.1` points to the accepted candidate commit
