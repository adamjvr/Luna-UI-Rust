// SPDX-License-Identifier: MPL-2.0

//! cosmic-text adapter for Luna's platform-neutral UTF-8 document model.
//!
//! This crate is Luna's only layer that knows about cosmic-text. It owns one long-lived
//! [`cosmic_text::FontSystem`] and [`cosmic_text::SwashCache`], performs advanced shaping, retains
//! per-document logical layout, and converts visible overscanned bands into immutable Luna
//! snapshots. Caret, selection, scrolling, hit-test, and accessibility geometry remain complete
//! even when glyph pixels cover only the active viewport band. Widgets never retain a font-system
//! borrow and renderers never depend on cosmic-text types.

use cosmic_text::{
    Attrs, Buffer, Color, Cursor, Family, FontSystem, Metrics, Scroll, Shaping, SwashCache, Wrap,
};
use luna_core::{PointI, RectI, SizeI};
use luna_render::{Framebuffer, FramebufferError, RasterImage, RasterImageError};
use luna_text::{TextDocument, TextLocation, TextRange};
use luna_theme::Rgba8;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

const CONTENT_PADDING: u32 = 4;
const DEFAULT_MAXIMUM_RASTER_WIDTH: u32 = 16_384;
const DEFAULT_OVERSCAN_VIEWPORTS: u32 = 1;

/// Inputs controlling one immutable shaped-text snapshot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextLayoutRequest {
    /// Logical viewport width. The snapshot is never narrower than this value.
    pub width: u32,
    /// Safety cap for an unwrapped line's raster width.
    ///
    /// Long lines are shaped at their natural width so horizontal scrolling is real rather than
    /// nominal. This cap bounds the temporary CPU image allocation.
    pub maximum_raster_width: u32,
    /// Font size in logical pixels.
    pub font_size: f32,
    /// Logical line height.
    pub line_height: f32,
    /// Default text color.
    pub foreground: Rgba8,
    /// Tab width in spaces.
    pub tab_width: u16,
}

impl TextLayoutRequest {
    /// Creates a practical editor-text request.
    #[must_use]
    pub const fn new(width: u32, font_size: f32, line_height: f32, foreground: Rgba8) -> Self {
        Self {
            width,
            maximum_raster_width: DEFAULT_MAXIMUM_RASTER_WIDTH,
            font_size,
            line_height,
            foreground,
            tab_width: 4,
        }
    }

    /// Overrides the safety cap for unwrapped raster width.
    #[must_use]
    pub const fn with_maximum_raster_width(mut self, maximum_raster_width: u32) -> Self {
        self.maximum_raster_width = maximum_raster_width;
        self
    }
}

/// One logical insertion point mapped into shaped visual geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaretStop {
    /// Stable Luna UTF-8 location.
    pub location: TextLocation,
    /// Visual insertion point in content-local logical pixels.
    pub point: PointI,
    /// Height of the containing visual line.
    pub height: u32,
}

/// Immutable logical text snapshot plus a possibly partial glyph raster.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextLayoutSnapshot {
    image: RasterImage,
    raster_bounds: RectI,
    content_size: SizeI,
    line_height: u32,
    content_padding: u32,
    caret_stops: Arc<[CaretStop]>,
}

impl TextLayoutSnapshot {
    /// Returns the transparent raster image containing glyph pixels.
    #[must_use]
    pub const fn image(&self) -> &RasterImage {
        &self.image
    }

    /// Returns the content-local rectangle represented by the raster image.
    #[must_use]
    pub const fn raster_bounds(&self) -> RectI {
        self.raster_bounds
    }

    /// Returns whether the retained raster fully covers a vertical viewport.
    #[must_use]
    pub fn covers_vertical_viewport(&self, scroll_y: i32, viewport_height: u32) -> bool {
        let top = i64::from(scroll_y.max(0));
        let content_bottom = i64::from(self.content_size.height);
        let bottom = top
            .saturating_add(i64::from(viewport_height))
            .min(content_bottom);
        i64::from(self.raster_bounds.y) <= top && self.raster_bounds.bottom() >= bottom
    }

    /// Returns the complete logical content size.
    #[must_use]
    pub const fn content_size(&self) -> SizeI {
        self.content_size
    }

    /// Returns rounded logical line height.
    #[must_use]
    pub const fn line_height(&self) -> u32 {
        self.line_height
    }

