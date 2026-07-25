// SPDX-License-Identifier: MPL-2.0

use crate::Widget;
use luna_accessibility::{AccessibilityNode, AccessibilityRole, AccessibilityTextRange};
use luna_core::{InsetsI, NodeId, PointI, RectI, SizeI};
use luna_render::DisplayList;
use luna_text::{SnapBias, TextDocument, TextLocation, TextRange, TextScroll};
use luna_text_cosmic::TextLayoutSnapshot;
use luna_theme::{Rgba8, Theme};

/// Visual metrics and colors for Luna's editor text surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextViewStyle {
    /// Padding around text content.
    pub content_insets: InsetsI,
    /// Width reserved for a future line-number gutter.
    pub gutter_width: u32,
    /// Editor background.
    pub background: Rgba8,
    /// Gutter background.
    pub gutter_background: Rgba8,
    /// Current-line background.
    pub current_line_background: Rgba8,
    /// Selection highlight.
    pub selection_background: Rgba8,
    /// Caret color.
    pub caret: Rgba8,
    /// Width reserved for the vertical scrollbar.
    pub scrollbar_width: u32,
}

impl TextViewStyle {
    /// Creates the default editor style from Luna's semantic theme.
    #[must_use]
    pub const fn from_theme(theme: Theme) -> Self {
        Self {
            content_insets: InsetsI::new(8, 8, 8, 8),
            gutter_width: 44,
            background: theme.background,
            gutter_background: theme.panel,
            current_line_background: theme.panel_header,
            selection_background: Rgba8::new(
                theme.accent.red,
                theme.accent.green,
                theme.accent.blue,
                96,
            ),
            caret: theme.foreground,
            scrollbar_width: 10,
        }
    }
}

/// Immutable editor text widget built from a shaped snapshot.
///
/// The document and shaped snapshot are supplied by application state. Paint, hit testing,
/// scrolling limits, caret geometry, selection geometry, and accessibility all derive from those
/// exact inputs and the same viewport rectangle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextView {
    id: NodeId,
    bounds: RectI,
    document: TextDocument,
    layout: TextLayoutSnapshot,
    caret: TextLocation,
    selection: Option<TextRange>,
    scroll: TextScroll,
    style: TextViewStyle,
    label: String,
    is_focused: bool,
    is_editable: bool,
}

