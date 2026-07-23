// SPDX-License-Identifier: MPL-2.0

use crate::Widget;
use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{NodeId, NodeIdError, PointI, RectI};
use luna_render::DisplayList;
use luna_theme::Theme;

/// One command-palette result supplied by an application command registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaletteItem {
    /// Stable command ID.
    pub id: String,
    /// Visible command title.
    pub title: String,
    /// Optional shortcut/help text.
    pub detail: String,
}

impl PaletteItem {
    /// Creates a palette item.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: detail.into(),
        }
    }
}

/// Application-owned quick-panel state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandPaletteState {
    /// Current query text.
    pub query: String,
    /// Complete application command list.
    pub items: Vec<PaletteItem>,
    /// Selected index within the filtered list.
    pub selected_index: usize,
}

impl CommandPaletteState {
    /// Returns case-insensitive filtered items in stable source order.
    #[must_use]
    pub fn filtered_items(&self) -> Vec<&PaletteItem> {
        let query = self.query.to_lowercase();
        self.items
            .iter()
            .filter(|item| {
                query.is_empty()
                    || item.title.to_lowercase().contains(&query)
                    || item.id.to_lowercase().contains(&query)
            })
            .collect()
    }

    /// Normalizes selection after a query or item-list change.
    pub fn normalize_selection(&mut self) {
        let count = self.filtered_items().len();
        self.selected_index = if count == 0 {
            0
        } else {
            self.selected_index.min(count.saturating_sub(1))
        };
    }

    /// Selects the next visible result, wrapping at the end.
    pub fn select_next(&mut self) {
        let count = self.filtered_items().len();
        if count > 0 {
            self.selected_index = self.selected_index.saturating_add(1) % count;
        }
    }

    /// Selects the previous visible result, wrapping at the beginning.
    pub fn select_previous(&mut self) {
        let count = self.filtered_items().len();
        if count > 0 {
            self.selected_index = if self.selected_index == 0 {
                count.saturating_sub(1)
            } else {
                self.selected_index.saturating_sub(1)
            };
        }
    }

    /// Returns the selected filtered item.
    #[must_use]
    pub fn selected_item(&self) -> Option<&PaletteItem> {
        self.filtered_items().get(self.selected_index).copied()
    }
}

/// Geometry for one visible palette row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaletteRowFrame {
    /// Stable command ID.
    pub id: String,
    /// Stable semantic node ID.
    pub node_id: NodeId,
    /// Visible title.
    pub title: String,
    /// Shortcut/help detail.
    pub detail: String,
    /// Shared row bounds.
    pub bounds: RectI,
    /// Whether the row is selected.
    pub is_selected: bool,
}

/// Command-palette geometry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandPaletteLayout {
    /// Full-window backdrop.
    pub backdrop: RectI,
    /// Elevated panel.
    pub panel: RectI,
    /// Title bounds.
    pub title: RectI,
    /// Query field bounds.
    pub input: RectI,
    /// Result rows.
    pub rows: Vec<PaletteRowFrame>,
    /// Empty-state bounds.
    pub empty_state: RectI,
}

/// Reusable command-palette/quick-panel widget.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandPalette {
    id: NodeId,
    bounds: RectI,
    theme: Theme,
    state: CommandPaletteState,
    input_id: NodeId,
    list_id: NodeId,
    layout: CommandPaletteLayout,
}

impl CommandPalette {
    /// Creates a command palette centered inside the supplied window bounds.
    pub fn new(
        id: NodeId,
        bounds: RectI,
        theme: Theme,
        state: CommandPaletteState,
    ) -> Result<Self, NodeIdError> {
        let input_id = id.child("input")?;
        let list_id = id.child("results")?;
        let layout = calculate_palette_layout(&id, bounds, &state)?;
        Ok(Self {
            id,
            bounds,
            theme,
            state,
            input_id,
            list_id,
            layout,
        })
    }

    /// Returns immutable panel geometry.
    #[must_use]
    pub const fn layout(&self) -> &CommandPaletteLayout {
        &self.layout
    }

    /// Returns the stable query-input semantic ID.
    #[must_use]
    pub const fn input_node_id(&self) -> &NodeId {
        &self.input_id
    }

