// SPDX-License-Identifier: MPL-2.0

use crate::Widget;
use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{NodeId, PointI, RectI};
use luna_render::DisplayList;
use luna_text::TextDocument;
use luna_text_cosmic::{TextEngine, TextLayoutError, TextLayoutRequest, TextLayoutSnapshot};
use luna_theme::Rgba8;
use std::collections::HashMap;

/// Horizontal alignment for an immutable shaped label inside its bounds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextAlignment {
    /// Place text against the leading edge.
    #[default]
    Leading,
    /// Center text horizontally.
    Center,
    /// Place text against the trailing edge.
    Trailing,
}

/// Lifetime counters for the reusable shaped-label cache.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextLabelCacheStats {
    /// Requests served by a retained shaped label.
    pub hits: u64,
    /// Requests that shaped and rasterized a new label.
    pub misses: u64,
    /// Number of distinct retained label layouts.
    pub entries: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TextLabelCacheKey {
    text: String,
    maximum_width: u32,
    font_size_bits: u32,
    line_height_bits: u32,
    foreground: Rgba8,
}

struct CachedTextLabel {
    key: TextLabelCacheKey,
    layout: TextLayoutSnapshot,
}

/// Application-owned cache for immutable editor-chrome labels.
///
/// Each stable label slot retains at most one layout. Bounds and alignment remain widget
/// properties; changing a slot's text, typography, color, or maximum width replaces only that
/// slot. Dynamic status labels therefore remain bounded instead of accumulating one entry per
/// displayed value.
#[derive(Default)]
pub struct TextLabelCache {
    layouts: HashMap<String, CachedTextLabel>,
    hits: u64,
    misses: u64,
}

impl TextLabelCache {
    /// Creates an empty label cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a retained label layout or shapes it once on a cache miss.
    pub fn layout(
        &mut self,
        engine: &mut TextEngine,
        slot_id: &str,
        text: &str,
        maximum_width: u32,
        font_size: f32,
        line_height: f32,
        foreground: Rgba8,
    ) -> Result<TextLayoutSnapshot, TextLayoutError> {
        let key = TextLabelCacheKey {
            text: text.to_owned(),
            maximum_width: maximum_width.max(1),
            font_size_bits: font_size.to_bits(),
            line_height_bits: line_height.to_bits(),
            foreground,
        };
        if let Some(cached) = self.layouts.get(slot_id)
            && cached.key == key
        {
            self.hits = self.hits.saturating_add(1);
            return Ok(cached.layout.clone());
        }

        let layout = engine.shape(
            &TextDocument::new(text),
            TextLayoutRequest::new(1, font_size, line_height, foreground)
                .with_maximum_raster_width(key.maximum_width),
        )?;
        self.layouts.insert(
            slot_id.to_owned(),
            CachedTextLabel {
                key,
                layout: layout.clone(),
            },
        );
        self.misses = self.misses.saturating_add(1);
        Ok(layout)
    }

    /// Removes retained label layouts while preserving lifetime counters.
    pub fn clear(&mut self) {
        self.layouts.clear();
    }

    /// Returns lifetime hit, miss, and entry counters.
    #[must_use]
    pub fn stats(&self) -> TextLabelCacheStats {
        TextLabelCacheStats {
            hits: self.hits,
            misses: self.misses,
            entries: self.layouts.len(),
        }
    }
}

/// Reusable static label backed by a shaped cosmic-text snapshot.
///
/// Shaping remains application-owned because [`luna_text_cosmic::TextEngine`] carries mutable font
/// caches. The widget owns only the immutable result, so the same pixels and bounds can be reused
/// by proof-gallery cards, editor chrome, dialogs, and accessibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextLabel {
    id: NodeId,
    bounds: RectI,
    text: String,
    layout: TextLayoutSnapshot,
    alignment: TextAlignment,
}

