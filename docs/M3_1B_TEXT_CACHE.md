# M3.1b Persistent Editor Text and Chrome

M3.1b removes document shaping and full-document glyph images from ordinary editor frames while
preserving Luna's single-source geometry invariant.

## Shared immutable document snapshots

`TextDocument` stores its UTF-8 bytes and indexed line table in shared immutable `Arc` storage.
Cloning a document for a widget or frame is therefore constant-time; edits replace the snapshot and
increment the editor revision.

## Retained document layout

Each open document owns one `TextLayoutCache`. Its logical key contains:

```text
document edit revision
viewport width
maximum raster width
font size
line height
tab width
```

Caret, selection, focus, overlay, and ordinary scroll state are intentionally absent. Those changes
must not reshape text.

A logical cache miss prepares a cosmic-text buffer and captures complete content size plus every
caret stop. A hit reuses those results.

## Overscanned raster bands

The cache retains one vertical image band containing the visible viewport plus one viewport of
overscan above and below. The immutable snapshot records both the partial image and its
content-local `raster_bounds`.

```text
full logical document coordinates
    ├── caret / selection / hit / accessibility geometry retained in full
    └── overscanned raster band
          └── image painted at content_origin + raster_bounds.origin
```

When scrolling remains inside the retained band, no glyph rasterization occurs. When the viewport
crosses the band boundary, the cache keeps logical geometry and produces only a new band.

Before drawing, the retained cosmic-text buffer is configured with the band height and corresponding
line/pixel scroll position. cosmic-text therefore exposes and rasterizes visible layout runs rather
than iterating every line in the document.

## Chrome labels

`TextLabelCache` is keyed by a stable application-provided slot ID. Each slot stores its current text,
maximum width, font metrics, foreground color, and immutable shaped snapshot.

This supports both static and dynamic chrome:

- menus, sidebar rows, and unchanged tabs become stable hits;
- dirty-tab title changes replace only that tab slot;
- line/column status changes replace one status slot instead of creating unbounded entries;
- theme changes clear all label slots because their foreground color changes.

Bounds and alignment are not shaping inputs, so moving unchanged chrome reuses its pixels.

## Invalidation mapping

```text
caret / selection / find match       -> TextOverlay
palette row / field focus             -> PaintOverlay
small or distant scroll               -> TextRaster
insert / delete / newline              -> TextLayout
sidebar / tab / document structure     -> WidgetLayout
AccessKit focus/action                  -> Accessibility
theme change                            -> FullFrame
```

The current CPU renderer still rebuilds a complete display list. These classes identify why work was
requested and let later phases skip unaffected layers without changing application APIs again.

## Runtime acceptance

Run the editor in release mode and watch `[luna-editor cache]` diagnostics:

```bash
cargo run --release -p luna-ui-rust-editor-demo
```

Required behavior:

1. Initial document frame records one logical miss and one raster miss.
2. Caret movement records logical and raster hits.
3. Selection dragging at an unchanged text location requests no frame.
4. Typing records a new logical miss and raster miss.
5. Small scrolling inside overscan records raster hits.
6. Distant scrolling records a raster miss without a logical miss.
7. Opening and navigating overlays does not change document miss counters.
8. Repeated line/column changes do not increase label entry count.
9. Idle editor produces no additional frames or cache reports.