    /// Returns the command ID under a pointer position.
    #[must_use]
    pub fn command_at(&self, point: PointI) -> Option<&str> {
        self.layout
            .rows
            .iter()
            .find(|row| row.bounds.contains(point))
            .map(|row| row.id.as_str())
    }

    /// Returns the command represented by a stable palette-row semantic node.
    #[must_use]
    pub fn command_for_node(&self, node_id: &NodeId) -> Option<&str> {
        self.layout
            .rows
            .iter()
            .find(|row| &row.node_id == node_id)
            .map(|row| row.id.as_str())
    }
}

impl Widget for CommandPalette {
    fn id(&self) -> &NodeId {
        &self.id
    }

    fn bounds(&self) -> RectI {
        self.bounds
    }

    fn build_display_list(&self, display_list: &mut DisplayList) {
        display_list.fill_rect(self.layout.backdrop, self.theme.background.with_alpha(184));
        display_list.fill_rect(self.layout.panel, self.theme.panel);
        display_list.fill_rect(self.layout.title, self.theme.panel_header);
        display_list.fill_rect(self.layout.input, self.theme.background);
        for row in &self.layout.rows {
            if row.is_selected {
                display_list.fill_rect(row.bounds, self.theme.selection());
                display_list.fill_rect(
                    RectI::new(row.bounds.x, row.bounds.y, 3, row.bounds.height),
                    self.theme.accent,
                );
            }
        }
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        let mut nodes = vec![
            AccessibilityNode::new(
                self.id.clone(),
                AccessibilityRole::Dialog,
                self.layout.panel,
            )
            .with_label("Command Palette")
            .with_children(vec![self.input_id.clone(), self.list_id.clone()]),
            AccessibilityNode::new(
                self.input_id.clone(),
                AccessibilityRole::TextField,
                self.layout.input,
            )
            .with_label("Command query")
            .with_value(self.state.query.clone())
            .with_focused(true)
            .with_editable(true),
            AccessibilityNode::new(
                self.list_id.clone(),
                AccessibilityRole::List,
                self.layout.panel,
            )
            .with_label("Matching commands")
            .with_children(
                self.layout
                    .rows
                    .iter()
                    .map(|row| row.node_id.clone())
                    .collect(),
            ),
        ];
        for row in &self.layout.rows {
            nodes.push(
                AccessibilityNode::new(
                    row.node_id.clone(),
                    AccessibilityRole::MenuItem,
                    row.bounds,
                )
                .with_label(row.title.clone())
                .with_value(row.detail.clone())
                .with_focused(row.is_selected),
            );
        }
        nodes
    }

    fn hit_test(&self, point: PointI) -> Option<NodeId> {
        self.layout
            .rows
            .iter()
            .find(|row| row.bounds.contains(point))
            .map(|row| row.node_id.clone())
            .or_else(|| {
                self.layout
                    .input
                    .contains(point)
                    .then_some(self.input_id.clone())
            })
            .or_else(|| self.layout.panel.contains(point).then_some(self.id.clone()))
    }
}

