# macOS Testing Track

## Support policy

Linux/Pop!_OS is the primary development, blocking CI, packaging, and release-acceptance platform.
macOS is the supported secondary native platform. M6 adds concrete native lifecycle, dirty-window,
dialog, session-path, FSEvents, Metal, and package seams, but macOS CI remains advisory until repeated
clean real-hardware reports justify promotion.

Windows is not an official Luna-UI-Rust target. Upstream dependency compilation and community fixes
may remain possible when they do not weaken Linux or macOS, but the project promises no Windows CI,
packages, releases, graphical acceptance, or support response.

## Automated advisory lane

The GitHub workflow runs `scripts/test-macos.sh` on an Apple-Silicon macOS 15 hosted runner. It performs:

- pinned-toolchain formatting verification;
- all-target/all-feature checking;
- focused host, dialog, session, workspace, integration, theme, and editor tests;
- strict Clippy, complete workspace tests, and warning-free rustdoc;
- release builds of the proof gallery and editor demo;
- creation, plist validation, ad-hoc signing, and verification of a development `.app` bundle.

Run it manually on a Mac with:

```bash
rustup toolchain install 1.97.1 --profile minimal --component clippy,rustfmt
rustup override set 1.97.1
./scripts/test-macos.sh
```

## Required real-hardware acceptance

Test on Apple Silicon with a Retina display and, where possible, an external display:

1. CPU/softbuffer and Metal/wgpu startup, resizing, full-screen transitions, sleep/wake, and shutdown.
2. Native document-edited indication while dirty and clearing after a saved checkpoint.
3. AppleScript Open File, Open Folder, Save As, dirty-close, save-conflict, rename, replace, and delete dialogs.
4. Session restoration below `~/Library/Application Support/luna-ui-rust/`.
5. FSEvents delivery for create, modify, atomic replace, rename, remove, directory bursts, and fallback polling.
6. Luna Dark, Luna Light, Amber Monitor, Green Terminal, and Different through CPU and GPU paths.
7. Menus, Command-key shortcuts, pointer capture, tab/splitter drag, clipboard integration where available, and close handling.
8. Dead keys, accent composition, emoji, and at least one multi-stage CJK IME with candidate placement.
9. VoiceOver traversal, focus, activation, editable replacement/value actions, announcements, and semantic bounds.
10. Launch the packaged `.app`; verify plist identity, high-DPI behavior, ad-hoc signature, CPU mode, and Metal mode.

Record machine model, macOS version, architecture, display scale, Rust version, selected wgpu adapter,
CPU/GPU backend, and the exact commit with every acceptance report.

## Packaging

Create the development bundle with:

```bash
./scripts/package-macos.sh
```

Use a custom signing identity or bundle metadata when a downstream application owns those values:

```bash
./scripts/package-macos.sh \
  --bundle-id org.example.Consumer \
  --display-name "Consumer" \
  --identity "Apple Development: Example"
```

Notarization, hardened runtime entitlements, icons, update channels, and distribution signing remain
release/downstream policy and are not claimed by the generic Luna proof bundle.

## Promotion gate

macOS CI becomes blocking only after multiple accepted real-hardware runs cover CPU and Metal,
Retina/external displays, lifecycle/memory pressure, dialogs, FSEvents, IME, VoiceOver, and packaged
launches. M6 builds the required seams; M7 records thresholds and release qualification.