    /// Returns padding included around the shaped content.
    #[must_use]
    pub const fn content_padding(&self) -> u32 {
        self.content_padding
    }

    /// Returns every grapheme insertion stop in document order.
    #[must_use]
    pub fn caret_stops(&self) -> &[CaretStop] {
        &self.caret_stops
    }

    /// Returns the visual caret rectangle for a logical location.
    #[must_use]
    pub fn caret_rect(&self, location: TextLocation) -> Option<RectI> {
        let stop = self.closest_stop_for_location(location)?;
        Some(RectI::new(stop.point.x, stop.point.y, 1, stop.height))
    }

    /// Maps a content-local point to the nearest shaped grapheme insertion stop.
    #[must_use]
    pub fn hit_test(&self, point: PointI) -> Option<TextLocation> {
        let content_y = point
            .y
            .saturating_sub(i32::try_from(self.content_padding).unwrap_or(i32::MAX))
            .max(0);
        let target_line = if self.line_height == 0 {
            0
        } else {
            usize::try_from(content_y).unwrap_or(0) / usize::try_from(self.line_height).unwrap_or(1)
        };
        self.caret_stops
            .iter()
            .filter(|stop| stop.location.line_index == target_line)
            .min_by_key(|stop| i64::from(stop.point.x).abs_diff(i64::from(point.x)))
            .or_else(|| {
                self.caret_stops.iter().min_by_key(|stop| {
                    i64::from(stop.point.y)
                        .abs_diff(i64::from(point.y))
                        .saturating_mul(1_000_000)
                        .saturating_add(i64::from(stop.point.x).abs_diff(i64::from(point.x)))
                })
            })
            .map(|stop| stop.location)
    }

    /// Produces one clipped visual rectangle per logical line touched by a selection.
    ///
    /// cosmic-text supplies bidirectional cursor positions, so line endpoints follow visual text
    /// direction. M2 intentionally keeps selection geometry at one span per logical line; a later
    /// rich-text phase can expose discontinuous visual spans for mixed-direction selections.
    #[must_use]
    pub fn selection_rects(&self, range: TextRange) -> Vec<RectI> {
        let normalized = range.normalized();
        if normalized.is_collapsed() {
            return Vec::new();
        }
        let Some(maximum_line_index) = self
            .caret_stops
            .iter()
            .map(|stop| stop.location.line_index)
            .max()
        else {
            return Vec::new();
        };
        let first_line = normalized.anchor.line_index.min(maximum_line_index);
        let last_line = normalized.focus.line_index.min(maximum_line_index);
        if first_line > last_line {
            return Vec::new();
        }
        let mut rectangles = Vec::new();
        for line_index in first_line..=last_line {
            let start_column = if line_index == normalized.anchor.line_index {
                normalized.anchor.utf8_column
            } else {
                0
            };
            let end_column = if line_index == normalized.focus.line_index {
                normalized.focus.utf8_column
            } else {
                self.caret_stops
                    .iter()
                    .filter(|stop| stop.location.line_index == line_index)
                    .map(|stop| stop.location.utf8_column)
                    .max()
                    .unwrap_or(0)
            };
            if start_column == end_column {
                continue;
            }
            let Some(start) =
                self.closest_stop_for_location(TextLocation::new(line_index, start_column))
            else {
                continue;
            };
            let Some(end) =
                self.closest_stop_for_location(TextLocation::new(line_index, end_column))
            else {
                continue;
            };
            let left = start.point.x.min(end.point.x);
            let right = start.point.x.max(end.point.x);
            rectangles.push(RectI::new(
                left,
                start.point.y.min(end.point.y),
                u32::try_from(i64::from(right).saturating_sub(i64::from(left)))
                    .unwrap_or(u32::MAX)
                    .max(1),
                start.height.max(end.height),
            ));
        }
        rectangles
    }

    /// Returns maximum legal scroll offsets for a viewport.
    #[must_use]
    pub fn maximum_scroll(&self, viewport: SizeI) -> PointI {
        PointI::new(
            i32::try_from(self.content_size.width.saturating_sub(viewport.width))
                .unwrap_or(i32::MAX),
            i32::try_from(self.content_size.height.saturating_sub(viewport.height))
                .unwrap_or(i32::MAX),
        )
    }

