# macOS Testing Track

## Support policy

Linux/Pop!_OS remains the primary development and acceptance platform. macOS is the second intended
native platform and now has an explicit advisory build/test lane. It becomes a blocking release gate
only after repeated clean automated and graphical runs on real Apple hardware.

Windows is not an official Luna-UI-Rust target. Dependency-level compilation may continue to work and
community fixes may be accepted when they do not weaken Linux or macOS, but the project does not
promise Windows CI, packaging, release artifacts, graphical acceptance, or support response times.

## Automated advisory lane

The GitHub workflow runs `scripts/test-macos.sh` on an Apple-Silicon macOS 15 hosted runner as a
non-blocking advisory job. The script performs:

- pinned-toolchain formatting verification;
- all-target/all-feature `cargo check`;
- strict Clippy;
- complete workspace tests;
- rustdoc with warnings denied;
- release builds of the proof gallery and editor demo.

Run it manually on a Mac with:

```bash
rustup toolchain install 1.97.1 --profile minimal --component clippy,rustfmt
rustup override set 1.97.1
./scripts/test-macos.sh
```

## Required real-hardware acceptance

Hosted CI cannot prove native presentation or assistive-technology behavior. Before macOS is promoted
to a blocking release platform, test on an Apple-Silicon Mac with a Retina display:

1. CPU/softbuffer proof gallery and editor startup, resizing, full-screen transitions, and shutdown.
2. `wgpu` Metal presentation, surface recreation, sleep/wake, display-scale changes, and prolonged use.
3. Luna Dark, Luna Light, Amber Monitor, and Green Terminal under CPU and GPU presentation.
4. Native menus, keyboard equivalents, pointer capture, drag operations, clipboard, and file dialogs.
5. Strict UTF-8 Open/Save/Save As, workspace mutation/watch behavior, and session restoration.
6. Dead keys, accent composition, emoji, and at least one multi-stage CJK IME with candidate placement.
7. VoiceOver traversal, focus, activation, editable replacement/value actions, and semantic bounds.
8. CPU/GPU visual comparisons at 1× and Retina scale, including syntax backgrounds and underlines.

Record machine model, macOS version, architecture, display scale, Rust version, and selected wgpu
adapter with every acceptance report.

## Roadmap gates

- **M5:** advisory macOS compile/test job and documented real-hardware protocol.
- **M6:** close macOS-specific host, IME, accessibility, dialog, watcher, and packaging gaps; promote
  the lane to blocking only after acceptance evidence exists.
- **Windows:** remains best-effort and non-blocking with no planned official support milestone.
