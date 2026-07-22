# Current Status

**Milestone:** M1 native host and reusable layout

## Implemented

- M0 deterministic foundation, compiler-verified on Pop!_OS with formatting, Clippy, build, unit
  tests, documentation tests, and the headless PPM demo passing.
- `luna-layout`: rows, columns, stacks, and two-pane splits with saturating integer allocation,
  deterministic remainder handling, immutable snapshots, and small-window regression fixtures.
- `luna-commands`: validated command IDs, metadata, key chords, repeat policy, deterministic
  registration, and immutable dispatch requests.
- `luna-accessibility-accesskit`: stable numeric identity mapping, DPI-aware bounds, role/action
  translation, and complete AccessKit tree updates.
- `luna-host-winit`: modern winit application lifecycle, native window/surface management,
  softbuffer presentation, resize and DPI handling, input normalization, AccessKit event routing,
  and explicit failure propagation.
- `WorkspaceDemo`: composite product-neutral editor-shell fixture whose paint, hit testing, and
  accessibility consume the same calculated geometry.
- `luna-ui-rust-native-demo`: live desktop proof with Control-P command dispatch, pointer activation,
  focus state, accessibility actions, and Escape-to-exit behavior.

## Not yet implemented

GPU rendering, shaped text, font fallback, glyph caching, editable text, retained widget trees,
advanced focus traversal, native dialogs, menu integration, theme-file parsing, editor widgets, or
Moth Text integration.

## Validation note

The M0 baseline was fully compiler-tested by the project owner. The M1 overlay was structurally and
API-reviewed during generation, but the generation container did not have Rust or outbound Cargo
registry access. Run `./scripts/validate.sh` immediately after applying the overlay; any compiler,
Clippy, test, or rustdoc finding is an M1 blocker.
