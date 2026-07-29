# M8.4 Apple-Silicon Evidence Campaign

M8.4 collects repeatable real-hardware evidence for Luna's advisory macOS tier. It does not promote
macOS to a blocking target and does not add public Luna API. A private executable records machine,
toolchain, source, bundle, signing, display, and operator facts; it never substitutes headless output
for graphical, accessibility, lifecycle, dialog, watcher, or input acceptance.

## Acceptance model

An accepted campaign contains at least three unique runs from one clean Git commit on Apple-Silicon
`arm64`. Each run validates separately packaged CPU and Metal/`wgpu` applications and supplies an
operator-status file covering:

- editor and proof-gallery behavior through CPU and Metal;
- source and packaged `.app` launches;
- Retina and optional external-display geometry;
- AppleScript dialogs and Application Support sessions;
- FSEvents workspace delivery;
- sleep/wake and memory-pressure recovery;
- dead keys, emoji, and multi-stage CJK IME;
- VoiceOver focus/actions;
- native document-edited indication and dirty-close Discard.

`external_display_geometry=not_applicable` is allowed when no second display is available. VoiceOver
may use `exception` only for a recorded advisory limitation. Both states require explanatory notes.

## Automated macOS preparation

From the repository root on Apple Silicon:

```bash
bash ./scripts/test-m8-4-macos.sh
```

This runs the M8.3.1 dialog repair gate, complete workspace check/Clippy/test/rustdoc gates,
deterministic release qualification, release builds, private evidence-recorder tests, and CPU/Metal
bundle construction with plist, arm64, signature, ZIP, and checksum validation.

Generated files remain below ignored `dist/m8.4`.

## Direct source launches

CPU:

```bash
LUNA_RENDER_BACKEND=cpu cargo run --release -p luna-ui-rust-editor-demo
cargo run --release -p luna-ui-rust-proof-gallery
```

Metal through `wgpu`:

```bash
WGPU_BACKEND=metal LUNA_RENDER_BACKEND=wgpu \
  cargo run --release -p luna-ui-rust-editor-demo

WGPU_BACKEND=metal LUNA_RENDER_BACKEND=wgpu \
  cargo run --release -p luna-ui-rust-proof-gallery
```

## Backend-pinned application bundles

```bash
bash ./scripts/package-macos.sh --backend cpu \
  --display-name "Luna UI Rust Editor Demo CPU" \
  --bundle-id org.lunaui.rust.editor-demo.cpu \
  --output dist/m8.4/packages

bash ./scripts/package-macos.sh --backend wgpu \
  --display-name "Luna UI Rust Editor Demo Metal" \
  --bundle-id org.lunaui.rust.editor-demo.metal \
  --output dist/m8.4/packages
```

Launch without setting backend environment variables:

```bash
open -na "$PWD/dist/m8.4/packages/Luna UI Rust Editor Demo CPU.app"
open -na "$PWD/dist/m8.4/packages/Luna UI Rust Editor Demo Metal.app"
```

The launcher inside each bundle pins its backend; the Metal bundle also pins `WGPU_BACKEND=metal`.

## Operator protocol

For each source and packaged backend:

1. Open, create, edit, save, Save As, close, and reopen documents.
2. Dirty a document, choose **Cancel**, then **Discard**; Discard must close without saving.
3. Confirm the macOS document-edited indicator appears when dirty and clears after save.
4. Exercise tabs, panes, menus, command palette, find/replace, completion, clipboard, undo/redo,
   multiple selections, scrolling, every theme, resize, full-screen, and minimize/restore.
5. Test Open, Save As, Open Folder, dirty-close, overwrite/reload/cancel, workspace replace, and
   workspace delete dialogs.
6. Relaunch and verify Application Support session restoration.
7. In an opened workspace, create, modify, rename, and delete files externally and verify FSEvents
   refresh without manual polling action.
8. Sleep and wake the Mac; then continue editing and render through the same backend.
9. Trigger available memory-pressure testing and confirm caches recover without state loss.
10. Enter a dead-key composition, emoji, and one multi-stage CJK IME sequence.
11. With VoiceOver, verify window, menus, tabs, tree, editable text, status, controls, focus, click,
    and editable-value actions.
12. On Retina and any available external display, verify pointer/text geometry and repeated scaling,
    resize, and full-screen transitions.

## Create an operator-status file

```bash
cargo run --release \
  -p luna-ui-rust-qualification \
  --bin luna-ui-rust-m8-4-macos-evidence -- \
  template --output dist/m8.4/operator-status-run-1.txt
```

Edit every `check.*=pending` entry to `pass`, or use only the documented `not_applicable`/`exception`
allowances with `notes=`.

## Capture one run

```bash
cargo run --release \
  -p luna-ui-rust-qualification \
  --bin luna-ui-rust-m8-4-macos-evidence -- \
  capture \
  --output-dir dist/m8.4/evidence \
  --run-id run-1 \
  --operator-status dist/m8.4/operator-status-run-1.txt \
  --cpu-bundle "$PWD/dist/m8.4/packages/Luna UI Rust Editor Demo CPU.app" \
  --metal-bundle "$PWD/dist/m8.4/packages/Luna UI Rust Editor Demo Metal.app"
```

Repeat as `run-2` and `run-3` on the same clean commit. `--allow-dirty` is available only for
provisional debugging evidence and is rejected by default campaign verification.

## Verify the campaign

```bash
cargo run --release \
  -p luna-ui-rust-qualification \
  --bin luna-ui-rust-m8-4-macos-evidence -- \
  verify \
  --evidence-dir dist/m8.4/evidence \
  --minimum-runs 3
```

A passing campaign ends with:

```text
m8_4_campaign=passed
```