impl TextView {
    /// Creates a text surface.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        id: NodeId,
        bounds: RectI,
        document: TextDocument,
        layout: TextLayoutSnapshot,
        caret: TextLocation,
        selection: Option<TextRange>,
        scroll: TextScroll,
        style: TextViewStyle,
        label: impl Into<String>,
        is_focused: bool,
        is_editable: bool,
    ) -> Self {
        let caret = document.clamp_location(caret, SnapBias::Backward);
        let selection = selection
            .map(|range| document.clamp_range(range))
            .filter(|range| !range.is_collapsed());
        Self {
            id,
            bounds,
            document,
            layout,
            caret,
            selection,
            scroll,
            style,
            label: label.into(),
            is_focused,
            is_editable,
        }
    }

    /// Returns the complete text viewport excluding outer padding and gutter.
    #[must_use]
    pub fn text_viewport(&self) -> RectI {
        let inner = self.bounds.inset(self.style.content_insets);
        RectI::new(
            inner
                .x
                .saturating_add(i32::try_from(self.style.gutter_width).unwrap_or(i32::MAX)),
            inner.y,
            inner
                .width
                .saturating_sub(self.style.gutter_width)
                .saturating_sub(self.style.scrollbar_width),
            inner.height,
        )
    }

    /// Returns the vertical scrollbar track geometry.
    #[must_use]
    pub fn vertical_scrollbar_bounds(&self) -> RectI {
        let inner = self.bounds.inset(self.style.content_insets);
        let width = self.style.scrollbar_width.min(inner.width);
        RectI::new(
            i32::try_from(inner.right().saturating_sub(i64::from(width))).unwrap_or(inner.x),
            inner.y,
            width,
            inner.height,
        )
    }

    /// Returns the vertical scrollbar thumb geometry.
    #[must_use]
    pub fn vertical_scrollbar_thumb(&self) -> RectI {
        let track = self.vertical_scrollbar_bounds();
        let viewport = self.text_viewport();
        let content_height = self
            .layout
            .content_size()
            .height
            .max(viewport.height)
            .max(1);
        let maximum = self.maximum_scroll().y.max(0);
        if maximum == 0 || track.height == 0 {
            return track;
        }
        let proportional = track.height.saturating_mul(viewport.height) / content_height.max(1);
        let thumb_height = proportional.clamp(24_u32.min(track.height), track.height);
        let travel = track.height.saturating_sub(thumb_height);
        let offset = u32::try_from(self.scroll.y.max(0))
            .unwrap_or(u32::MAX)
            .saturating_mul(travel)
            / u32::try_from(maximum).unwrap_or(u32::MAX).max(1);
        RectI::new(
            track.x,
            track
                .y
                .saturating_add(i32::try_from(offset).unwrap_or(i32::MAX)),
            track.width,
            thumb_height,
        )
    }

    /// Maps a pointer position on the vertical track to a clamped scroll offset.
    #[must_use]
    pub fn scroll_y_for_scrollbar_point(&self, point: PointI) -> i32 {
        let track = self.vertical_scrollbar_bounds();
        let thumb = self.vertical_scrollbar_thumb();
        let maximum = self.maximum_scroll().y.max(0);
        let travel = track.height.saturating_sub(thumb.height);
        if maximum == 0 || travel == 0 {
            return 0;
        }
        let centered = point
            .y
            .saturating_sub(track.y)
            .saturating_sub(i32::try_from(thumb.height / 2).unwrap_or(0));
        let track_offset = u32::try_from(centered.max(0)).unwrap_or(0).min(travel);
        i32::try_from(
            track_offset.saturating_mul(u32::try_from(maximum).unwrap_or(u32::MAX)) / travel,
        )
        .unwrap_or(i32::MAX)
        .clamp(0, maximum)
    }

    /// Returns whether a pointer lies on the vertical scrollbar track.
    #[must_use]
    pub fn vertical_scrollbar_contains(&self, point: PointI) -> bool {
        self.vertical_scrollbar_bounds().contains(point)
    }

    /// Returns gutter geometry.
    #[must_use]
    pub fn gutter_bounds(&self) -> RectI {
        let inner = self.bounds.inset(self.style.content_insets);
        RectI::new(
            inner.x,
            inner.y,
            self.style.gutter_width.min(inner.width),
            inner.height,
        )
    }

    /// Maps a widget-space point to the nearest shaped text position.
    #[must_use]
    pub fn text_hit_test(&self, point: PointI) -> Option<TextLocation> {
        let viewport = self.text_viewport();
        if !viewport.contains(point) {
            return None;
        }
        let content_point = PointI::new(
            point
                .x
                .saturating_sub(viewport.x)
                .saturating_add(self.scroll.x),
            point
                .y
                .saturating_sub(viewport.y)
                .saturating_add(self.scroll.y),
        );
        self.layout
            .hit_test(content_point)
            .map(|location| self.document.clamp_location(location, SnapBias::Backward))
    }

    /// Returns the maximum scroll offsets for the current viewport.
    #[must_use]
    pub fn maximum_scroll(&self) -> PointI {
        let viewport = self.text_viewport();
        self.layout
            .maximum_scroll(SizeI::new(viewport.width, viewport.height))
    }

    /// Returns the visible widget-space caret rectangle.
    #[must_use]
    pub fn caret_bounds(&self) -> Option<RectI> {
        self.layout
            .caret_rect(self.caret)
            .and_then(|rectangle| self.translated_content_rect(rectangle))
    }

    /// Returns a scroll position that keeps the caret visible.
    #[must_use]
    pub fn scroll_revealing_caret(&self) -> TextScroll {
        let viewport = self.text_viewport();
        let maximum = self.maximum_scroll();
        let mut scroll = self.scroll;
        let Some(caret) = self.layout.caret_rect(self.caret) else {
            scroll.clamp(maximum.x, maximum.y);
            return scroll;
        };

        if caret.x < scroll.x {
            scroll.x = caret.x.max(0);
        } else if caret.right() > i64::from(scroll.x).saturating_add(i64::from(viewport.width)) {
            scroll.x = i32::try_from(caret.right().saturating_sub(i64::from(viewport.width)))
                .unwrap_or(i32::MAX);
        }
        if caret.y < scroll.y {
            scroll.y = caret.y.max(0);
        } else if caret.bottom() > i64::from(scroll.y).saturating_add(i64::from(viewport.height)) {
            scroll.y = i32::try_from(caret.bottom().saturating_sub(i64::from(viewport.height)))
                .unwrap_or(i32::MAX);
        }
        scroll.clamp(maximum.x, maximum.y);
        scroll
    }

    fn content_origin(&self) -> PointI {
        let viewport = self.text_viewport();
        PointI::new(
            viewport.x.saturating_sub(self.scroll.x),
            viewport.y.saturating_sub(self.scroll.y),
        )
    }

    fn translated_content_rect(&self, rectangle: RectI) -> Option<RectI> {
        let origin = self.content_origin();
        RectI::new(
            origin.x.saturating_add(rectangle.x),
            origin.y.saturating_add(rectangle.y),
            rectangle.width,
            rectangle.height,
        )
        .intersection(self.text_viewport())
    }

    fn visible_line_indices(&self) -> std::ops::RangeInclusive<usize> {
        let viewport = self.text_viewport();
        let visible = self
            .layout
            .visible_range(&self.document, self.scroll.y, viewport.height);
        visible.anchor.line_index..=visible.focus.line_index
    }

    fn line_node_id(&self, line_index: usize) -> Option<NodeId> {
        self.id.child(&format!("line-{line_index}")).ok()
    }
}

