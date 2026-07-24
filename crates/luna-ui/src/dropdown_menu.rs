// SPDX-License-Identifier: MPL-2.0

use crate::Widget;
use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{NodeId, NodeIdError, PointI, RectI};
use luna_render::DisplayList;
use luna_theme::Theme;

const PANEL_PADDING: u32 = 4;
const COMMAND_ROW_HEIGHT: u32 = 28;
const SEPARATOR_HEIGHT: u32 = 9;
const PREFERRED_PANEL_WIDTH: u32 = 252;

/// One command projected into a dropdown menu and other command surfaces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuCommand {
    /// Stable application command ID.
    pub id: String,
    /// Visible menu-row title.
    pub title: String,
    /// Optional platform-neutral shortcut description.
    pub shortcut: String,
    /// Whether the command can currently execute.
    pub is_enabled: bool,
    /// Whether the command represents an active toggle or mode.
    pub is_checked: bool,
}

impl MenuCommand {
    /// Creates an enabled unchecked command.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        shortcut: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            shortcut: shortcut.into(),
            is_enabled: true,
            is_checked: false,
        }
    }

    /// Replaces the enabled state using builder syntax.
    #[must_use]
    pub const fn with_enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    /// Replaces the checked state using builder syntax.
    #[must_use]
    pub const fn with_checked(mut self, is_checked: bool) -> Self {
        self.is_checked = is_checked;
        self
    }
}

/// One command or visual separator inside a dropdown menu.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MenuItem {
    /// Activatable or disabled command row.
    Command(MenuCommand),
    /// Non-interactive visual grouping separator.
    Separator,
}

impl MenuItem {
    /// Creates a command item.
    #[must_use]
    pub fn command(command: MenuCommand) -> Self {
        Self::Command(command)
    }

    /// Returns the command payload when this is a command row.
    #[must_use]
    pub const fn as_command(&self) -> Option<&MenuCommand> {
        match self {
            Self::Command(command) => Some(command),
            Self::Separator => None,
        }
    }
}

/// One top-level dropdown menu definition supplied by an application command registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuDefinition {
    /// Stable menu ID shared with the editor-shell menu heading.
    pub id: String,
    /// Visible top-level title.
    pub title: String,
    /// Ordered menu contents.
    pub items: Vec<MenuItem>,
}

impl MenuDefinition {
    /// Creates a dropdown menu definition.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            items,
        }
    }

    fn first_enabled_index(&self) -> Option<usize> {
        self.items
            .iter()
            .position(|item| item.as_command().is_some_and(|command| command.is_enabled))
    }

    fn last_enabled_index(&self) -> Option<usize> {
        self.items
            .iter()
            .rposition(|item| item.as_command().is_some_and(|command| command.is_enabled))
    }

    fn enabled_index_after(&self, current: usize) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        for offset in 1..=self.items.len() {
            let index = current.saturating_add(offset) % self.items.len();
            if self.items[index]
                .as_command()
                .is_some_and(|command| command.is_enabled)
            {
                return Some(index);
            }
        }
        None
    }

    fn enabled_index_before(&self, current: usize) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        for offset in 1..=self.items.len() {
            let wrapped = current
                .saturating_add(self.items.len())
                .saturating_sub(offset)
                % self.items.len();
            if self.items[wrapped]
                .as_command()
                .is_some_and(|command| command.is_enabled)
            {
                return Some(wrapped);
            }
        }
        None
    }
}

/// Application-owned dropdown-menu interaction state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DropdownMenuState {
    /// Active top-level menu ID, or `None` when all menus are closed.
    pub active_menu_id: Option<String>,
    /// Selected item index within the active menu definition.
    pub selected_index: usize,
}