    /// Returns the document range intersecting a vertical viewport.
    #[must_use]
    pub fn visible_range(
        &self,
        document: &TextDocument,
        scroll_y: i32,
        viewport_height: u32,
    ) -> TextRange {
        let line_height = usize::try_from(self.line_height.max(1)).unwrap_or(1);
        let padding = i32::try_from(self.content_padding).unwrap_or(i32::MAX);
        let content_scroll_y = scroll_y.saturating_sub(padding).max(0);
        let first = usize::try_from(content_scroll_y).unwrap_or(0) / line_height;
        let visible_height = usize::try_from(viewport_height).unwrap_or(0);
        let bottom_inclusive = usize::try_from(content_scroll_y)
            .unwrap_or(0)
            .saturating_add(visible_height.saturating_sub(1));
        let last = bottom_inclusive / line_height;
        let first = first.min(document.line_count().saturating_sub(1));
        let last = last.min(document.line_count().saturating_sub(1));
        let end_column = document.line(last).map_or(0, |line| line.utf8_length);
        TextRange::new(
            TextLocation::new(first, 0),
            TextLocation::new(last, end_column),
        )
    }

    fn closest_stop_for_location(&self, location: TextLocation) -> Option<&CaretStop> {
        self.caret_stops
            .iter()
            .filter(|stop| stop.location.line_index == location.line_index)
            .min_by_key(|stop| stop.location.utf8_column.abs_diff(location.utf8_column))
    }
}

/// Cache statistics for retained editor text.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextLayoutCacheStats {
    /// Requests that reused existing logical shaping and caret geometry.
    pub layout_hits: u64,
    /// Requests that rebuilt logical shaping and caret geometry.
    pub layout_misses: u64,
    /// Requests satisfied by the current overscanned raster band.
    pub raster_hits: u64,
    /// Requests that generated a new raster band.
    pub raster_misses: u64,
}

impl TextLayoutCacheStats {
    /// Adds another cache's counters into this aggregate.
    pub fn accumulate(&mut self, other: Self) {
        self.layout_hits = self.layout_hits.saturating_add(other.layout_hits);
        self.layout_misses = self.layout_misses.saturating_add(other.layout_misses);
        self.raster_hits = self.raster_hits.saturating_add(other.raster_hits);
        self.raster_misses = self.raster_misses.saturating_add(other.raster_misses);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TextGeometryKey {
    document_revision: u64,
    width: u32,
    maximum_raster_width: u32,
    font_size_bits: u32,
    line_height_bits: u32,
    tab_width: u16,
}

impl TextGeometryKey {
    fn new(document_revision: u64, request: TextLayoutRequest) -> Self {
        Self {
            document_revision,
            width: request.width,
            maximum_raster_width: request.maximum_raster_width,
            font_size_bits: request.font_size.to_bits(),
            line_height_bits: request.line_height.to_bits(),
            tab_width: request.tab_width,
        }
    }
}

struct PreparedTextLayout {
    buffer: Buffer,
    content_size: SizeI,
    line_height: u32,
    caret_stops: Arc<[CaretStop]>,
}

/// Retained logical layout and overscanned glyph raster for one document view.
///
/// Applications normally keep one cache per open document. Document revision, width, font
/// metrics, and tab width invalidate logical shaping. Foreground changes invalidate only glyph
/// pixels. Caret, selection, focus, and overlay changes do not invalidate either layer.
pub struct TextLayoutCache {
    geometry_key: Option<TextGeometryKey>,
    raster_foreground: Option<Rgba8>,
    prepared: Option<PreparedTextLayout>,
    snapshot: Option<TextLayoutSnapshot>,
    overscan_viewports: u32,
    stats: TextLayoutCacheStats,
}

impl TextLayoutCache {
    /// Creates an empty cache with one viewport of vertical overscan above and below visible text.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            geometry_key: None,
            raster_foreground: None,
            prepared: None,
            snapshot: None,
            overscan_viewports: DEFAULT_OVERSCAN_VIEWPORTS,
            stats: TextLayoutCacheStats {
                layout_hits: 0,
                layout_misses: 0,
                raster_hits: 0,
                raster_misses: 0,
            },
        }
    }

    /// Sets the number of complete viewport heights retained above and below the visible region.
    #[must_use]
    pub const fn with_overscan_viewports(mut self, overscan_viewports: u32) -> Self {
        self.overscan_viewports = overscan_viewports;
        self
    }

