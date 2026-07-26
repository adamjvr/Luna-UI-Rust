# Current Status

**Milestone:** M5 broader editor component parity — implemented; local Pop!_OS validation pending

## Baseline

M5 is based on committed and locally validated M4 at
`517480db511a209c3a365511763bc0db099f2619`. The user confirmed CPU/GPU editor operation before the
M4 commit. That baseline supplies the optional `wgpu` path, CPU oracle, nested clips, atlas/batching,
surface/device recovery, and four built-in theme presets.

## Implemented in M5

- new `luna-editor` crate with product-neutral syntax, Sublime scheme, history, selection, IME, and
  deterministic parity-fixture contracts;
- validated UTF-8 syntax snapshots and a lightweight Rust-like demonstration provider;
- comment-tolerant Sublime `.sublime-color-scheme` JSON parsing with global and scoped styles;
- style-revision-aware rich text shaping and retained syntax foregrounds;
- backend-neutral syntax backgrounds and underlines in `TextView`;
- bounded coalescing undo/redo history and text-only saved checkpoints;
- normalized multiple directional selections and simultaneous right-to-left edits;
- grapheme-safe multi-cursor Backspace/Delete plus vertical cursor creation;
- IME enable/disable/pre-edit/commit delivery through `luna-input` and both native hosts;
- application-owned IME candidate-window geometry and visible pre-edit state;
- dynamic command enabled/checked contracts for Undo/Redo projection;
- explicit semantic actions and UTF-8 accessibility replacement/value payload routing;
- editor integration for typing, deletion, completion, find/replace, IME, accessibility, undo/redo,
  and secondary-cursor presentation;
- focused M5 validation script and runtime checklist;
- advisory macOS 15 Apple-Silicon CI lane plus real-hardware test protocol;
- explicit support policy: Linux primary, macOS intended/advisory, Windows best-effort and unofficial.

## Architectural boundary

`luna-editor` contains reusable mechanisms, not language semantics or product workflow. Applications
supply syntax providers and imported schemes. `luna-text-cosmic` receives validated visual spans but
knows nothing about syntax scopes. `luna-ui::TextView` paints immutable decorations and selections but
does not own edit history. Native hosts translate IME and accessibility requests without deciding
what an editor command means.

## Validation status

Static delimiter checks, TOML parsing, shell syntax, local-path checks, Markdown-link checks, patch
whitespace checks, SPDX checks, and archive reconstruction are performed in the delivery environment.
That environment has no Rust 1.97.1 toolchain and cannot download crates, so it does not claim
rustfmt, rustc, Clippy, tests, rustdoc, native IME, accessibility, or graphical success.

The authoritative Pop!_OS gate is:

```bash
cargo fmt --all
./scripts/test-m5.sh
```

The macOS advisory gate is:

```bash
./scripts/test-macos.sh
```

See [`M5_EDITOR_COMPONENT_PARITY.md`](M5_EDITOR_COMPONENT_PARITY.md) and
[`MACOS_TESTING.md`](MACOS_TESTING.md) for runtime acceptance.

## Next milestone

M6 hardens macOS hosting, Metal/wgpu presentation, IME, VoiceOver, dialogs, watchers, and packaging,
then adds product-neutral downstream adapter examples. Windows is not planned as an official support
or release platform.
