# Current Status

**Milestone:** M8 release-candidate and ecosystem validation — active

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

M7.0.1 is accepted on the blocking Linux/Pop!_OS lane. The authoritative automated gate covers
formatting, API-contract validation, all-target compilation, strict Clippy, complete tests, rustdoc,
deterministic qualification, packaged-resource loading, and Linux bundle construction. CPU, Vulkan/
`wgpu`, proof-gallery, and extracted-package checks were completed during local acceptance.

macOS remains supported but advisory. Repeated Apple-Silicon CPU/Metal, IME, VoiceOver, dialog,
FSEvents, lifecycle, and packaged-launch evidence is still required before any promotion to a
blocking lane.

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

## M8.1a clipboard candidate

The next acceptance candidate adds the provisional `luna-clipboard` crate, native and memory adapters,
Cut/Copy/Paste menu and shortcut routing, deterministic editor tests, and explicit classification of
the additive M7-to-M8.1 crate-contract difference. It is pending the complete M8.1a automated gate and
CPU/`wgpu` cross-application clipboard operator pass.
