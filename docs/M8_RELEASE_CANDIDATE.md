# M8 Release-Candidate and Ecosystem Validation

M8 turns the accepted M7 framework into a reproducible development release candidate. It is a
qualification and downstream-consumption phase, not a broad feature-expansion phase.

## Goals

1. Retain and compare public API snapshots from the accepted M7 baseline.
2. Record reproducible Linux qualification, package, checksum, and operator evidence.
3. Exercise Luna from a separate downstream reference application using only public APIs.
4. Run repeated CPU/GPU, workspace-watcher, session, accessibility, and package workloads.
5. Record repeated Apple-Silicon hardware evidence while macOS remains advisory.
6. Resolve intentional provisional API churn before a versioned `0.2.0-rc.1` development release.

## M8.0 — M7 closeout and baseline freeze

- mark M7.0.1 accepted on the blocking Linux/Pop!_OS lane;
- preserve M7 as the compatibility baseline for M8;
- correct stale architecture and status documentation;
- establish retained evidence and API-snapshot directories;
- add an M8 gate that first re-runs the complete M7 qualification;
- add a changelog and explicit release-candidate policy.

## M8.1 — API compatibility cycle

M8.1a first closes the known clipboard command gap with a provisional product-neutral clipboard crate
and classifies that additive crate-contract change before symbol-level snapshot work continues.

- generate a deterministic public API snapshot for every public library crate;
- compare the current snapshot with the accepted M7 snapshot;
- classify every difference as compatible, intentionally breaking, or accidental;
- require migration notes and fixture updates for intentional provisional-API changes;
- keep stable-crate changes source compatible across ordinary pre-release updates.

The first focused review covers `luna-document-services`, `luna-panes`, `luna-session`, and
`luna-workspaces`. Platform and implementation adapters remain provisional unless downstream evidence
shows that their boundaries are sufficiently narrow and durable.

## M8.2 — downstream consumer proof

Create a small application outside the main workspace dependency graph that:

- consumes Luna through public package APIs;
- opens a native window through the CPU and `wgpu` hosts;
- renders reusable controls and editable text;
- opens a workspace and persists a session;
- resolves packaged resources through `ResourceLocator`;
- packages and launches without repository-relative assumptions.

The reference consumer remains product-neutral. It is not Moth Text and must not introduce Moth
commands, compatibility policy, settings policy, language-server policy, or product identity into
Luna crates.

M8.2 is accepted on the blocking Linux/Pop!_OS lane after the independent workspace gate, CPU and
`wgpu` operator passes, session relaunch, and extracted-package resource and launch checks completed.

## M8.3 — repeated long-session qualification

Retain evidence for repeated workloads covering:

- open/edit/save/close cycles;
- large-document scrolling and cache reuse;
- pane and tab create/move/close cycles;
- workspace mutations and watcher-event bursts;
- session save/restore loops;
- theme, DPI, resize, suspend/resume, and memory-pressure transitions;
- GPU resource reconstruction and retained-capacity trimming;
- extracted-package and executable-relative resource loading.

M8.3 implements a private qualification binary inside the existing qualification application package.
It adds no public Luna API and reports seven subsystem-specific workloads in stable JSON order. The
automated gate repeats source and extracted package self-tests, runs the structural harness twice, and
compares normalized reports after removing diagnostic-only timing fields. Real CPU and `wgpu` devices
remain subject to the documented operator protocol; headless scene analysis does not claim to replace
native host acceptance.

Blocking limits remain deterministic counts, capacities, and high-water marks. Wall-clock timings are
diagnostic only.

## M8.4 — macOS evidence campaign

Record repeated Apple-Silicon results for CPU and Metal/`wgpu` launch, `.app` packaging, Retina and
external-display geometry, dialogs, Application Support sessions, FSEvents, sleep/wake, memory
pressure, dead keys, emoji, multi-stage CJK IME, VoiceOver, native document-edited indication, and
dirty-close Discard.

The private `luna-ui-rust-m8-4-macos-evidence` executable records machine, toolchain, display, commit,
bundle, signing, and explicit operator-status facts. It does not add public Luna API. An accepted
campaign contains at least three unique evidence runs from one clean arm64 commit. External-display
coverage may be marked not applicable and VoiceOver may record an advisory exception only with notes.
Advisory CI becomes blocking only through a later explicit roadmap decision supported by repeated
real-hardware evidence.

M8.4 is accepted at clean Apple-Silicon implementation commit `3c93849df19fea32412b673f9f5a69a39ff7b145`. The complete
automated gate and three unique CPU/Metal operator evidence runs passed, and campaign verification
reported `m8_4_campaign=passed`. The retained evidence is stored under
`retained-evidence/m8.4/apple-silicon`. macOS remains advisory.

## M8.5 — development release candidate

The first candidate is expected to be `0.2.0-rc.1`. A candidate must include:

- source tag and changelog;
- retained M7-to-RC API comparison;
- deterministic qualification JSON;
- Linux development bundle and checksum;
- operator and long-session results;
- macOS advisory evidence;
- provisional API inventory and known limitations;
- migration notes for intentional API changes.

Windows remains unofficial, best-effort, and non-blocking.

## Non-goals

M8 does not prioritize regex search, deeper language services, docking, cross-window tab movement,
Moth integration, or official Windows support. Those changes may follow the release-candidate cycle
without moving the M8 compatibility target during qualification.
