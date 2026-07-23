# Current Status

**Milestone:** M2 editor-grade text

## Implemented

- M0 deterministic foundation, compiler-verified on Pop!_OS.
- M1 native host/layout/command/accessibility milestone, compiler-verified by the project owner after
  correcting AccessKit 0.24's `ActionRequest::target_node` field usage.
- `luna-text`: immutable logical lines, UTF-8 offsets, explicit scalar-boundary snapping,
  location/absolute conversion, anchor/focus ranges, grapheme boundaries, compact editable state,
  selection replacement, grapheme-safe deletion, horizontal and vertical movement, revisions, and
  scroll clamping.
- `luna-text-cosmic`: one long-lived `FontSystem` and `SwashCache`, advanced shaping, installed-font
  fallback, bidi-aware cursor placement, rasterized glyph snapshots, grapheme caret stops, point hit
  testing, selection rectangles, exact visible ranges, and measured horizontal/vertical scroll geometry.
- `luna-render`: immutable `RasterImage` snapshots backed by shared pixel storage, clipped image
  commands, fractional-DPI image scaling, and correct straight-alpha source-over composition.
- `TextView`: shared editor geometry for background, gutter, current line, selection, glyph image,
  caret, text hit testing, scrolling, caret reveal, and accessibility.
- Accessibility text-area role, values, editability, total/caret/selection/visible UTF-8 ranges, and
  visible line children.
- `luna-ui-rust-text-demo`: editable multilingual text, complex scripts, fallback, bidi, ligatures,
  combining sequences, emoji graphemes, click/drag selection, keyboard-layout text, IME commits, keyboard editing,
  manual scrolling, resizing, and AccessKit focus.

## Deliberately deferred

A production rope or piece table, undo/redo, IME pre-edit composition, soft wrapping, syntax spans,
multiple cursors, rectangular selection, discontinuous bidi selection highlights, rich AccessKit
text-run selection mapping, font-family configuration, GPU glyph atlases, and Moth document/session
integration remain later milestones.

## Validation note

M0 and corrected M1 were compiler-tested by the project owner. The M2 generation container did not
contain Rust tooling or Cargo registry access, so its report covers structural validation and API
review rather than claiming a compiler run. Run `cargo fmt --all && ./scripts/validate.sh` after
applying the overlay. Any compiler, Clippy, test, rustdoc, rendering, input, shaping, or accessibility
failure is an M2 blocker.
