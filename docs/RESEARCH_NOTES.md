# Research Notes

The M2 implementation was checked against current primary Rust ecosystem documentation on
July 22, 2026.

## Rust workspace policy

- Rust 1.97.1 remains the pinned stable release.
- Edition 2024 virtual workspaces explicitly use Cargo resolver version 3.
- Package metadata and lint policy are inherited by every member crate.
- Clippy's `all` group is combined with selected policy lints; the full `restriction` group is not
  enabled indiscriminately.

## M2 text dependencies

- **unicode-segmentation 1.13.3:** supplies Unicode Standard Annex #29 extended grapheme boundaries.
  Durable document coordinates remain UTF-8 byte offsets; grapheme segmentation is used only for
  user-visible movement and deletion.
- **cosmic-text 0.19.0:** M2 uses `Buffer::new`, `Wrap::None`, unbounded shaping width with a bounded raster-allocation cap,
  explicit height and tab width, `Shaping::Advanced`, `shape_until_scroll`, `layout_runs`,
  `cursor_position`, and the safe `draw` callback.
- **winit 0.30.13 text input:** ordinary keyboard-layout text is preserved from `KeyEvent::text`,
  while completed IME composition arrives through `Ime::Commit`. IME pre-edit display remains a
  later host/widget contract.
- One long-lived `FontSystem` is retained because installed-font discovery is expensive. One
  `SwashCache` is retained so glyph rasterization is reused between frames.
- cosmic-text mutable state is isolated in `luna-text-cosmic`; Luna widgets receive immutable
  integer geometry and a backend-neutral BGRA8 image.
- cosmic-text 0.19.0 requires Rust 1.89, below Luna's pinned Rust 1.97.1 baseline.

## Accessibility

- AccessKit 0.24.1 uses UTF-8 units for text indices and includes the `MultilineTextInput` role.
- M2's platform-neutral tree carries complete, caret, selected, and visible UTF-8 ranges. The first
  adapter increment maps role, value, editability intent, bounds, children, and focus. Rich
  AccessKit `TextRun` character metrics and `TextSelection` action round-tripping are deliberately
  deferred until Luna exposes a complete per-run semantic model.

## Deliberate exclusions

M2 does not introduce an async runtime, GPU API, syntax parser, rope, piece table, undo engine, IME
pre-edit model, soft wrapping, or multiple-cursor policy. Those concerns should build on the stable
text-coordinate and shaped-snapshot contracts rather than becoming hidden requirements of them.