impl Widget for TextView {
    fn id(&self) -> &NodeId {
        &self.id
    }

    fn bounds(&self) -> RectI {
        self.bounds
    }

    fn build_display_list(&self, display_list: &mut DisplayList) {
        display_list.fill_rect(self.bounds, self.style.background);
        display_list.fill_rect(self.gutter_bounds(), self.style.gutter_background);
        let scrollbar = self.vertical_scrollbar_bounds();
        display_list.fill_rect(scrollbar, self.style.gutter_background);
        display_list.fill_rect(
            self.vertical_scrollbar_thumb(),
            self.style.current_line_background,
        );
        let viewport = self.text_viewport();

        if let Some(caret) = self.layout.caret_rect(self.caret) {
            let current_line =
                RectI::new(0, caret.y, self.layout.content_size().width, caret.height);
            if let Some(bounds) = self.translated_content_rect(current_line) {
                display_list.fill_rect(bounds, self.style.current_line_background);
            }
        }

        if let Some(selection) = self.selection {
            for rectangle in self.layout.selection_rects(selection) {
                if let Some(bounds) = self.translated_content_rect(rectangle) {
                    display_list.fill_rect(bounds, self.style.selection_background);
                }
            }
        }

        let content_origin = self.content_origin();
        let raster_bounds = self.layout.raster_bounds();
        display_list.draw_image_clipped(
            PointI::new(
                content_origin.x.saturating_add(raster_bounds.x),
                content_origin.y.saturating_add(raster_bounds.y),
            ),
            self.layout.image().clone(),
            viewport,
        );

        if self.is_focused
            && let Some(caret) = self.layout.caret_rect(self.caret)
            && let Some(bounds) = self.translated_content_rect(caret)
        {
            display_list.fill_rect(bounds, self.style.caret);
        }
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        let visible_range =
            self.layout
                .visible_range(&self.document, self.scroll.y, self.text_viewport().height);
        let total = AccessibilityTextRange::new(0, self.document.text().len());
        let caret_offset = self
            .document
            .absolute_offset(self.caret, SnapBias::Backward);
        let caret = AccessibilityTextRange::new(caret_offset, 0);
        let selected = self.selection.map(|range| {
            let absolute = self.document.absolute_range(range);
            AccessibilityTextRange::new(absolute.start, absolute.end.saturating_sub(absolute.start))
        });
        let visible_absolute = self.document.absolute_range(visible_range);
        let visible = AccessibilityTextRange::new(
            visible_absolute.start,
            visible_absolute.end.saturating_sub(visible_absolute.start),
        );

        let mut children = Vec::new();
        let mut nodes = Vec::new();
        for line_index in self.visible_line_indices() {
            let Some(id) = self.line_node_id(line_index) else {
                continue;
            };
            let Some(line) = self.document.line(line_index) else {
                continue;
            };
            let y = i32::try_from(line_index)
                .unwrap_or(i32::MAX)
                .saturating_mul(i32::try_from(self.layout.line_height()).unwrap_or(i32::MAX))
                .saturating_add(i32::try_from(self.layout.content_padding()).unwrap_or(i32::MAX));
            let local = RectI::new(
                0,
                y,
                self.layout.content_size().width,
                self.layout.line_height(),
            );
            let Some(bounds) = self.translated_content_rect(local) else {
                continue;
            };
            children.push(id.clone());
            nodes.push(
                AccessibilityNode::new(id, AccessibilityRole::Label, bounds)
                    .with_label(format!("Line {}", line.line_number()))
                    .with_value(line.text.clone())
                    .with_text_ranges(
                        Some(AccessibilityTextRange::new(
                            line.utf8_offset,
                            line.utf8_length,
                        )),
                        None,
                        None,
                        None,
                    ),
            );
        }

        let root =
            AccessibilityNode::new(self.id.clone(), AccessibilityRole::TextArea, self.bounds)
                .with_label(self.label.clone())
                .with_value(self.document.text().to_owned())
                .with_children(children)
                .with_focused(self.is_focused)
                .with_editable(self.is_editable)
                .with_text_ranges(Some(total), Some(caret), selected, Some(visible));
        nodes.insert(0, root);
        nodes
    }