    /// Returns the most recent immutable snapshot, when one has been generated.
    #[must_use]
    pub const fn snapshot(&self) -> Option<&TextLayoutSnapshot> {
        self.snapshot.as_ref()
    }

    /// Returns lifetime hit and miss counters.
    #[must_use]
    pub const fn stats(&self) -> TextLayoutCacheStats {
        self.stats
    }

    /// Discards only glyph pixels while retaining logical shaping and caret geometry.
    pub fn invalidate_raster(&mut self) {
        self.raster_foreground = None;
        self.snapshot = None;
    }

    /// Updates the cache and returns a snapshot covering the requested viewport.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        engine: &mut TextEngine,
        document: &TextDocument,
        document_revision: u64,
        request: TextLayoutRequest,
        scroll_y: i32,
        viewport_height: u32,
    ) -> Result<&TextLayoutSnapshot, TextLayoutError> {
        validate_request(request)?;
        let geometry_key = TextGeometryKey::new(document_revision, request);
        if self.geometry_key == Some(geometry_key) {
            self.stats.layout_hits = self.stats.layout_hits.saturating_add(1);
        } else {
            self.prepared = Some(engine.prepare(document, request)?);
            self.geometry_key = Some(geometry_key);
            self.raster_foreground = None;
            self.snapshot = None;
            self.stats.layout_misses = self.stats.layout_misses.saturating_add(1);
        }

        let raster_is_current = self.raster_foreground == Some(request.foreground)
            && self.snapshot.as_ref().is_some_and(|snapshot| {
                snapshot.covers_vertical_viewport(scroll_y, viewport_height)
            });
        if raster_is_current {
            self.stats.raster_hits = self.stats.raster_hits.saturating_add(1);
        } else {
            let Some(prepared) = self.prepared.as_mut() else {
                return Err(TextLayoutError::InvalidCacheState);
            };
            let overscan = viewport_height.saturating_mul(self.overscan_viewports);
            let raster_bounds = raster_window(
                prepared.content_size,
                prepared.line_height,
                scroll_y,
                viewport_height,
                overscan,
            );
            self.snapshot = Some(engine.rasterize(prepared, request.foreground, raster_bounds)?);
            self.raster_foreground = Some(request.foreground);
            self.stats.raster_misses = self.stats.raster_misses.saturating_add(1);
        }

        self.snapshot
            .as_ref()
            .ok_or(TextLayoutError::InvalidCacheState)
    }
}

impl Default for TextLayoutCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Stateful shaping and glyph-raster cache shared across text views.
///
/// Construct this once per application or rendering thread. `FontSystem::new` scans installed
/// fonts, while `SwashCache` reuses rasterized glyph images across successive snapshots.
#[derive(Debug)]
pub struct TextEngine {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl TextEngine {
    /// Creates an engine backed by installed system fonts.
    #[must_use]
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }

    /// Shapes and rasterizes one complete immutable document snapshot.
    ///
    /// This compatibility path is useful for labels and tests. Editor views should retain a
    /// [`TextLayoutCache`] and call [`TextLayoutCache::update`] instead.
    pub fn shape(
        &mut self,
        document: &TextDocument,
        request: TextLayoutRequest,
    ) -> Result<TextLayoutSnapshot, TextLayoutError> {
        validate_request(request)?;
        let mut prepared = self.prepare(document, request)?;
        let bounds = RectI::new(
            0,
            0,
            prepared.content_size.width,
            prepared.content_size.height.max(1),
        );
        self.rasterize(&mut prepared, request.foreground, bounds)
    }

