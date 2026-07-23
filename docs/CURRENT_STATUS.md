# Current Status

**Milestone:** M3 twin native demos and reusable editor anatomy

## Implemented

- M0 deterministic foundation and M1 native host/layout/commands, compiler-verified on Pop!_OS.
- M2 editor-grade document, shaping, raster, text-view, input, and accessibility foundation,
  compiler-verified and committed by the project owner.
- Shared `TextLabel`, `Button`, `Toggle`, `ProgressBar`, and card-border primitives.
- Responsive `ProofGallery` with control, layout, text, animation, accessibility, and theme cards.
- Shared `EditorShell` with Swift-derived menu/tab/sidebar/status metrics and one geometry snapshot for
  paint, hit testing, labels, and semantics.
- Reusable `CommandPalette` and `FindPanel` overlays.
- Additional platform-neutral semantic roles and AccessKit mappings for editor chrome and controls.
- Optional time-driven `NativeApplication::frame_interval` and `update` hooks. Static applications
  continue to sleep on events; animated applications use winit `WaitUntil` and request ordinary
  redraws.
- `luna-ui-rust-proof-gallery`, a dedicated native visual/interaction/accessibility regression
  application.
- `luna-ui-rust-editor-demo`, a dedicated event-driven editor integration harness with editable
  documents, dirty state, tab/sidebar navigation, command palette, find panel, saving, document
  creation/closing, theme switching, and editor text interaction.

## Deliberately deferred

GPU rendering, production menu popups, completion popups, splitter dragging, full replace commands,
undo/redo, syntax spans, multiple cursors, IME pre-edit composition, production file I/O, project
watching, Moth session policy, and packaging remain later milestones.

## Validation note

M0 through M2 were compiled and run by the project owner. The M3 generation environment did not
contain Rust tooling or Cargo registry access, so M3 receives structural, dependency, source-policy,
archive, and patch-round-trip validation here. Run `cargo fmt --all && ./scripts/validate.sh`, then
run both M3 applications after extraction. Any compiler, Clippy, test, rustdoc, rendering, input,
timing, shaping, or accessibility failure is an M3 blocker.