    fn hit_test(&self, point: PointI) -> Option<NodeId> {
        self.bounds.contains(point).then_some(self.id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::{TextView, TextViewStyle};
    use crate::{UiFrame, Widget};
    use luna_core::{NodeId, PointI, RectI};
    use luna_text::{TextDocument, TextLocation, TextRange, TextScroll};
    use luna_text_cosmic::{TextEngine, TextLayoutRequest};
    use luna_theme::Theme;
    use std::error::Error;

    #[test]
    fn paint_hit_testing_and_accessibility_share_text_geometry() -> Result<(), Box<dyn Error>> {
        let theme = Theme::luna_dark();
        let document = TextDocument::new("alpha\nbeta\ngamma");
        let mut engine = TextEngine::new();
        let layout = engine.shape(
            &document,
            TextLayoutRequest::new(300, 14.0, 20.0, theme.foreground),
        )?;
        let view = TextView::new(
            NodeId::new("text-view")?,
            RectI::new(10, 20, 300, 100),
            document,
            layout,
            TextLocation::new(1, 2),
            Some(TextRange::new(
                TextLocation::new(0, 2),
                TextLocation::new(1, 2),
            )),
            TextScroll::default(),
            TextViewStyle::from_theme(theme),
            "Editor",
            true,
            true,
        );
        let frame = UiFrame::build(&view, theme.background)?;

        assert_eq!(view.hit_test(PointI::new(20, 30)), Some(view.id().clone()));
        assert!(view.text_hit_test(PointI::new(80, 50)).is_some());
        assert_eq!(
            frame
                .accessibility_tree
                .node(view.id())
                .and_then(|node| node.caret_range)
                .map(|range| range.utf8_offset),
            Some(8)
        );
        assert!(frame.display_list.commands().len() >= 5);
        Ok(())
    }

    #[test]
    fn scrollbar_thumb_and_pointer_mapping_are_clamped() -> Result<(), Box<dyn Error>> {
        let document = TextDocument::new(
            (0..120)
                .map(|index| format!("line {index}\n"))
                .collect::<String>(),
        );
        let mut engine = TextEngine::new();
        let layout = engine.shape(
            &document,
            TextLayoutRequest::new(400, 15.0, 22.0, Theme::luna_dark().foreground),
        )?;
        let view = TextView::new(
            NodeId::new("editor")?,
            RectI::new(0, 0, 500, 180),
            document,
            layout,
            TextLocation::default(),
            None,
            TextScroll::new(0, 100),
            TextViewStyle::from_theme(Theme::luna_dark()),
            "Editor",
            true,
            true,
        );
        let track = view.vertical_scrollbar_bounds();
        let thumb = view.vertical_scrollbar_thumb();
        assert!(track.contains(PointI::new(thumb.x, thumb.y)));
        assert_eq!(
            view.scroll_y_for_scrollbar_point(PointI::new(track.x, track.y)),
            0
        );
        assert_eq!(
            view.scroll_y_for_scrollbar_point(PointI::new(
                track.x,
                i32::try_from(track.bottom()).unwrap_or(i32::MAX),
            )),
            view.maximum_scroll().y,
        );
        Ok(())
    }
}
