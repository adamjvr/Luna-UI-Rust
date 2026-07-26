# Current Status

**Milestone:** M6 macOS hardening and downstream integration — implemented; local validation pending

## Baseline

M6 is based on committed and locally validated M5 at
`b52685b7e528466b6257f1f48521746132764352`. The user confirmed the complete M5 Linux gate and editor
runtime before commit. That baseline supplies syntax spans, Sublime color-scheme import,
transactional undo/redo, multiple selections, IME composition, dynamic command state, editable
accessibility actions, and the advisory macOS lane.

## Implemented in M6

- explicit native Resumed, Suspended, MemoryWarning, close-request, and unsaved-state contracts;
- identical lifecycle delivery through CPU/softbuffer and GPU/wgpu hosts;
- macOS native document-edited indication driven by application-owned dirty state;
- session persistence on editor suspend, memory warning, and accepted native close;
- conventional macOS Application Support session paths;
- AppleScript Open, Save As, folder, prompt, conflict, and confirmation dialogs;
- `notify`-backed FSEvents workspace watching with polling fallback;
- product-neutral `PlatformWorkspaceWatchService` and macOS compatibility alias;
- product-neutral `luna-integration` downstream adapter composition crate;
- explicit platform support tiers in `luna-host-core`;
- development `.app` bundle creation, plist validation, ad-hoc signing, and verification;
- **Different**, a fifth built-in theme with translucent late-1990s/early-2000s desktop character;
- focused M6 Linux and macOS validation scripts and expanded graphical acceptance checklists;
- updated architecture, roadmap, porting, parity, support-policy, and validation documentation.

## Architectural boundary

Native hosts report lifecycle and dirty-state presentation but do not decide whether documents should
be saved or discarded. AppleScript is confined to the dialog adapter. FSEvents delivery is confined
to the workspace adapter. Platform state paths remain in the session adapter. The integration crate
only groups existing application-selected adapters and does not introduce product semantics or a
global service locator.

## Validation status

Static delimiter checks, TOML parsing, shell syntax, local dependency checks, Markdown-link checks,
patch whitespace checks, SPDX checks, and archive reconstruction are performed in the delivery
environment. That environment has no Rust 1.97.1 toolchain or graphical desktop and therefore cannot
claim rustfmt, rustc, Clippy, tests, rustdoc, native dialogs, FSEvents, VoiceOver, Metal presentation,
application signing, or graphical success.

Authoritative Linux/Pop!_OS gate:

```bash
cargo fmt --all
./scripts/test-m6.sh
```

Advisory macOS gate:

```bash
./scripts/test-macos.sh
```

See [`M6_MACOS_INTEGRATION.md`](M6_MACOS_INTEGRATION.md) and
[`MACOS_TESTING.md`](MACOS_TESTING.md) for runtime acceptance.

## Next milestone

M7 will concentrate on public API stabilization and release qualification: semver-facing contracts,
resource retention, replay/performance thresholds, documentation examples, Linux packaging, repeated
macOS hardware acceptance, and a decision on promoting macOS CI from advisory to blocking. Windows
remains unofficial and non-blocking.