impl DropdownMenuState {
    /// Returns whether a dropdown menu is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.active_menu_id.is_some()
    }

    /// Opens the supplied menu and selects its first enabled command.
    pub fn open(&mut self, definition: &MenuDefinition) {
        self.active_menu_id = Some(definition.id.clone());
        self.selected_index = definition.first_enabled_index().unwrap_or(0);
    }

    /// Closes any active dropdown.
    pub fn close(&mut self) {
        self.active_menu_id = None;
        self.selected_index = 0;
    }

    /// Selects the next enabled command, wrapping around separators and disabled rows.
    pub fn select_next(&mut self, definition: &MenuDefinition) {
        if let Some(index) = definition.enabled_index_after(self.selected_index) {
            self.selected_index = index;
        }
    }

    /// Selects the previous enabled command, wrapping around separators and disabled rows.
    pub fn select_previous(&mut self, definition: &MenuDefinition) {
        if let Some(index) = definition.enabled_index_before(self.selected_index) {
            self.selected_index = index;
        }
    }

    /// Selects the first enabled command.
    pub fn select_first(&mut self, definition: &MenuDefinition) {
        if let Some(index) = definition.first_enabled_index() {
            self.selected_index = index;
        }
    }

    /// Selects the last enabled command.
    pub fn select_last(&mut self, definition: &MenuDefinition) {
        if let Some(index) = definition.last_enabled_index() {
            self.selected_index = index;
        }
    }

    /// Selects a pointer-hovered enabled row and reports whether selection changed.
    pub fn select_hovered(&mut self, definition: &MenuDefinition, index: usize) -> bool {
        let selectable = definition
            .items
            .get(index)
            .and_then(MenuItem::as_command)
            .is_some_and(|command| command.is_enabled);
        if !selectable || self.selected_index == index {
            return false;
        }
        self.selected_index = index;
        true
    }

    /// Returns the selected enabled command ID.
    #[must_use]
    pub fn selected_command<'a>(&self, definition: &'a MenuDefinition) -> Option<&'a str> {
        definition
            .items
            .get(self.selected_index)
            .and_then(MenuItem::as_command)
            .filter(|command| command.is_enabled)
            .map(|command| command.id.as_str())
    }
}

/// Geometry and semantic state for one dropdown row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DropdownMenuRowFrame {
    /// Source item index in the menu definition.
    pub item_index: usize,
    /// Stable command ID, absent for separators.
    pub command_id: Option<String>,
    /// Stable semantic node ID, absent for separators.
    pub node_id: Option<NodeId>,
    /// Visible row title.
    pub title: String,
    /// Visible shortcut description.
    pub shortcut: String,
    /// Shared paint, hit-test, and accessibility bounds.
    pub bounds: RectI,
    /// Whether this row is a separator.
    pub is_separator: bool,
    /// Whether this command can execute.
    pub is_enabled: bool,
    /// Whether this command represents an active toggle or mode.
    pub is_checked: bool,
    /// Whether this row owns current keyboard selection.
    pub is_selected: bool,
}

/// Complete geometry for one open dropdown menu.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DropdownMenuLayout {
    /// Elevated panel bounds.
    pub panel: RectI,
    /// Ordered visible row frames.
    pub rows: Vec<DropdownMenuRowFrame>,
}

/// Reusable product-neutral desktop dropdown-menu widget.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DropdownMenu {
    id: NodeId,
    bounds: RectI,
    theme: Theme,
    definition: MenuDefinition,
    layout: DropdownMenuLayout,
}

impl DropdownMenu {
    /// Creates a dropdown anchored below a top-level menu heading and clamped to the viewport.
    pub fn new(
        id: NodeId,
        viewport: RectI,
        anchor: RectI,
        theme: Theme,
        definition: MenuDefinition,
        selected_index: usize,
    ) -> Result<Self, NodeIdError> {
        let layout = calculate_layout(&id, viewport, anchor, &definition, selected_index)?;
        Ok(Self {
            id,
            bounds: viewport,
            theme,
            definition,
            layout,
        })
    }

    /// Returns the immutable dropdown geometry.
    #[must_use]
    pub const fn layout(&self) -> &DropdownMenuLayout {
        &self.layout
    }

    /// Returns the active menu definition.
    #[must_use]
    pub const fn definition(&self) -> &MenuDefinition {
        &self.definition
    }

    /// Returns the source item index under a pointer position.
    #[must_use]
    pub fn item_index_at(&self, point: PointI) -> Option<usize> {
        self.layout
            .rows
            .iter()
            .find(|row| row.bounds.contains(point))
            .map(|row| row.item_index)
    }

    /// Returns an enabled command under a pointer position.
    #[must_use]
    pub fn command_at(&self, point: PointI) -> Option<&str> {
        self.layout
            .rows
            .iter()
            .find(|row| row.bounds.contains(point) && row.is_enabled)
            .and_then(|row| row.command_id.as_deref())
    }

    /// Returns an enabled command represented by an accessibility node.
    #[must_use]
    pub fn command_for_node(&self, node_id: &NodeId) -> Option<&str> {
        self.layout
            .rows
            .iter()
            .find(|row| row.node_id.as_ref() == Some(node_id) && row.is_enabled)
            .and_then(|row| row.command_id.as_deref())
    }

    /// Returns whether the point lies inside the dropdown panel.
    #[must_use]
    pub fn contains(&self, point: PointI) -> bool {
        self.layout.panel.contains(point)
    }
}

impl Widget for DropdownMenu {
    fn id(&self) -> &NodeId {
        &self.id
    }

    fn bounds(&self) -> RectI {
        self.bounds
    }

