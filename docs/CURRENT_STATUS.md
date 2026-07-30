# Current Status

**Milestone:** M8.5 development release candidate — qualification

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
- public API, Linux packaging, accessibility audit, release checklist, packaging, and qualification documentation;
- M7 Linux blocking CI and expanded macOS advisory qualification.

## Architectural boundary

Compatibility tier and error-code policy live in small contracts rather than application logic.
Qualification uses deterministic structural evidence instead of unreliable shared-runner timing.
GPU retention remains entirely inside the GPU leaf backend. Resource discovery finds application-
owned files but does not choose product configuration. Linux packaging distributes proof applications
and operator documentation without declaring a stable end-user product release.

## Validation status

M7.0.1 is accepted on the blocking Linux/Pop!_OS lane. The authoritative automated gate covers
formatting, API-contract validation, all-target compilation, strict Clippy, complete tests, rustdoc,
deterministic qualification, packaged-resource loading, and Linux bundle construction. CPU, Vulkan/
`wgpu`, proof-gallery, and extracted-package checks were completed during local acceptance.

macOS remains supported but advisory. The M8.4 Apple-Silicon campaign is accepted at
`3c93849df19fea32412b673f9f5a69a39ff7b145` after three unique CPU/Metal real-hardware runs from one clean commit.
This acceptance does not promote macOS to a blocking lane.

M8 re-runs the complete M7 gate before retaining release evidence:

```bash
./scripts/test-m8.sh
```

See [`M8_RELEASE_CANDIDATE.md`](M8_RELEASE_CANDIDATE.md),
[`M7_RELEASE_QUALIFICATION.md`](M7_RELEASE_QUALIFICATION.md),
[`EDITOR_DEMO_COMMANDS.md`](EDITOR_DEMO_COMMANDS.md), and
[`RELEASE_CHECKLIST.md`](RELEASE_CHECKLIST.md).

## Active milestone

M8 retains the accepted M7.0.1 API and qualification baseline, exercises Luna from an external
consumer, records repeated Linux and macOS evidence, and resolves intentional provisional-API churn
before the planned `0.2.0-rc.1` development release. Windows remains unofficial and non-blocking.

## M8.1a clipboard acceptance

The provisional `luna-clipboard` crate, native and memory adapters, Cut/Copy/Paste menu and shortcut
routing, deterministic editor tests, and additive M7-to-M8.1 crate-contract classification are accepted
on the blocking Linux/Pop!_OS lane. CPU and `wgpu` cross-application transfer, multi-selection,
undo/redo, and extracted-package operation completed locally.

## M8.1b symbol-level API acceptance

M8.1b is accepted on the blocking Linux/Pop!_OS lane. Pinned symbol snapshots were retained for every
accepted M7 and current public crate, every difference was explicitly classified, snapshot checksums
passed, and `cargo-semver-checks` completed across every stable crate shared with the baseline. The
accepted comparison contains no unclassified or accidental change.

## M8.2 external downstream consumer acceptance

The separate `luna-reference-consumer` Cargo workspace is accepted on the blocking Linux/Pop!_OS
lane. Root and consumer format/check/strict-Clippy/test/rustdoc gates passed; every Luna demo built in
release mode; source-tree and extracted-package self-tests passed; CPU and `wgpu` operator checks
completed; edited text and view/workspace state survived session relaunch; and the extracted Linux ZIP
loaded resources without a repository-relative path.

## M8.3 repeated long-session qualification — accepted

The complete M8.3 automated gate and manual release qualification are accepted. All seven
deterministic workloads passed twice, source and extracted-package self-tests passed, normalized
reports were byte-for-byte identical, and extracted CPU and Vulkan/`wgpu` launches passed.
The native dirty-close defect discovered during qualification was repaired and accepted through
M8.3.1.

## M8.3.1 native dirty-close repair — accepted

The Zenity adapter recognizes exact extra-button labels before interpreting Cancel-like process
statuses. Focused parser and editor lifecycle regressions pass, and graphical dirty-close Cancel,
Discard, and Save behavior was accepted through CPU and `wgpu` hosts.

## M8.4 Apple-Silicon evidence campaign — accepted

M8.4 is accepted at clean implementation commit `3c93849df19fea32412b673f9f5a69a39ff7b145`. The Apple-Silicon automated
gate passed on an M2 Pro Mac mini, including workspace checks, strict Clippy, complete tests,
rustdoc, deterministic qualification, arm64 CPU and Metal application packaging, plist validation,
ad-hoc signing, ZIP construction, and checksum validation.

Three unique operator evidence records passed from the same clean commit. The campaign covers
source and packaged CPU/Metal applications, editor and proof-gallery operation, Retina and external
display geometry, native dialogs, Application Support sessions, FSEvents, sleep/wake, memory
pressure, dead keys, emoji, CJK IME, VoiceOver, native document-edited indication, and dirty-close
Discard.

Campaign verification ended with `m8_4_campaign=passed`. The retained campaign-summary SHA-256 is
`e4b56fd1dd730363c2a36e52580d99f1a601c2eb0a480fda81ba89c451dd54b2`. macOS remains advisory rather than blocking.

## M8.5 development release candidate — active

The workspace and inherited crate versions now target `0.2.0-rc.1`. Final qualification revalidates
the retained M7-to-M8 API comparison, runs the blocking Linux M8 gate, performs one CPU and one
`wgpu` graphical smoke pass, builds the Linux development bundle and checksum, and then creates the
source tag. Accepted M8.4 evidence remains advisory and is not repeated.
