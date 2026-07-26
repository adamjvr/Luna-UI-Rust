# M5 Validation Report

## Baseline

M5 is developed from committed and locally validated M4 at
`517480db511a209c3a365511763bc0db099f2619`.

## Change set

- added `luna-editor` with syntax snapshots, Sublime color-scheme import, transaction history,
  multiple selections, IME composition, and behavior-parity fixtures;
- added style-revision-aware rich shaping and syntax decorations;
- integrated undo/redo, secondary cursors, simultaneous editing, completion/find transactions, and
  visible IME pre-edit into the editor harness;
- added dynamic command state and explicit accessibility action/value delivery;
- added native IME translation and candidate-window positioning to CPU and GPU hosts;
- added `docs/M5_EDITOR_COMPONENT_PARITY.md`, `docs/MACOS_TESTING.md`, `scripts/test-m5.sh`, and
  `scripts/test-macos.sh`;
- added a non-blocking macOS 15 Apple-Silicon GitHub Actions job;
- updated README, architecture, current status, roadmap, porting map, Swift parity, Rust practices,
  and platform support policy.

## Implemented invariants

- syntax ranges are valid UTF-8 boundaries, sorted, and non-overlapping before shaping;
- syntax/language providers remain outside text shaping and widgets;
- simultaneous edits calculate pre-edit ranges and apply from right to left;
- undo/redo restores complete text and directional selection sets;
- caret-only movement does not change the saved checkpoint;
- pre-edit text does not mutate document history before IME commit;
- IME candidate geometry derives from the same shaped caret used for paint and hit testing;
- completion, find replacement, accessibility replacement, and normal typing share transactions;
- explicit semantic actions are translated by the AccessKit leaf adapter, not executed there;
- CPU and GPU hosts deliver identical input, IME, command, and accessibility application contracts;
- macOS is advisory until real-hardware acceptance exists;
- Windows is not a blocking target or release promise.

## Static validation performed in the delivery environment

- all TOML files parse;
- every workspace member and local path dependency exists;
- Rust lexical delimiter and duplicate-splice scans pass;
- shell scripts pass `bash -n`;
- `git diff --check` reports no malformed whitespace;
- changed Rust, shell, and workflow files retain required licensing/documentation conventions;
- local Markdown links resolve;
- generated archives are tested with `unzip -t`;
- the repo-root overlay is reconstructed over the exact M4 baseline and compared byte-for-byte;
- SHA-256 manifests are generated after final archive creation.

## Compiler and runtime boundary

Rust 1.97.1 is unavailable in the artifact-building container. This report therefore does **not**
claim Cargo resolution, rustfmt, rustc, strict Clippy, tests, rustdoc, native IME, AccessKit actions,
VoiceOver, GPU startup, or graphical presentation were executed there.

Authoritative Linux/Pop!_OS validation:

```bash
./scripts/test-m5.sh
```

Advisory macOS validation:

```bash
./scripts/test-macos.sh
```

Any formatting, compiler, Clippy, test, rustdoc, syntax, history, multi-selection, IME, accessibility,
CPU/GPU, or runtime regression blocks M5 acceptance on Linux. macOS failures remain advisory during
M5 but must be recorded for M6.