    fn build_display_list(&self, display_list: &mut DisplayList) {
        let panel = self.layout.panel;
        let shadow = RectI::new(
            panel.x.saturating_add(4),
            panel.y.saturating_add(4),
            panel.width,
            panel.height,
        );
        display_list.fill_rect(shadow, self.theme.background.with_alpha(144));
        display_list.fill_rect(panel, self.theme.border());
        display_list.fill_rect(
            RectI::new(
                panel.x.saturating_add(1),
                panel.y.saturating_add(1),
                panel.width.saturating_sub(2),
                panel.height.saturating_sub(2),
            ),
            self.theme.panel,
        );
        for row in &self.layout.rows {
            if row.is_separator {
                let y = row.y_center();
                display_list.fill_rect(
                    RectI::new(
                        row.bounds.x.saturating_add(8),
                        y,
                        row.bounds.width.saturating_sub(16),
                        1,
                    ),
                    self.theme.border(),
                );
                continue;
            }
            if row.is_selected && row.is_enabled {
                display_list.fill_rect(row.bounds, self.theme.selection());
            }
            if row.is_checked {
                display_list.fill_rect(
                    RectI::new(
                        row.bounds.x.saturating_add(8),
                        row.bounds.y.saturating_add(10),
                        8,
                        8,
                    ),
                    self.theme.accent,
                );
            }
        }
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        let children: Vec<NodeId> = self
            .layout
            .rows
            .iter()
            .filter_map(|row| row.node_id.clone())
            .collect();
        let mut nodes = vec![
            AccessibilityNode::new(self.id.clone(), AccessibilityRole::Menu, self.layout.panel)
                .with_label(format!("{} menu", self.definition.title))
                .with_children(children),
        ];
        for row in &self.layout.rows {
            let Some(node_id) = row.node_id.clone() else {
                continue;
            };
            let value = match (row.is_checked, row.shortcut.is_empty()) {
                (true, true) => "Checked".to_owned(),
                (true, false) => format!("Checked, {}", row.shortcut),
                (false, true) => String::new(),
                (false, false) => row.shortcut.clone(),
            };
            nodes.push(
                AccessibilityNode::new(node_id, AccessibilityRole::MenuItem, row.bounds)
                    .with_label(row.title.clone())
                    .with_value(value)
                    .with_disabled(!row.is_enabled)
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
            .and_then(|row| row.node_id.clone())
            .or_else(|| self.layout.panel.contains(point).then_some(self.id.clone()))
    }
}

impl DropdownMenuRowFrame {
    fn y_center(&self) -> i32 {
        self.bounds
            .y
            .saturating_add(i32::try_from(self.bounds.height / 2).unwrap_or(0))
    }
}

fn calculate_layout(
    id: &NodeId,
    viewport: RectI,
    anchor: RectI,
    definition: &MenuDefinition,
    selected_index: usize,
) -> Result<DropdownMenuLayout, NodeIdError> {
    let horizontal_margin = 4_u32.min(viewport.width / 2);
    let available_width = viewport
        .width
        .saturating_sub(horizontal_margin.saturating_mul(2))
        .max(1);
    let panel_width = PREFERRED_PANEL_WIDTH.min(available_width);
    let desired_height =
        definition
            .items
            .iter()
            .fold(PANEL_PADDING.saturating_mul(2), |height, item| {
                height.saturating_add(match item {
                    MenuItem::Command(_) => COMMAND_ROW_HEIGHT,
                    MenuItem::Separator => SEPARATOR_HEIGHT,
                })
            });
    let anchor_bottom = i32::try_from(anchor.bottom()).unwrap_or(i32::MAX);
    let viewport_bottom = i32::try_from(viewport.bottom()).unwrap_or(i32::MAX);
    let available_height =
        u32::try_from(viewport_bottom.saturating_sub(anchor_bottom)).unwrap_or(0);
    let panel_height = desired_height.min(available_height.max(1));
    let minimum_x = viewport
        .x
        .saturating_add(i32::try_from(horizontal_margin).unwrap_or(0));
    let maximum_x = i32::try_from(
        viewport
            .right()
            .saturating_sub(i64::from(panel_width))
            .saturating_sub(i64::from(horizontal_margin)),
    )
    .unwrap_or(minimum_x)
    .max(minimum_x);
    let panel_x = anchor.x.clamp(minimum_x, maximum_x);
    let maximum_y = viewport_bottom.saturating_sub(1).max(viewport.y);
    let panel_y = anchor_bottom.clamp(viewport.y, maximum_y);
    let panel = RectI::new(panel_x, panel_y, panel_width, panel_height);

    let mut rows = Vec::new();
    let mut y = panel
        .y
        .saturating_add(i32::try_from(PANEL_PADDING).unwrap_or(0));
    let content_bottom = panel.bottom().saturating_sub(i64::from(PANEL_PADDING));
    for (item_index, item) in definition.items.iter().enumerate() {
        let desired_row_height = match item {
            MenuItem::Command(_) => COMMAND_ROW_HEIGHT,
            MenuItem::Separator => SEPARATOR_HEIGHT,
        };
        let remaining = u32::try_from(content_bottom.saturating_sub(i64::from(y))).unwrap_or(0);
        let row_height = desired_row_height.min(remaining);
        if row_height == 0 {
            break;
        }
        let bounds = RectI::new(
            panel
                .x
                .saturating_add(i32::try_from(PANEL_PADDING).unwrap_or(0)),
            y,
            panel.width.saturating_sub(PANEL_PADDING.saturating_mul(2)),
            row_height,
        );
        match item {
            MenuItem::Command(command) => {
                rows.push(DropdownMenuRowFrame {
                    item_index,
                    command_id: Some(command.id.clone()),
                    node_id: Some(id.child(&format!("command-{}", command.id))?),
                    title: command.title.clone(),
                    shortcut: command.shortcut.clone(),
                    bounds,
                    is_separator: false,
                    is_enabled: command.is_enabled,
                    is_checked: command.is_checked,
                    is_selected: item_index == selected_index,
                });
            }
            MenuItem::Separator => rows.push(DropdownMenuRowFrame {
                item_index,
                command_id: None,
                node_id: None,
                title: String::new(),
                shortcut: String::new(),
                bounds,
                is_separator: true,
                is_enabled: false,
                is_checked: false,
                is_selected: false,
            }),
        }
        y = y.saturating_add(i32::try_from(row_height).unwrap_or(i32::MAX));
    }
    Ok(DropdownMenuLayout { panel, rows })
}

#[cfg(test)]
mod tests {
    use super::{DropdownMenu, DropdownMenuState, MenuCommand, MenuDefinition, MenuItem};
    use crate::Widget;
    use luna_accessibility::AccessibilityRole;
    use luna_core::{NodeId, PointI, RectI};
    use luna_theme::Theme;
    use std::error::Error;

    fn file_menu() -> MenuDefinition {
        MenuDefinition::new(
            "file",
            "File",
            vec![
                MenuItem::command(MenuCommand::new("new-file", "New File", "Ctrl+N")),
                MenuItem::command(MenuCommand::new("open", "Open…", "Ctrl+O").with_enabled(false)),
                MenuItem::Separator,
                MenuItem::command(MenuCommand::new("exit", "Exit", "")),
            ],
        )
    }

    #[test]
    fn keyboard_selection_skips_disabled_rows_and_separators() {
        let definition = file_menu();
        let mut state = DropdownMenuState::default();
        state.open(&definition);
        assert_eq!(state.selected_command(&definition), Some("new-file"));
        state.select_next(&definition);
        assert_eq!(state.selected_command(&definition), Some("exit"));
        state.select_previous(&definition);
        assert_eq!(state.selected_command(&definition), Some("new-file"));
    }

    #[test]
    fn dropdown_is_anchored_and_clamped_to_the_viewport() -> Result<(), Box<dyn Error>> {
        let menu = DropdownMenu::new(
            NodeId::new("dropdown")?,
            RectI::new(0, 0, 320, 240),
            RectI::new(280, 0, 40, 28),
            Theme::luna_dark(),
            file_menu(),
            0,
        )?;
        assert!(menu.layout().panel.x >= 0);
        assert!(menu.layout().panel.right() <= 320);
        assert_eq!(menu.layout().panel.y, 28);
        Ok(())
    }

    #[test]
    fn tiny_viewports_produce_bounded_nonempty_panels() -> Result<(), Box<dyn Error>> {
        let menu = DropdownMenu::new(
            NodeId::new("dropdown")?,
            RectI::new(5, 7, 1, 1),
            RectI::new(5, 7, 1, 1),
            Theme::luna_dark(),
            file_menu(),
            0,
        )?;

        assert_eq!(menu.layout().panel, RectI::new(5, 7, 1, 1));
        Ok(())
    }

    #[test]
    fn disabled_commands_do_not_activate() -> Result<(), Box<dyn Error>> {
        let menu = DropdownMenu::new(
            NodeId::new("dropdown")?,
            RectI::new(0, 0, 640, 480),
            RectI::new(8, 0, 64, 28),
            Theme::luna_dark(),
            file_menu(),
            1,
        )?;
        let disabled = menu
            .layout()
            .rows
            .iter()
            .find(|row| row.command_id.as_deref() == Some("open"))
            .ok_or_else(|| std::io::Error::other("open row missing"))?;
        assert_eq!(
            menu.command_at(PointI::new(disabled.bounds.x, disabled.bounds.y)),
            None
        );
        let semantics = menu.accessibility_nodes();
        assert!(semantics.iter().any(|node| {
            node.role == AccessibilityRole::MenuItem
                && node.label.as_deref() == Some("Open…")
                && node.is_disabled
        }));
        Ok(())
    }
}