impl TextLabel {
    /// Creates a static label.
    #[must_use]
    pub fn new(
        id: NodeId,
        bounds: RectI,
        text: impl Into<String>,
        layout: TextLayoutSnapshot,
        alignment: TextAlignment,
    ) -> Self {
        Self {
            id,
            bounds,
            text: text.into(),
            layout,
            alignment,
        }
    }

    /// Returns the image origin calculated from the immutable label geometry.
    #[must_use]
    pub fn image_origin(&self) -> PointI {
        let image = self.layout.image().size();
        let horizontal_room = self.bounds.width.saturating_sub(image.width);
        let offset_x = match self.alignment {
            TextAlignment::Leading => 0,
            TextAlignment::Center => horizontal_room / 2,
            TextAlignment::Trailing => horizontal_room,
        };
        let offset_y = self.bounds.height.saturating_sub(image.height) / 2;
        PointI::new(
            self.bounds
                .x
                .saturating_add(i32::try_from(offset_x).unwrap_or(i32::MAX)),
            self.bounds
                .y
                .saturating_add(i32::try_from(offset_y).unwrap_or(i32::MAX)),
        )
    }
}

impl Widget for TextLabel {
    fn id(&self) -> &NodeId {
        &self.id
    }

    fn bounds(&self) -> RectI {
        self.bounds
    }

    fn build_display_list(&self, display_list: &mut DisplayList) {
        display_list.draw_image_clipped(
            self.image_origin(),
            self.layout.image().clone(),
            self.bounds,
        );
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        vec![
            AccessibilityNode::new(self.id.clone(), AccessibilityRole::Label, self.bounds)
                .with_label(self.text.clone()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{TextAlignment, TextLabel, TextLabelCache};
    use crate::Widget;
    use luna_core::{NodeId, RectI};
    use luna_text::TextDocument;
    use luna_text_cosmic::{TextEngine, TextLayoutRequest};
    use luna_theme::Theme;
    use std::error::Error;

    #[test]
    fn centered_label_uses_the_same_clip_and_semantic_bounds() -> Result<(), Box<dyn Error>> {
        let theme = Theme::luna_dark();
        let mut engine = TextEngine::new();
        let layout = engine.shape(
            &TextDocument::new("Luna"),
            TextLayoutRequest::new(80, 14.0, 18.0, theme.foreground),
        )?;
        let label = TextLabel::new(
            NodeId::new("label")?,
            RectI::new(10, 20, 120, 30),
            "Luna",
            layout,
            TextAlignment::Center,
        );

        assert!(label.bounds().contains(label.image_origin()));
        assert_eq!(label.accessibility_nodes()[0].bounds, label.bounds());
        Ok(())
    }

    #[test]
    fn label_cache_reuses_shape_across_position_changes() -> Result<(), Box<dyn Error>> {
        let theme = Theme::luna_dark();
        let mut engine = TextEngine::new();
        let mut cache = TextLabelCache::new();

        let first = cache.layout(
            &mut engine,
            "status-slot",
            "Status",
            120,
            13.0,
            19.0,
            theme.foreground,
        )?;
        let second = cache.layout(
            &mut engine,
            "status-slot",
            "Status",
            120,
            13.0,
            19.0,
            theme.foreground,
        )?;

        assert_eq!(first, second);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().entries, 1);
        Ok(())
    }

    #[test]
    fn label_cache_replaces_dynamic_slot_content() -> Result<(), Box<dyn Error>> {
        let theme = Theme::luna_dark();
        let mut engine = TextEngine::new();
        let mut cache = TextLabelCache::new();

        let _ = cache.layout(
            &mut engine,
            "status-slot",
            "Line 1",
            120,
            13.0,
            19.0,
            theme.foreground,
        )?;
        let _ = cache.layout(
            &mut engine,
            "status-slot",
            "Line 2",
            120,
            13.0,
            19.0,
            theme.foreground,
        )?;

        assert_eq!(cache.stats().misses, 2);
        assert_eq!(cache.stats().entries, 1);
        Ok(())
    }
}
