// SPDX-License-Identifier: MPL-2.0

use crate::Widget;
use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{InsetsI, NodeId, RectI};
use luna_render::DisplayList;
use luna_theme::Theme;

/// A deliberately small proof widget used by M0 tests and the headless demo.
///
/// The panel proves Luna's architecture before text shaping or a native window backend is added:
/// stable identity, deterministic geometry, display-list emission, hit testing, and accessibility
/// are all exercised by one reusable widget.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemoPanel {
    id: NodeId,
    bounds: RectI,
    theme: Theme,
    title: String,
}

impl DemoPanel {
    /// Creates a proof panel.
    #[must_use]
    pub fn new(id: NodeId, bounds: RectI, theme: Theme, title: impl Into<String>) -> Self {
        Self {
            id,
            bounds,
            theme,
            title: title.into(),
        }
    }

    fn header_bounds(&self) -> RectI {
        RectI::new(self.bounds.x, self.bounds.y, self.bounds.width, 42)
            .intersection(self.bounds)
            .unwrap_or_default()
    }

    fn accent_bounds(&self) -> RectI {
        self.bounds.inset(InsetsI::new(58, 24, 24, 24))
    }
}

impl Widget for DemoPanel {
    fn id(&self) -> &NodeId {
        &self.id
    }

    fn bounds(&self) -> RectI {
        self.bounds
    }

    fn build_display_list(&self, display_list: &mut DisplayList) {
        display_list.fill_rect(self.bounds, self.theme.panel);
        display_list.fill_rect(self.header_bounds(), self.theme.panel_header);
        display_list.fill_rect(self.accent_bounds(), self.theme.accent);
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        vec![
            AccessibilityNode::new(self.id.clone(), AccessibilityRole::Group, self.bounds)
                .with_label(self.title.clone()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::DemoPanel;
    use crate::{UiFrame, Widget};
    use luna_core::{NodeId, PointI, RectI};
    use luna_theme::Theme;
    use std::error::Error;

    #[test]
    fn paint_hit_testing_and_accessibility_share_bounds() -> Result<(), Box<dyn Error>> {
        let panel = DemoPanel::new(
            NodeId::new("demo")?,
            RectI::new(20, 30, 200, 100),
            Theme::luna_dark(),
            "Luna UI Rust proof panel",
        );
        let frame = UiFrame::build(&panel, Theme::luna_dark().background)?;

        assert_eq!(
            panel.hit_test(PointI::new(20, 30)),
            Some(panel.id().clone())
        );
        assert_eq!(
            frame
                .accessibility_tree
                .node(panel.id())
                .map(|node| node.bounds),
            Some(panel.bounds())
        );
        assert_eq!(frame.display_list.commands().len(), 4);
        Ok(())
    }
}
