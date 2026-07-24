// SPDX-License-Identifier: MPL-2.0

use crate::{Widget, card_border};
use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{InsetsI, NodeId, NodeIdError, PointI, RectI};
use luna_render::DisplayList;
use luna_theme::Theme;

/// Mutable state visualized by the native proof gallery.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProofGalleryState {
    /// Number of times the primary proof button has been activated.
    pub activation_count: u32,
    /// Binary toggle proof value.
    pub toggle_is_on: bool,
    /// Logical animation time in milliseconds.
    pub animation_millis: u64,
}

/// One named gallery card.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofCardFrame {
    /// Stable card ID.
    pub id: String,
    /// Stable semantic node ID.
    pub node_id: NodeId,
    /// Visible title.
    pub title: String,
    /// Complete card bounds.
    pub bounds: RectI,
    /// Inner content bounds.
    pub content: RectI,
}

/// Complete proof-gallery geometry snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofGalleryLayout {
    /// Complete gallery bounds.
    pub bounds: RectI,
    /// Top gallery title region.
    pub header: RectI,
    /// Subtitle/status line.
    pub subtitle: RectI,
    /// Responsive proof cards.
    pub cards: Vec<ProofCardFrame>,
    /// Push-button proof bounds.
    pub button: RectI,
    /// Toggle proof bounds.
    pub toggle: RectI,
    /// Progress proof bounds.
    pub progress: RectI,
    /// First pane in the deterministic split proof.
    pub split_primary: RectI,
    /// Divider in the deterministic split proof.
    pub split_divider: RectI,
    /// Second pane in the deterministic split proof.
    pub split_secondary: RectI,
    /// Multilingual text fixture bounds.
    pub text_sample: RectI,
    /// Animation viewport.
    pub animation_lane: RectI,
    /// Animated square inside the animation viewport.
    pub animation_square: RectI,
    /// Accessibility explanation bounds.
    pub accessibility_note: RectI,
}

impl ProofGalleryLayout {
    /// Returns this immutable geometry with only the animation square recomputed.
    ///
    /// Responsive card, control, and text geometry remain unchanged, allowing applications to retain
    /// layout across animation samples.
    #[must_use]
    pub fn with_animation_millis(mut self, animation_millis: u64) -> Self {
        self.animation_square = animation_square(self.animation_lane, animation_millis);
        self
    }
}

/// Product-neutral regression gallery chrome.
///
/// The gallery intentionally keeps animation and proof-only state out of the editor demo. Its
/// deterministic card geometry is suitable for screenshots, manual QA, and future golden tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofGallery {
    id: NodeId,
    bounds: RectI,
    theme: Theme,
    state: ProofGalleryState,
    layout: ProofGalleryLayout,
}

impl ProofGallery {
    /// Creates the responsive proof gallery.
    pub fn new(
        id: NodeId,
        bounds: RectI,
        theme: Theme,
        state: ProofGalleryState,
    ) -> Result<Self, NodeIdError> {
        let layout = calculate_layout(&id, bounds, state)?;
        Ok(Self {
            id,
            bounds,
            theme,
            state,
            layout,
        })
    }

    /// Reuses a previously calculated layout while advancing only animation geometry.
    #[must_use]
    pub fn from_layout_snapshot(
        id: NodeId,
        theme: Theme,
        state: ProofGalleryState,
        layout: ProofGalleryLayout,
    ) -> Self {
        let layout = layout.with_animation_millis(state.animation_millis);
        Self {
            id,
            bounds: layout.bounds,
            theme,
            state,
            layout,
        }
    }

    /// Returns shared proof geometry.
    #[must_use]
    pub const fn layout(&self) -> &ProofGalleryLayout {
        &self.layout
    }

    /// Appends paint that is independent from logical animation time.
    pub fn build_static_display_list(&self, display_list: &mut DisplayList) {
        display_list.fill_rect(self.layout.header, self.theme.panel_header);
        for card in &self.layout.cards {
            display_list.fill_rect(card.bounds, self.theme.panel);
            draw_border(display_list, card.bounds, card_border(self.theme));
            let title_rule = RectI::new(
                card.bounds.x,
                card.bounds.y.saturating_add(36),
                card.bounds.width,
                1,
            );
            display_list.fill_rect(title_rule, self.theme.border());
        }
        display_list.fill_rect(self.layout.split_primary, self.theme.hover_surface());
        display_list.fill_rect(self.layout.split_divider, self.theme.accent);
        display_list.fill_rect(self.layout.split_secondary, self.theme.panel_header);
        display_list.fill_rect(self.layout.animation_lane, self.theme.background);
    }

    /// Appends only time-varying animation paint.
    pub fn build_animation_display_list(&self, display_list: &mut DisplayList) {
        display_list.fill_rect(self.layout.animation_square, self.theme.accent);
        let pulse = u8::try_from((self.state.animation_millis / 8) % 160).unwrap_or(0);
        display_list.fill_rect(
            RectI::new(
                self.layout.animation_lane.x,
                i32::try_from(self.layout.animation_lane.bottom().saturating_sub(4))
                    .unwrap_or(i32::MAX),
                self.layout.animation_lane.width,
                4,
            ),
            self.theme
                .accent
                .with_alpha(80_u8.saturating_add(pulse.min(120))),
        );
    }
}

