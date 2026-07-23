// SPDX-License-Identifier: MPL-2.0

use crate::Widget;
use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{NodeId, PointI, RectI};
use luna_render::DisplayList;
use luna_text_cosmic::TextLayoutSnapshot;

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
    use super::{TextAlignment, TextLabel};
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
}