fn calculate_palette_layout(
    id: &NodeId,
    bounds: RectI,
    state: &CommandPaletteState,
) -> Result<CommandPaletteLayout, NodeIdError> {
    let panel_width = 620_u32.min(bounds.width.saturating_sub(32)).max(1);
    let visible_items = state.filtered_items();
    let visible_count = visible_items.len().min(8);
    let row_height = 44_u32;
    let panel_height = 46_u32
        .saturating_add(48)
        .saturating_add(
            u32::try_from(visible_count)
                .unwrap_or(u32::MAX)
                .saturating_mul(row_height),
        )
        .saturating_add(12)
        .min(bounds.height.saturating_sub(32).max(1));
    let panel_x = bounds.x.saturating_add(
        i32::try_from(bounds.width.saturating_sub(panel_width) / 2).unwrap_or(i32::MAX),
    );
    let panel_y = bounds.y.saturating_add(54);
    let panel = RectI::new(panel_x, panel_y, panel_width, panel_height);
    let title = RectI::new(panel.x, panel.y, panel.width, 38_u32.min(panel.height));
    let input = RectI::new(
        panel.x.saturating_add(12),
        panel.y.saturating_add(46),
        panel.width.saturating_sub(24),
        34_u32.min(panel.height.saturating_sub(46)),
    );
    let mut rows = Vec::new();
    let mut y = input
        .y
        .saturating_add(i32::try_from(input.height).unwrap_or(i32::MAX))
        .saturating_add(8);
    for (index, item) in visible_items.into_iter().take(8).enumerate() {
        let remaining = u32::try_from(panel.bottom().saturating_sub(i64::from(y))).unwrap_or(0);
        let height = row_height.min(remaining);
        if height == 0 {
            break;
        }
        rows.push(PaletteRowFrame {
            id: item.id.clone(),
            node_id: id.child(&format!("item-{index}"))?,
            title: item.title.clone(),
            detail: item.detail.clone(),
            bounds: RectI::new(
                panel.x.saturating_add(8),
                y,
                panel.width.saturating_sub(16),
                height,
            ),
            is_selected: index == state.selected_index,
        });
        y = y.saturating_add(i32::try_from(height).unwrap_or(i32::MAX));
    }
    let empty_state = RectI::new(input.x, y, input.width, 36);
    Ok(CommandPaletteLayout {
        backdrop: bounds,
        panel,
        title,
        input,
        rows,
        empty_state,
    })
}

/// Active editable field in a find/replace panel.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FindField {
    /// Find query field.
    #[default]
    Query,
    /// Replacement text field.
    Replacement,
}

/// Application-owned find/replace panel state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FindPanelState {
    /// Search query.
    pub query: String,
    /// Replacement text.
    pub replacement: String,
    /// Number of current literal matches.
    pub match_count: usize,
    /// One-based selected match number, or zero when no match is selected.
    pub selected_match: usize,
    /// Active editable field.
    pub active_field: FindField,
    /// Whether replacement UI is visible.
    pub replacement_is_visible: bool,
}

/// Find/replace panel geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FindPanelLayout {
    /// Complete panel.
    pub panel: RectI,
    /// Query field.
    pub query: RectI,
    /// Replacement field.
    pub replacement: RectI,
    /// Previous-match button.
    pub previous: RectI,
    /// Next-match button.
    pub next: RectI,
    /// Close button.
    pub close: RectI,
    /// Match status label.
    pub status: RectI,
}

/// Compact editor find/replace overlay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FindPanel {
    id: NodeId,
    bounds: RectI,
    theme: Theme,
    state: FindPanelState,
    query_id: NodeId,
    replacement_id: NodeId,
    previous_id: NodeId,
    next_id: NodeId,
    close_id: NodeId,
    status_id: NodeId,
    layout: FindPanelLayout,
}

impl FindPanel {
    /// Creates a top-right find/replace panel.
    pub fn new(
        id: NodeId,
        bounds: RectI,
        theme: Theme,
        state: FindPanelState,
    ) -> Result<Self, NodeIdError> {
        let query_id = id.child("query")?;
        let replacement_id = id.child("replacement")?;
        let previous_id = id.child("previous")?;
        let next_id = id.child("next")?;
        let close_id = id.child("close")?;
        let status_id = id.child("status")?;
        let layout = calculate_find_layout(bounds, state.replacement_is_visible);
        Ok(Self {
            id,
            bounds,
            theme,
            state,
            query_id,
            replacement_id,
            previous_id,
            next_id,
            close_id,
            status_id,
            layout,
        })
    }

    /// Returns immutable find-panel geometry.
    #[must_use]
    pub const fn layout(&self) -> FindPanelLayout {
        self.layout
    }

    /// Returns the active field bounds.
    #[must_use]
    pub const fn active_field_bounds(&self) -> RectI {
        match self.state.active_field {
            FindField::Query => self.layout.query,
            FindField::Replacement => self.layout.replacement,
        }
    }

    /// Returns the stable query-field semantic ID.
    #[must_use]
    pub const fn query_node_id(&self) -> &NodeId {
        &self.query_id
    }

    /// Returns the stable replacement-field semantic ID.
    #[must_use]
    pub const fn replacement_node_id(&self) -> &NodeId {
        &self.replacement_id
    }

    /// Returns the stable previous-match semantic ID.
    #[must_use]
    pub const fn previous_node_id(&self) -> &NodeId {
        &self.previous_id
    }