    fn prepare(
        &mut self,
        document: &TextDocument,
        request: TextLayoutRequest,
    ) -> Result<PreparedTextLayout, TextLayoutError> {
        let rounded_line_height = rounded_positive_u32(request.line_height);
        let content_height = u32::try_from(document.line_count())
            .unwrap_or(u32::MAX)
            .saturating_mul(rounded_line_height)
            .saturating_add(CONTENT_PADDING.saturating_mul(2));
        let metrics = Metrics::new(request.font_size, request.line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_wrap(Wrap::None);
        buffer.set_tab_width(request.tab_width);
        buffer.set_size(None, Some(content_height as f32));
        let attrs = Attrs::new().family(Family::Monospace);
        buffer.set_text(document.text(), &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let measured_text_width = buffer
            .layout_runs()
            .fold(0.0_f32, |maximum, run| maximum.max(run.line_w));
        let minimum_width = request.width.max(1);
        let maximum_width = request.maximum_raster_width.max(minimum_width);
        let measured_content_width = rounded_nonnegative_u32(measured_text_width)
            .saturating_add(CONTENT_PADDING.saturating_mul(2));
        let content_size = SizeI::new(
            measured_content_width.clamp(minimum_width, maximum_width),
            content_height.max(1),
        );
        let caret_stops = collect_caret_stops(
            document,
            &buffer,
            rounded_line_height,
            i32::try_from(CONTENT_PADDING).unwrap_or(0),
        );

        Ok(PreparedTextLayout {
            buffer,
            content_size,
            line_height: rounded_line_height,
            caret_stops: caret_stops.into(),
        })
    }

    fn rasterize(
        &mut self,
        prepared: &mut PreparedTextLayout,
        foreground: Rgba8,
        raster_bounds: RectI,
    ) -> Result<TextLayoutSnapshot, TextLayoutError> {
        let image_size = SizeI::new(raster_bounds.width.max(1), raster_bounds.height.max(1));
        let mut framebuffer = Framebuffer::new(image_size)?;
        let foreground = Color::rgba(
            foreground.red,
            foreground.green,
            foreground.blue,
            foreground.alpha,
        );
        let padding = i32::try_from(CONTENT_PADDING).unwrap_or(0);
        let line_height = i32::try_from(prepared.line_height.max(1)).unwrap_or(i32::MAX);
        let buffer_offset = raster_bounds.y.saturating_sub(padding).max(0);
        let scroll_line = usize::try_from(buffer_offset / line_height).unwrap_or(usize::MAX);
        let scroll_vertical = (buffer_offset % line_height) as f32;
        prepared
            .buffer
            .set_size(None, Some(raster_bounds.height.max(1) as f32));
        prepared
            .buffer
            .set_scroll(Scroll::new(scroll_line, scroll_vertical, 0.0));
        prepared.buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            foreground,
            |x, y, width, height, color| {
                let local_y = y
                    .saturating_add(buffer_offset)
                    .saturating_add(padding)
                    .saturating_sub(raster_bounds.y);
                let (red, green, blue, alpha) = color.as_rgba_tuple();
                framebuffer.blend_rect(
                    RectI::new(x.saturating_add(padding), local_y, width, height),
                    Rgba8::new(red, green, blue, alpha),
                );
            },
        );
        let image = framebuffer.into_raster_image()?;

        Ok(TextLayoutSnapshot {
            image,
            raster_bounds,
            content_size: prepared.content_size,
            line_height: prepared.line_height,
            content_padding: CONTENT_PADDING,
            caret_stops: prepared.caret_stops.clone(),
        })
    }
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn raster_window(
    content_size: SizeI,
    line_height: u32,
    scroll_y: i32,
    viewport_height: u32,
    overscan: u32,
) -> RectI {
    let content_height = content_size.height.max(1);
    let viewport_top = u32::try_from(scroll_y.max(0))
        .unwrap_or(u32::MAX)
        .min(content_height.saturating_sub(1));
    let raw_start = viewport_top.saturating_sub(overscan);
    let alignment = line_height.max(1);
    let start = raw_start / alignment * alignment;
    let raw_end = viewport_top
        .saturating_add(viewport_height.max(1))
        .saturating_add(overscan)
        .min(content_height);
    let aligned_line = raw_end.saturating_add(alignment.saturating_sub(1)) / alignment;
    let aligned_end = aligned_line.saturating_mul(alignment);
    let end = aligned_end.min(content_height).max(start.saturating_add(1));
    RectI::new(
        0,
        i32::try_from(start).unwrap_or(i32::MAX),
        content_size.width.max(1),
        end.saturating_sub(start).max(1),
    )
}

fn collect_caret_stops(
    document: &TextDocument,
    buffer: &Buffer,
    line_height: u32,
    padding: i32,
) -> Vec<CaretStop> {
    let mut stops = Vec::new();
    for line_index in 0..document.line_count() {
        for utf8_column in document.grapheme_boundaries(line_index) {
            let cursor = Cursor::new(line_index, utf8_column);
            let fallback_y = i32::try_from(line_index)
                .unwrap_or(i32::MAX)
                .saturating_mul(i32::try_from(line_height).unwrap_or(i32::MAX));
            let (x, y) = buffer
                .cursor_position(&cursor)
                .map_or((0.0, fallback_y as f32), |position| position);
            stops.push(CaretStop {
                location: TextLocation::new(line_index, utf8_column),
                point: PointI::new(
                    rounded_i32(x).saturating_add(padding),
                    rounded_i32(y).saturating_add(padding),
                ),
                height: line_height,
            });
        }
    }
    stops
}

fn validate_request(request: TextLayoutRequest) -> Result<(), TextLayoutError> {
    if !request.font_size.is_finite() || request.font_size <= 0.0 {
        return Err(TextLayoutError::InvalidFontSize);
    }
    if !request.line_height.is_finite() || request.line_height <= 0.0 {
        return Err(TextLayoutError::InvalidLineHeight);
    }
    Ok(())
}

fn rounded_nonnegative_u32(value: f32) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        0
    } else if value >= u32::MAX as f32 {
        u32::MAX
    } else {
        value.ceil() as u32
    }
}

