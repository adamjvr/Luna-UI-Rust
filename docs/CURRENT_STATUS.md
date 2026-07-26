# Current Status

**Milestone:** M4 GPU backend and rendering scalability — implemented; local Pop!_OS validation pending

## Baseline

M4 is based on committed and locally runnable M3.3c at
`b9aa4bbe8e65bc03e28ada1ec7a60726840bbb03`. That baseline supplies durable recursive pane sessions,
asynchronous completion delivery, deep popup routing, search history/options, scrollbar paging, and
native-first incremental workspace refresh.

## Implemented in M4

- new `luna-render-wgpu` display-list backend pinned to `wgpu` 29.0.3;
- new `luna-host-wgpu` native host driving the existing `NativeApplication` contract;
- surface resize/loss/suboptimal/outdated/timeout/occlusion handling;
- device-loss callback, event-loop wakeup, and complete GPU-resource reconstruction;
- solid/image quad compilation with ordered scissor batches;
- bounded per-frame BGRA image atlas and repeated-image deduplication;
- nested `PushClip`/`PopClip` commands implemented by both CPU and GPU renderers;
- retained static and dynamic display-list submission in painter order;
- GPU timing, scene, atlas, surface-recovery, and device-recovery diagnostics;
- CPU/GPU proof-gallery and editor selection through `LUNA_RENDER_BACKEND`;
- shared `ThemePreset` catalog with Luna Dark, Luna Light, Amber Monitor, and Green Terminal;
- four-palette theme cycling in the proof gallery;
- checked **View > Color Scheme** submenu in the editor demo;
- focused M4 tests, runtime checklist, architecture documentation, roadmap, parity, and porting updates.

## Architectural boundary

Widgets, text layout, command routing, editor state, and accessibility remain independent of `wgpu`.
`luna-render-wgpu` only compiles immutable paint data. `luna-host-wgpu` is a leaf adapter that owns
native GPU resources and translates lifecycle events. `luna-render` remains the reference CPU
implementation and supplies the shared logical-to-physical rectangle rule.

## Validation status

Static delimiter checks, TOML parsing, shell syntax, local-path checks, patch whitespace checks, and
package reconstruction are performed in the delivery environment. That environment has no Rust
1.97.1 toolchain and cannot download crates, so it does not claim rustfmt, rustc, Clippy, tests,
rustdoc, shader validation, GPU startup, or presentation success.

The authoritative Pop!_OS gate is:

```bash
cargo fmt --all
./scripts/test-m4.sh
```

The first connected run will update `Cargo.lock` for the new pinned GPU dependencies. Commit that
lockfile update with the phase. Then complete both backend runs described in
[`M4_GPU_RENDERING.md`](M4_GPU_RENDERING.md).

## Next milestone

M5 adds broader editor component parity: syntax spans, Sublime color-scheme adapters, richer command
routing/accessibility actions, undo/redo, multiple cursors, and complete IME pre-edit handling.
