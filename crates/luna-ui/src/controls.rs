// SPDX-License-Identifier: MPL-2.0

use crate::{TextAlignment, TextLabel, Widget};
use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{InsetsI, NodeId, PointI, RectI};
use luna_render::DisplayList;
use luna_text_cosmic::TextLayoutSnapshot;
use luna_theme::{Rgba8, Theme};

/// Shared visual state for clickable proof and editor controls.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ControlState {
    /// Whether the pointer is currently over the control.
    pub is_hovered: bool,
    /// Whether the control owns semantic keyboard focus.
    pub is_focused: bool,
    /// Whether the control is disabled.
    pub is_disabled: bool,
}

/// Reusable push button with immutable shaped text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Button {
    id: NodeId,
    bounds: RectI,
    label: String,
    label_layout: TextLayoutSnapshot,
    theme: Theme,
    state: ControlState,
}

impl Button {
    /// Creates a push button.
    #[must_use]
    pub fn new(
        id: NodeId,
        bounds: RectI,
        label: impl Into<String>,
        label_layout: TextLayoutSnapshot,
        theme: Theme,
        state: ControlState,
    ) -> Self {
        Self {
            id,
            bounds,
            label: label.into(),
            label_layout,
            theme,
            state,
        }
    }

    fn label_widget(&self) -> Option<TextLabel> {
        let id = self.id.child("label").ok()?;
        Some(TextLabel::new(
            id,
            self.bounds.inset(InsetsI::symmetric(8, 4)),
            self.label.clone(),
            self.label_layout.clone(),
            TextAlignment::Center,
        ))
    }
}

impl Widget for Button {
    fn id(&self) -> &NodeId {
        &self.id
    }

    fn bounds(&self) -> RectI {
        self.bounds
    }

    fn build_display_list(&self, display_list: &mut DisplayList) {
        let color = if self.state.is_disabled {
            self.theme.panel_header.mix(self.theme.background, 96)
        } else if self.state.is_hovered || self.state.is_focused {
            self.theme.hover_surface()
        } else {
            self.theme.panel_header
        };
        display_list.fill_rect(self.bounds, color);
        if self.state.is_focused {
            let border = self.bounds.inset(InsetsI::symmetric(2, 2));
            display_list.fill_rect(
                RectI::new(border.x, border.y, border.width, 2),
                self.theme.accent,
            );
            display_list.fill_rect(
                RectI::new(
                    border.x,
                    i32::try_from(border.bottom().saturating_sub(2)).unwrap_or(i32::MAX),
                    border.width,
                    2,
                ),
                self.theme.accent,
            );
        }
        if let Some(label) = self.label_widget() {
            label.build_display_list(display_list);
        }
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        vec![
            AccessibilityNode::new(self.id.clone(), AccessibilityRole::Button, self.bounds)
                .with_label(self.label.clone())
                .with_disabled(self.state.is_disabled)
                .with_focused(self.state.is_focused),
        ]
    }
}

/// Reusable binary toggle rendered as a compact switch plus label.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Toggle {
    id: NodeId,
    bounds: RectI,
    label: String,
    label_layout: TextLayoutSnapshot,
    theme: Theme,
    state: ControlState,
    is_on: bool,
}

impl Toggle {
    /// Creates a binary toggle.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        id: NodeId,
        bounds: RectI,
        label: impl Into<String>,
        label_layout: TextLayoutSnapshot,
        theme: Theme,
        state: ControlState,
        is_on: bool,
    ) -> Self {
        Self {
            id,
            bounds,
            label: label.into(),
            label_layout,
            theme,
            state,
            is_on,
        }
    }

    /// Returns the switch lane shared by painting and pointer explanation.
    #[must_use]
    pub fn switch_bounds(&self) -> RectI {
        RectI::new(
            self.bounds.x,
            self.bounds.y.saturating_add(4),
            42_u32.min(self.bounds.width),
            self.bounds.height.saturating_sub(8).max(1),
        )
    }
}

impl Widget for Toggle {
    fn id(&self) -> &NodeId {
        &self.id
    }

    fn bounds(&self) -> RectI {
        self.bounds
    }