fn rounded_positive_u32(value: f32) -> u32 {
    if !value.is_finite() || value <= 1.0 {
        1
    } else if value >= u32::MAX as f32 {
        u32::MAX
    } else {
        value.round() as u32
    }
}

fn rounded_i32(value: f32) -> i32 {
    if !value.is_finite() {
        0
    } else if value <= i32::MIN as f32 {
        i32::MIN
    } else if value >= i32::MAX as f32 {
        i32::MAX
    } else {
        value.round() as i32
    }
}

/// Failures produced while creating a shaped text snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextLayoutError {
    /// Font size was zero, negative, NaN, or infinite.
    InvalidFontSize,
    /// Line height was zero, negative, NaN, or infinite.
    InvalidLineHeight,
    /// An internal cache invariant was unexpectedly unavailable.
    InvalidCacheState,
    /// Transparent glyph framebuffer allocation failed.
    Framebuffer(FramebufferError),
    /// Conversion to an immutable raster image failed.
    RasterImage(RasterImageError),
}

impl Display for TextLayoutError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFontSize => {
                formatter.write_str("text font size must be finite and positive")
            }
            Self::InvalidLineHeight => {
                formatter.write_str("text line height must be finite and positive")
            }
            Self::InvalidCacheState => formatter.write_str("text layout cache state was invalid"),
            Self::Framebuffer(error) => write!(formatter, "text framebuffer failed: {error}"),
            Self::RasterImage(error) => write!(formatter, "text raster image failed: {error}"),
        }
    }
}

impl Error for TextLayoutError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Framebuffer(error) => Some(error),
            Self::RasterImage(error) => Some(error),
            Self::InvalidFontSize | Self::InvalidLineHeight | Self::InvalidCacheState => None,
        }
    }
}

impl From<FramebufferError> for TextLayoutError {
    fn from(value: FramebufferError) -> Self {
        Self::Framebuffer(value)
    }
}