    /// Returns the stable next-match semantic ID.
    #[must_use]
    pub const fn next_node_id(&self) -> &NodeId {
        &self.next_id
    }

    /// Returns the stable close-button semantic ID.
    #[must_use]
    pub const fn close_node_id(&self) -> &NodeId {
        &self.close_id
    }

    /// Returns the stable match-status semantic ID.
    #[must_use]
    pub const fn status_node_id(&self) -> &NodeId {
        &self.status_id
    }
}

impl Widget for FindPanel {
    fn id(&self) -> &NodeId {
        &self.id
    }

    fn bounds(&self) -> RectI {
        self.bounds
    }

    fn build_display_list(&self, display_list: &mut DisplayList) {
        display_list.fill_rect(self.layout.panel, self.theme.panel);
        display_list.fill_rect(self.layout.query, self.theme.background);
        if self.state.replacement_is_visible {
            display_list.fill_rect(self.layout.replacement, self.theme.background);
        }
        display_list.fill_rect(self.layout.previous, self.theme.panel_header);
        display_list.fill_rect(self.layout.next, self.theme.panel_header);
        display_list.fill_rect(self.layout.close, self.theme.panel_header);
        let active = self.active_field_bounds();
        display_list.fill_rect(
            RectI::new(active.x, active.y, active.width, 2),
            self.theme.accent,
        );
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        let mut children = vec![
            self.query_id.clone(),
            self.previous_id.clone(),
            self.next_id.clone(),
            self.close_id.clone(),
            self.status_id.clone(),
        ];
        if self.state.replacement_is_visible {
            children.insert(1, self.replacement_id.clone());
        }
        let mut nodes = vec![
            AccessibilityNode::new(
                self.id.clone(),
                AccessibilityRole::Dialog,
                self.layout.panel,
            )
            .with_label("Find and Replace")
            .with_children(children),
            AccessibilityNode::new(
                self.query_id.clone(),
                AccessibilityRole::TextField,
                self.layout.query,
            )
            .with_label("Find")
            .with_value(self.state.query.clone())
            .with_focused(self.state.active_field == FindField::Query)
            .with_editable(true),
            AccessibilityNode::new(
                self.previous_id.clone(),
                AccessibilityRole::Button,
                self.layout.previous,
            )
            .with_label("Previous match"),
            AccessibilityNode::new(
                self.next_id.clone(),
                AccessibilityRole::Button,
                self.layout.next,
            )
            .with_label("Next match"),
            AccessibilityNode::new(
                self.close_id.clone(),
                AccessibilityRole::Button,
                self.layout.close,
            )
            .with_label("Close find panel"),
            AccessibilityNode::new(
                self.status_id.clone(),
                AccessibilityRole::Status,
                self.layout.status,
            )
            .with_label("Find results")
            .with_value(if self.state.match_count == 0 {
                if self.state.query.is_empty() {
                    "No query".to_owned()
                } else {
                    "No matches".to_owned()
                }
            } else {
                format!(
                    "{} of {}",
                    self.state.selected_match, self.state.match_count
                )
            }),
        ];
        if self.state.replacement_is_visible {
            nodes.push(
                AccessibilityNode::new(
                    self.replacement_id.clone(),
                    AccessibilityRole::TextField,
                    self.layout.replacement,
                )
                .with_label("Replace")
                .with_value(self.state.replacement.clone())
                .with_focused(self.state.active_field == FindField::Replacement)
                .with_editable(true),
            );
        }
        nodes
    }

    fn hit_test(&self, point: PointI) -> Option<NodeId> {
        if self.layout.query.contains(point) {
            Some(self.query_id.clone())
        } else if self.state.replacement_is_visible && self.layout.replacement.contains(point) {
            Some(self.replacement_id.clone())
        } else if self.layout.previous.contains(point) {
            Some(self.previous_id.clone())
        } else if self.layout.next.contains(point) {
            Some(self.next_id.clone())
        } else if self.layout.close.contains(point) {
            Some(self.close_id.clone())
        } else {
            self.layout.panel.contains(point).then_some(self.id.clone())
        }
    }
}