impl Widget for ProofGallery {
    fn id(&self) -> &NodeId {
        &self.id
    }

    fn bounds(&self) -> RectI {
        self.bounds
    }

    fn build_display_list(&self, display_list: &mut DisplayList) {
        self.build_static_display_list(display_list);
        self.build_animation_display_list(display_list);
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        let children = self
            .layout
            .cards
            .iter()
            .map(|card| card.node_id.clone())
            .collect();
        let mut nodes = vec![
            AccessibilityNode::new(self.id.clone(), AccessibilityRole::Group, self.bounds)
                .with_label("Luna UI Rust proof gallery")
                .with_children(children),
        ];
        for card in &self.layout.cards {
            let role = if card.id == "theme" {
                AccessibilityRole::Button
            } else {
                AccessibilityRole::Group
            };
            let node = AccessibilityNode::new(card.node_id.clone(), role, card.bounds)
                .with_label(card.title.clone());
            nodes.push(if card.id == "theme" {
                node.with_value(if self.theme == Theme::luna_light() {
                    "Light palette; activate for dark mode"
                } else {
                    "Dark palette; activate for light mode"
                })
            } else {
                node
            });
        }
        nodes
    }

    fn hit_test(&self, point: PointI) -> Option<NodeId> {
        self.layout
            .cards
            .iter()
            .find(|card| card.bounds.contains(point))
            .map(|card| card.node_id.clone())
            .or_else(|| self.bounds.contains(point).then_some(self.id.clone()))
    }
}

fn calculate_layout(
    id: &NodeId,
    bounds: RectI,
    state: ProofGalleryState,
) -> Result<ProofGalleryLayout, NodeIdError> {
    let outer = bounds.inset(InsetsI::symmetric(18, 18));
    let header_height = 54_u32.min(outer.height);
    let header = RectI::new(outer.x, outer.y, outer.width, header_height);
    let subtitle = RectI::new(
        header.x,
        header.y.saturating_add(30),
        header.width,
        header.height.saturating_sub(30),
    );
    let grid_y = header
        .y
        .saturating_add(i32::try_from(header.height).unwrap_or(i32::MAX))
        .saturating_add(14);
    let grid_height = u32::try_from(outer.bottom().saturating_sub(i64::from(grid_y))).unwrap_or(0);
    let gap = 12_u32;
    let columns = if outer.width >= 1_020 {
        3_u32
    } else if outer.width >= 680 {
        2
    } else {
        1
    };
    let card_count = 6_u32;
    let rows = card_count.saturating_add(columns.saturating_sub(1)) / columns.max(1);
    let card_width = outer
        .width
        .saturating_sub(gap.saturating_mul(columns.saturating_sub(1)))
        / columns.max(1);
    let card_height = grid_height.saturating_sub(gap.saturating_mul(rows.saturating_sub(1))) / rows;
    let definitions = [
        ("controls", "Controls and State"),
        ("layout", "Deterministic Layout"),
        ("text", "Text and Fallback"),
        ("animation", "Timed Invalidation"),
        ("accessibility", "Accessibility Semantics"),
        ("theme", "Theme Tokens"),
    ];
    let mut cards = Vec::new();
    for (index, (card_id, title)) in definitions.iter().enumerate() {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        let column = index % columns;
        let row = index / columns;
        let x = outer.x.saturating_add(
            i32::try_from(column.saturating_mul(card_width.saturating_add(gap)))
                .unwrap_or(i32::MAX),
        );
        let y = grid_y.saturating_add(
            i32::try_from(row.saturating_mul(card_height.saturating_add(gap))).unwrap_or(i32::MAX),
        );
        let bounds = RectI::new(x, y, card_width, card_height);
        cards.push(ProofCardFrame {
            id: (*card_id).to_owned(),
            node_id: id.child(card_id)?,
            title: (*title).to_owned(),
            bounds,
            content: bounds.inset(InsetsI::new(48, 14, 14, 14)),
        });
    }

    let controls = &cards[0].content;
    let button = RectI::new(
        controls.x,
        controls.y,
        150_u32.min(controls.width),
        34_u32.min(controls.height),
    );
    let toggle_y = button.y.saturating_add(44);
    let toggle = RectI::new(
        controls.x,
        toggle_y,
        controls.width,
        34_u32.min(controls.height),
    );
    let progress_y = toggle.y.saturating_add(48);
    let progress = RectI::new(
        controls.x,
        progress_y,
        controls.width,
        10_u32.min(controls.height),
    );

    let layout_content = cards[1].content;
    let divider_width = 5_u32.min(layout_content.width);
    let remaining = layout_content.width.saturating_sub(divider_width);
    let primary_width = remaining.saturating_mul(2) / 5;
    let split_primary = RectI::new(
        layout_content.x,
        layout_content.y,
        primary_width,
        layout_content.height,
    );
    let split_divider = RectI::new(
        layout_content
            .x
            .saturating_add(i32::try_from(primary_width).unwrap_or(i32::MAX)),
        layout_content.y,
        divider_width,
        layout_content.height,
    );
    let split_secondary = RectI::new(
        split_divider
            .x
            .saturating_add(i32::try_from(divider_width).unwrap_or(i32::MAX)),
        layout_content.y,
        remaining.saturating_sub(primary_width),
        layout_content.height,
    );

    let text_sample = cards[2].content;
    let animation_lane = cards[3].content.inset(InsetsI::symmetric(6, 20));
    let animation_square = animation_square(animation_lane, state.animation_millis);
    let accessibility_note = cards[4].content;

    Ok(ProofGalleryLayout {
        bounds,
        header,
        subtitle,
        cards,
        button,
        toggle,
        progress,
        split_primary,
        split_divider,
        split_secondary,
        text_sample,
        animation_lane,
        animation_square,
        accessibility_note,
    })
}