impl From<RasterImageError> for TextLayoutError {
    fn from(value: RasterImageError) -> Self {
        Self::RasterImage(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{TextEngine, TextLayoutCache, TextLayoutError, TextLayoutRequest};
    use luna_core::{PointI, SizeI};
    use luna_text::{TextDocument, TextLocation, TextRange};
    use luna_theme::Rgba8;
    use std::error::Error;

    #[test]
    fn invalid_metrics_are_rejected_before_cosmic_text() {
        let mut engine = TextEngine::new();
        let request = TextLayoutRequest::new(100, 0.0, 18.0, Rgba8::opaque(255, 255, 255));
        assert_eq!(
            engine.shape(&TextDocument::new("test"), request),
            Err(TextLayoutError::InvalidFontSize)
        );
    }

    #[test]
    fn swift_text_fixture_shapes_caret_selection_and_hits() -> Result<(), Box<dyn Error>> {
        let document = TextDocument::new("alpha\nbeta\ngamma");
        let mut engine = TextEngine::new();
        let snapshot = engine.shape(
            &document,
            TextLayoutRequest::new(240, 14.0, 20.0, Rgba8::opaque(240, 240, 240)),
        )?;

        assert_eq!(snapshot.content_size().width, 240);
        assert!(snapshot.caret_rect(TextLocation::new(1, 2)).is_some());
        assert_eq!(
            snapshot
                .selection_rects(TextRange::new(
                    TextLocation::new(0, 2),
                    TextLocation::new(2, 3)
                ))
                .len(),
            3
        );
        assert!(snapshot.hit_test(PointI::new(4, 24)).is_some());
        assert!(snapshot.maximum_scroll(SizeI::new(240, 20)).y > 0);
        Ok(())
    }

    #[test]
    fn unwrapped_long_lines_have_real_horizontal_scroll_extent() -> Result<(), Box<dyn Error>> {
        let document = TextDocument::new("long line ".repeat(80));
        let mut engine = TextEngine::new();
        let snapshot = engine.shape(
            &document,
            TextLayoutRequest::new(160, 14.0, 20.0, Rgba8::opaque(240, 240, 240))
                .with_maximum_raster_width(4_096),
        )?;

        assert!(snapshot.content_size().width > 160);
        assert!(snapshot.maximum_scroll(SizeI::new(160, 40)).x > 0);
        Ok(())
    }

    #[test]
    fn exactly_one_line_viewport_exposes_only_one_line() -> Result<(), Box<dyn Error>> {
        let document = TextDocument::new("alpha\nbeta\ngamma");
        let mut engine = TextEngine::new();
        let snapshot = engine.shape(
            &document,
            TextLayoutRequest::new(240, 14.0, 20.0, Rgba8::opaque(240, 240, 240)),
        )?;

        assert_eq!(
            snapshot.visible_range(&document, 4, snapshot.line_height()),
            TextRange::new(TextLocation::new(0, 0), TextLocation::new(0, 5))
        );
        Ok(())
    }

    #[test]
    fn viewport_cache_reuses_layout_and_overscanned_raster() -> Result<(), Box<dyn Error>> {
        let document = TextDocument::new(
            (0..100)
                .map(|line| format!("line {line}\n"))
                .collect::<String>(),
        );
        let request = TextLayoutRequest::new(240, 14.0, 20.0, Rgba8::opaque(240, 240, 240));
        let mut engine = TextEngine::new();
        let mut cache = TextLayoutCache::new();

        let first = cache
            .update(&mut engine, &document, 0, request, 0, 100)?
            .clone();
        let second = cache.update(&mut engine, &document, 0, request, 40, 100)?;

        assert!(first.raster_bounds().height < first.content_size().height);
        assert_eq!(first.raster_bounds(), second.raster_bounds());
        assert_eq!(
            cache.stats(),
            super::TextLayoutCacheStats {
                layout_hits: 1,
                layout_misses: 1,
                raster_hits: 1,
                raster_misses: 1,
            }
        );
        Ok(())
    }

    #[test]
    fn distant_scroll_rerasterizes_without_reshaping() -> Result<(), Box<dyn Error>> {
        let document = TextDocument::new(
            (0..200)
                .map(|line| format!("line {line}\n"))
                .collect::<String>(),
        );
        let request = TextLayoutRequest::new(240, 14.0, 20.0, Rgba8::opaque(240, 240, 240));
        let mut engine = TextEngine::new();
        let mut cache = TextLayoutCache::new();

        let first = cache
            .update(&mut engine, &document, 0, request, 0, 100)?
            .raster_bounds();
        let second = cache
            .update(&mut engine, &document, 0, request, 2_000, 100)?
            .raster_bounds();

        assert_ne!(first, second);
        assert_eq!(cache.stats().layout_misses, 1);
        assert_eq!(cache.stats().raster_misses, 2);
        Ok(())
    }

    #[test]
    fn color_change_rerasterizes_but_revision_change_reshapes() -> Result<(), Box<dyn Error>> {
        let document = TextDocument::new("alpha\nbeta\ngamma");
        let request = TextLayoutRequest::new(240, 14.0, 20.0, Rgba8::opaque(240, 240, 240));
        let mut engine = TextEngine::new();
        let mut cache = TextLayoutCache::new();

        let _ = cache.update(&mut engine, &document, 0, request, 0, 60)?;
        let recolored = TextLayoutRequest {
            foreground: Rgba8::opaque(10, 20, 30),
            ..request
        };
        let _ = cache.update(&mut engine, &document, 0, recolored, 0, 60)?;
        let _ = cache.update(&mut engine, &document, 1, recolored, 0, 60)?;

        assert_eq!(cache.stats().layout_misses, 2);
        assert_eq!(cache.stats().layout_hits, 1);
        assert_eq!(cache.stats().raster_misses, 3);
        Ok(())
    }
}