fn calculate_find_layout(bounds: RectI, replacement_is_visible: bool) -> FindPanelLayout {
    let panel_width = 520_u32.min(bounds.width.saturating_sub(24)).max(1);
    let panel_height = if replacement_is_visible { 108 } else { 66 };
    let panel_x = i32::try_from(bounds.right().saturating_sub(i64::from(panel_width + 12)))
        .unwrap_or(bounds.x);
    let panel = RectI::new(
        panel_x,
        bounds.y.saturating_add(12),
        panel_width,
        panel_height.min(bounds.height.saturating_sub(24).max(1)),
    );
    let button_width = 34_u32.min(panel.width / 5);
    let close = RectI::new(
        i32::try_from(panel.right().saturating_sub(i64::from(button_width + 8))).unwrap_or(panel.x),
        panel.y.saturating_add(8),
        button_width,
        32_u32.min(panel.height),
    );
    let next = RectI::new(
        close
            .x
            .saturating_sub(i32::try_from(button_width + 6).unwrap_or(i32::MAX)),
        close.y,
        button_width,
        close.height,
    );
    let previous = RectI::new(
        next.x
            .saturating_sub(i32::try_from(button_width + 6).unwrap_or(i32::MAX)),
        next.y,
        button_width,
        next.height,
    );
    let query = RectI::new(
        panel.x.saturating_add(8),
        panel.y.saturating_add(8),
        u32::try_from(i64::from(previous.x).saturating_sub(i64::from(panel.x) + 14)).unwrap_or(0),
        previous.height,
    );
    let replacement = RectI::new(
        query.x,
        query.y.saturating_add(42),
        query.width,
        query.height,
    );
    let status = RectI::new(
        previous.x,
        previous.y.saturating_add(34),
        panel.width / 3,
        24,
    );
    FindPanelLayout {
        panel,
        query,
        replacement,
        previous,
        next,
        close,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CommandPalette, CommandPaletteState, FindField, FindPanel, FindPanelState, PaletteItem,
    };
    use crate::Widget;
    use luna_accessibility::AccessibilityRole;
    use luna_core::{NodeId, RectI};
    use luna_theme::Theme;
    use std::error::Error;

    #[test]
    fn command_filter_and_selection_are_deterministic() {
        let mut state = CommandPaletteState {
            query: "file".to_owned(),
            items: vec![
                PaletteItem::new("new-file", "New File", "Ctrl+N"),
                PaletteItem::new("toggle-sidebar", "Toggle Sidebar", "Ctrl+B"),
            ],
            selected_index: 9,
        };
        state.normalize_selection();
        assert_eq!(state.filtered_items().len(), 1);
        assert_eq!(
            state.selected_item().map(|item| item.id.as_str()),
            Some("new-file")
        );
    }

    #[test]
    fn palette_semantic_nodes_resolve_back_to_commands() -> Result<(), Box<dyn Error>> {
        let palette = CommandPalette::new(
            NodeId::new("palette")?,
            RectI::new(0, 0, 800, 600),
            Theme::luna_dark(),
            CommandPaletteState {
                query: String::new(),
                items: vec![PaletteItem::new("save", "Save", "Ctrl+S")],
                selected_index: 0,
            },
        )?;
        let row = &palette.layout().rows[0];
        assert_eq!(palette.command_for_node(&row.node_id), Some("save"));
        Ok(())
    }

    #[test]
    fn find_panel_defaults_to_query_field() {
        assert_eq!(FindPanelState::default().active_field, FindField::Query);
    }

    #[test]
    fn find_panel_exposes_match_status_semantics() -> Result<(), Box<dyn Error>> {
        let panel = FindPanel::new(
            NodeId::new("find")?,
            RectI::new(0, 0, 800, 600),
            Theme::luna_dark(),
            FindPanelState {
                query: "Luna".to_owned(),
                match_count: 3,
                selected_match: 2,
                ..FindPanelState::default()
            },
        )?;
        let status = panel
            .accessibility_nodes()
            .into_iter()
            .find(|node| node.role == AccessibilityRole::Status)
            .ok_or_else(|| std::io::Error::other("find status node missing"))?;
        assert_eq!(status.value.as_deref(), Some("2 of 3"));
        Ok(())
    }
}