    fn build_display_list(&self, display_list: &mut DisplayList) {
        let lane = self.switch_bounds();
        let lane_color = if self.state.is_disabled {
            self.theme.panel_header.mix(self.theme.background, 96)
        } else if self.is_on && self.state.is_hovered {
            self.theme.accent.mix(self.theme.foreground, 40)
        } else if self.is_on {
            self.theme.accent
        } else if self.state.is_hovered {
            self.theme.hover_surface()
        } else {
            self.theme.panel_header
        };
        display_list.fill_rect(lane, lane_color);
        let knob_size = lane.height.saturating_sub(6).max(1);
        let knob_x = if self.is_on {
            i32::try_from(lane.right().saturating_sub(i64::from(knob_size) + 3)).unwrap_or(i32::MAX)
        } else {
            lane.x.saturating_add(3)
        };
        display_list.fill_rect(
            RectI::new(knob_x, lane.y.saturating_add(3), knob_size, knob_size),
            self.theme.foreground,
        );
        let label_bounds = RectI::new(
            lane.x
                .saturating_add(i32::try_from(lane.width).unwrap_or(i32::MAX))
                .saturating_add(10),
            self.bounds.y,
            self.bounds
                .width
                .saturating_sub(lane.width)
                .saturating_sub(10),
            self.bounds.height,
        );
        if let Ok(label_id) = self.id.child("label") {
            TextLabel::new(
                label_id,
                label_bounds,
                self.label.clone(),
                self.label_layout.clone(),
                TextAlignment::Leading,
            )
            .build_display_list(display_list);
        }
        if self.state.is_focused {
            display_list.fill_rect(
                RectI::new(self.bounds.x, self.bounds.y, self.bounds.width, 2),
                self.theme.accent,
            );
        }
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        vec![
            AccessibilityNode::new(self.id.clone(), AccessibilityRole::CheckBox, self.bounds)
                .with_label(self.label.clone())
                .with_value(if self.is_on { "On" } else { "Off" })
                .with_disabled(self.state.is_disabled)
                .with_focused(self.state.is_focused),
        ]
    }
}

/// Deterministic progress indicator used by proof cards and editor status surfaces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressBar {
    id: NodeId,
    bounds: RectI,
    theme: Theme,
    value: u16,
    maximum: u16,
    label: String,
}

impl ProgressBar {
    /// Creates a progress indicator. A zero maximum is normalized to one.
    #[must_use]
    pub fn new(
        id: NodeId,
        bounds: RectI,
        theme: Theme,
        value: u16,
        maximum: u16,
        label: impl Into<String>,
    ) -> Self {
        let maximum = maximum.max(1);
        Self {
            id,
            bounds,
            theme,
            value: value.min(maximum),
            maximum,
            label: label.into(),
        }
    }

    /// Returns the completed fill geometry.
    #[must_use]
    pub fn fill_bounds(&self) -> RectI {
        let width = u64::from(self.bounds.width).saturating_mul(u64::from(self.value))
            / u64::from(self.maximum);
        RectI::new(
            self.bounds.x,
            self.bounds.y,
            u32::try_from(width)
                .unwrap_or(u32::MAX)
                .min(self.bounds.width),
            self.bounds.height,
        )
    }
}

impl Widget for ProgressBar {
    fn id(&self) -> &NodeId {
        &self.id
    }

    fn bounds(&self) -> RectI {
        self.bounds
    }

    fn build_display_list(&self, display_list: &mut DisplayList) {
        display_list.fill_rect(self.bounds, self.theme.panel_header);
        display_list.fill_rect(self.fill_bounds(), self.theme.accent);
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        vec![
            AccessibilityNode::new(
                self.id.clone(),
                AccessibilityRole::ProgressIndicator,
                self.bounds,
            )
            .with_label(self.label.clone())
            .with_value(format!("{} of {}", self.value, self.maximum)),
        ]
    }

    fn hit_test(&self, _point: PointI) -> Option<NodeId> {
        None
    }
}

/// Returns a one-pixel border color used by product-neutral cards.
#[must_use]
pub fn card_border(theme: Theme) -> Rgba8 {
    theme.border()
}

#[cfg(test)]
mod tests {
    use super::ProgressBar;
    use crate::Widget;
    use luna_core::{NodeId, RectI};
    use luna_theme::Theme;
    use std::error::Error;

    #[test]
    fn progress_geometry_is_clamped_to_its_track() -> Result<(), Box<dyn Error>> {
        let progress = ProgressBar::new(
            NodeId::new("progress")?,
            RectI::new(10, 20, 100, 8),
            Theme::luna_dark(),
            150,
            100,
            "Build",
        );

        assert_eq!(progress.fill_bounds().width, 100);
        assert_eq!(
            progress.accessibility_nodes()[0].value.as_deref(),
            Some("100 of 100")
        );
        Ok(())
    }
}