fn animation_square(animation_lane: RectI, animation_millis: u64) -> RectI {
    let square_size = 26_u32.min(animation_lane.height).min(animation_lane.width);
    let travel = animation_lane.width.saturating_sub(square_size);
    let offset = if travel == 0 {
        0
    } else {
        let cycle = u64::from(travel).saturating_mul(2);
        let phase = animation_millis / 5 % cycle;
        if phase <= u64::from(travel) {
            phase
        } else {
            cycle.saturating_sub(phase)
        }
    };
    RectI::new(
        animation_lane
            .x
            .saturating_add(i32::try_from(offset).unwrap_or(i32::MAX)),
        animation_lane.y.saturating_add(
            i32::try_from(animation_lane.height.saturating_sub(square_size) / 2).unwrap_or(0),
        ),
        square_size,
        square_size,
    )
}

fn draw_border(display_list: &mut DisplayList, bounds: RectI, color: luna_theme::Rgba8) {
    if bounds.is_empty() {
        return;
    }
    display_list.fill_rect(RectI::new(bounds.x, bounds.y, bounds.width, 1), color);
    display_list.fill_rect(
        RectI::new(
            bounds.x,
            i32::try_from(bounds.bottom().saturating_sub(1)).unwrap_or(i32::MAX),
            bounds.width,
            1,
        ),
        color,
    );
    display_list.fill_rect(RectI::new(bounds.x, bounds.y, 1, bounds.height), color);
    display_list.fill_rect(
        RectI::new(
            i32::try_from(bounds.right().saturating_sub(1)).unwrap_or(i32::MAX),
            bounds.y,
            1,
            bounds.height,
        ),
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::{ProofGallery, ProofGalleryState};
    use crate::Widget;
    use luna_core::{NodeId, RectI};
    use luna_theme::Theme;
    use std::error::Error;

    #[test]
    fn wide_gallery_builds_six_cards_in_three_columns() -> Result<(), Box<dyn Error>> {
        let gallery = ProofGallery::new(
            NodeId::new("gallery")?,
            RectI::new(0, 0, 1_200, 760),
            Theme::luna_dark(),
            ProofGalleryState::default(),
        )?;

        assert_eq!(gallery.layout().cards.len(), 6);
        assert_eq!(
            gallery.layout().cards[0].bounds.y,
            gallery.layout().cards[1].bounds.y
        );
        let accessibility = gallery.accessibility_nodes();
        assert_eq!(accessibility.len(), 7);
        assert_eq!(
            accessibility.last().map(|node| node.role),
            Some(luna_accessibility::AccessibilityRole::Button)
        );
        Ok(())
    }

    #[test]
    fn reused_layout_changes_only_animation_geometry() -> Result<(), Box<dyn Error>> {
        let initial = ProofGallery::new(
            NodeId::new("gallery")?,
            RectI::new(0, 0, 900, 700),
            Theme::luna_dark(),
            ProofGalleryState::default(),
        )?;
        let initial_layout = initial.layout().clone();
        let advanced = ProofGallery::from_layout_snapshot(
            NodeId::new("gallery")?,
            Theme::luna_dark(),
            ProofGalleryState {
                animation_millis: 500,
                ..ProofGalleryState::default()
            },
            initial_layout.clone(),
        );

        assert_eq!(advanced.layout().cards, initial_layout.cards);
        assert_eq!(advanced.layout().button, initial_layout.button);
        assert_ne!(
            advanced.layout().animation_square,
            initial_layout.animation_square
        );
        Ok(())
    }

    #[test]
    fn animation_square_never_leaves_its_lane() -> Result<(), Box<dyn Error>> {
        let gallery = ProofGallery::new(
            NodeId::new("gallery")?,
            RectI::new(0, 0, 900, 700),
            Theme::luna_dark(),
            ProofGalleryState {
                animation_millis: u64::MAX,
                ..ProofGalleryState::default()
            },
        )?;
        let square = gallery.layout().animation_square;
        let lane = gallery.layout().animation_lane;
        assert!(lane.contains(luna_core::PointI::new(square.x, square.y)));
        assert!(square.right() <= lane.right());
        Ok(())
    }
}
