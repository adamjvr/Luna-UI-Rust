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
    /// Optional case-insensitive desktop mnemonic.
    pub mnemonic: Option<char>,
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
            mnemonic: None,
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

    /// Assigns a desktop mnemonic using builder syntax.
    #[must_use]
    pub fn with_mnemonic(mut self, mnemonic: char) -> Self {
        self.mnemonic = Some(mnemonic.to_ascii_lowercase());
        self
    }
}

/// One command, nested submenu, or visual separator inside a dropdown menu.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MenuItem {
    /// Activatable or disabled command row.
    Command(MenuCommand),
    /// Nested desktop submenu.
    Submenu(Box<MenuDefinition>),
    /// Non-interactive visual grouping separator.
    Separator,
}

impl MenuItem {
    /// Creates a command item.
    #[must_use]
    pub fn command(command: MenuCommand) -> Self {
        Self::Command(command)
    }

    /// Creates a nested submenu item.
    #[must_use]
    pub fn submenu(definition: MenuDefinition) -> Self {
        Self::Submenu(Box::new(definition))
    }

    /// Returns the command payload when this is a command row.
    #[must_use]
    pub const fn as_command(&self) -> Option<&MenuCommand> {
        match self {
            Self::Command(command) => Some(command),
            Self::Submenu(_) | Self::Separator => None,
        }
    }

    /// Returns the nested menu definition when this is a submenu row.
    #[must_use]
    pub fn as_submenu(&self) -> Option<&MenuDefinition> {
        match self {
            Self::Submenu(definition) => Some(definition),
            Self::Command(_) | Self::Separator => None,
        }
    }

    fn is_selectable(&self) -> bool {
        match self {
            Self::Command(command) => command.is_enabled,
            Self::Submenu(definition) => definition.first_enabled_index().is_some(),
            Self::Separator => false,
        }
    }
}

/// One top-level or nested dropdown menu definition supplied by an application command registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuDefinition {
    /// Stable menu ID shared with the editor-shell menu heading.
    pub id: String,
    /// Visible menu title.
    pub title: String,
    /// Optional case-insensitive desktop mnemonic.
    pub mnemonic: Option<char>,
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
            mnemonic: None,
            items,
        }
    }

    /// Assigns a desktop mnemonic using builder syntax.
    #[must_use]
    pub fn with_mnemonic(mut self, mnemonic: char) -> Self {
        self.mnemonic = Some(mnemonic.to_ascii_lowercase());
        self
    }

    fn first_enabled_index(&self) -> Option<usize> {
        self.items.iter().position(MenuItem::is_selectable)
    }

    fn last_enabled_index(&self) -> Option<usize> {
        self.items.iter().rposition(MenuItem::is_selectable)
    }

    fn enabled_index_after(&self, current: usize) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        for offset in 1..=self.items.len() {
            let index = current.saturating_add(offset) % self.items.len();
            if self.items[index].is_selectable() {
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
            if self.items[wrapped].is_selectable() {
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
    /// Selected item index within the active top-level definition.
    pub selected_index: usize,
    /// Open first-level submenu item index.
    pub active_submenu_index: Option<usize>,
    /// Selected item index inside the open submenu.
    pub submenu_selected_index: usize,
}

impl DropdownMenuState {
    /// Returns whether a dropdown menu is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.active_menu_id.is_some()
    }

    /// Returns whether a nested submenu is open.
    #[must_use]
    pub const fn submenu_is_open(&self) -> bool {
        self.active_submenu_index.is_some()
    }

    /// Opens the supplied menu and selects its first enabled row.
    pub fn open(&mut self, definition: &MenuDefinition) {
        self.active_menu_id = Some(definition.id.clone());
        self.selected_index = definition.first_enabled_index().unwrap_or(0);
        self.active_submenu_index = None;
        self.submenu_selected_index = 0;
    }

    /// Closes any active dropdown.
    pub fn close(&mut self) {
        self.active_menu_id = None;
        self.selected_index = 0;
        self.active_submenu_index = None;
        self.submenu_selected_index = 0;
    }

    /// Selects the next enabled row, wrapping around separators and disabled rows.
    pub fn select_next(&mut self, definition: &MenuDefinition) {
        match self.active_submenu(definition) {
            Some(submenu) => {
                if let Some(index) = submenu.enabled_index_after(self.submenu_selected_index) {
                    self.submenu_selected_index = index;
                }
            }
            None => {
                if let Some(index) = definition.enabled_index_after(self.selected_index) {
                    self.selected_index = index;
                    self.active_submenu_index = None;
                }
            }
        }
    }

    /// Selects the previous enabled row, wrapping around separators and disabled rows.
    pub fn select_previous(&mut self, definition: &MenuDefinition) {
        match self.active_submenu(definition) {
            Some(submenu) => {
                if let Some(index) = submenu.enabled_index_before(self.submenu_selected_index) {
                    self.submenu_selected_index = index;
                }
            }
            None => {
                if let Some(index) = definition.enabled_index_before(self.selected_index) {
                    self.selected_index = index;
                    self.active_submenu_index = None;
                }
            }
        }
    }

    /// Selects the first enabled command in the current menu level.
    pub fn select_first(&mut self, definition: &MenuDefinition) {
        match self.active_submenu(definition) {
            Some(submenu) => {
                if let Some(index) = submenu.first_enabled_index() {
                    self.submenu_selected_index = index;
                }
            }
            None => {
                if let Some(index) = definition.first_enabled_index() {
                    self.selected_index = index;
                }
            }
        }
    }

    /// Selects the last enabled command in the current menu level.
    pub fn select_last(&mut self, definition: &MenuDefinition) {
        match self.active_submenu(definition) {
            Some(submenu) => {
                if let Some(index) = submenu.last_enabled_index() {
                    self.submenu_selected_index = index;
                }
            }
            None => {
                if let Some(index) = definition.last_enabled_index() {
                    self.selected_index = index;
                }
            }
        }
    }

    /// Opens the currently selected nested submenu.
    pub fn open_selected_submenu(&mut self, definition: &MenuDefinition) -> bool {
        let Some(MenuItem::Submenu(submenu)) = definition.items.get(self.selected_index) else {
            return false;
        };
        self.active_submenu_index = Some(self.selected_index);
        self.submenu_selected_index = submenu.first_enabled_index().unwrap_or(0);
        true
    }

    /// Closes the active nested submenu while retaining its parent selection.
    pub fn close_submenu(&mut self) -> bool {
        let was_open = self.active_submenu_index.take().is_some();
        self.submenu_selected_index = 0;
        was_open
    }

    /// Selects a pointer-hovered row and reports whether interaction state changed.
    pub fn select_hovered_path(
        &mut self,
        definition: &MenuDefinition,
        menu_depth: usize,
        index: usize,
    ) -> bool {
        if menu_depth == 0 {
            let Some(item) = definition.items.get(index) else {
                return false;
            };
            if !item.is_selectable() {
                return false;
            }
            let changed = self.selected_index != index;
            self.selected_index = index;
            match item {
                MenuItem::Submenu(submenu) => {
                    let submenu_changed = self.active_submenu_index != Some(index);
                    self.active_submenu_index = Some(index);
                    self.submenu_selected_index = submenu.first_enabled_index().unwrap_or(0);
                    changed || submenu_changed
                }
                MenuItem::Command(_) | MenuItem::Separator => {
                    let submenu_changed = self.active_submenu_index.take().is_some();
                    self.submenu_selected_index = 0;
                    changed || submenu_changed
                }
            }
        } else {
            let Some(submenu) = self.active_submenu(definition) else {
                return false;
            };
            if !submenu
                .items
                .get(index)
                .is_some_and(MenuItem::is_selectable)
            {
                return false;
            }
            if self.submenu_selected_index == index {
                return false;
            }
            self.submenu_selected_index = index;
            true
        }
    }

    /// Backward-compatible selection helper for a top-level hovered row.
    pub fn select_hovered(&mut self, definition: &MenuDefinition, index: usize) -> bool {
        self.select_hovered_path(definition, 0, index)
    }

    /// Returns the selected enabled command ID across the active menu level.
    #[must_use]
    pub fn selected_command<'a>(&self, definition: &'a MenuDefinition) -> Option<&'a str> {
        if let Some(submenu) = self.active_submenu(definition) {
            return submenu
                .items
                .get(self.submenu_selected_index)
                .and_then(MenuItem::as_command)
                .filter(|command| command.is_enabled)
                .map(|command| command.id.as_str());
        }
        definition
            .items
            .get(self.selected_index)
            .and_then(MenuItem::as_command)
            .filter(|command| command.is_enabled)
            .map(|command| command.id.as_str())
    }

    /// Activates a case-insensitive mnemonic, returning a command when one is selected.
    pub fn activate_mnemonic(
        &mut self,
        definition: &MenuDefinition,
        mnemonic: char,
    ) -> Option<String> {
        let mnemonic = mnemonic.to_ascii_lowercase();
        if let Some(submenu) = self.active_submenu(definition) {
            let index = submenu.items.iter().position(|item| match item {
                MenuItem::Command(command) => {
                    command.is_enabled && command.mnemonic == Some(mnemonic)
                }
                MenuItem::Submenu(definition) => definition.mnemonic == Some(mnemonic),
                MenuItem::Separator => false,
            })?;
            self.submenu_selected_index = index;
            return submenu
                .items
                .get(index)
                .and_then(MenuItem::as_command)
                .map(|command| command.id.clone());
        }
        let index = definition.items.iter().position(|item| match item {
            MenuItem::Command(command) => command.is_enabled && command.mnemonic == Some(mnemonic),
            MenuItem::Submenu(definition) => definition.mnemonic == Some(mnemonic),
            MenuItem::Separator => false,
        })?;
        self.selected_index = index;
        match definition.items.get(index) {
            Some(MenuItem::Command(command)) => Some(command.id.clone()),
            Some(MenuItem::Submenu(submenu)) => {
                self.active_submenu_index = Some(index);
                self.submenu_selected_index = submenu.first_enabled_index().unwrap_or(0);
                None
            }
            Some(MenuItem::Separator) | None => None,
        }
    }

    fn active_submenu<'a>(&self, definition: &'a MenuDefinition) -> Option<&'a MenuDefinition> {
        definition
            .items
            .get(self.active_submenu_index?)
            .and_then(MenuItem::as_submenu)
    }
}

/// Geometry and semantic state for one dropdown row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DropdownMenuRowFrame {
    /// Menu nesting depth, where zero is the top-level panel.
    pub menu_depth: usize,
    /// Source item index in its menu definition.
    pub item_index: usize,
    /// Stable command ID, absent for separators and submenu rows.
    pub command_id: Option<String>,
    /// Stable submenu ID, absent for commands and separators.
    pub submenu_id: Option<String>,
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
    /// Whether this row has a nested submenu.
    pub has_submenu: bool,
    /// Whether this command can execute.
    pub is_enabled: bool,
    /// Whether this command represents an active toggle or mode.
    pub is_checked: bool,
    /// Whether this row owns current keyboard selection.
    pub is_selected: bool,
}

/// Complete geometry for one open dropdown menu and its optional nested panel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DropdownMenuLayout {
    /// Primary elevated panel bounds.
    pub panel: RectI,
    /// All visible panel bounds in parent-to-child order.
    pub panels: Vec<RectI>,
    /// Ordered visible row frames across all panels.
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
    /// Creates a dropdown with no nested submenu open.
    pub fn new(
        id: NodeId,
        viewport: RectI,
        anchor: RectI,
        theme: Theme,
        definition: MenuDefinition,
        selected_index: usize,
    ) -> Result<Self, NodeIdError> {
        let state = DropdownMenuState {
            active_menu_id: Some(definition.id.clone()),
            selected_index,
            active_submenu_index: None,
            submenu_selected_index: 0,
        };
        Self::new_with_state(id, viewport, anchor, theme, definition, &state)
    }

    /// Creates a dropdown and optional first-level submenu from full interaction state.
    pub fn new_with_state(
        id: NodeId,
        viewport: RectI,
        anchor: RectI,
        theme: Theme,
        definition: MenuDefinition,
        state: &DropdownMenuState,
    ) -> Result<Self, NodeIdError> {
        let layout = calculate_layout(&id, viewport, anchor, &definition, state)?;
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

    /// Returns menu depth and source item index under a pointer position.
    #[must_use]
    pub fn menu_item_at(&self, point: PointI) -> Option<(usize, usize)> {
        self.layout
            .rows
            .iter()
            .find(|row| row.bounds.contains(point))
            .map(|row| (row.menu_depth, row.item_index))
    }

    /// Returns the top-level source item index under a pointer position.
    #[must_use]
    pub fn item_index_at(&self, point: PointI) -> Option<usize> {
        self.layout
            .rows
            .iter()
            .find(|row| row.menu_depth == 0 && row.bounds.contains(point))
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

    /// Returns whether the point lies inside any open dropdown panel.
    #[must_use]
    pub fn contains(&self, point: PointI) -> bool {
        self.layout.panels.iter().any(|panel| panel.contains(point))
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
        for panel in &self.layout.panels {
            let shadow = RectI::new(
                panel.x.saturating_add(4),
                panel.y.saturating_add(4),
                panel.width,
                panel.height,
            );
            display_list.fill_rect(shadow, self.theme.background.with_alpha(144));
            display_list.fill_rect(*panel, self.theme.border());
            display_list.fill_rect(
                RectI::new(
                    panel.x.saturating_add(1),
                    panel.y.saturating_add(1),
                    panel.width.saturating_sub(2),
                    panel.height.saturating_sub(2),
                ),
                self.theme.panel,
            );
        }
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
            if row.has_submenu {
                display_list.fill_rect(
                    RectI::new(
                        i32::try_from(row.bounds.right().saturating_sub(12))
                            .unwrap_or(row.bounds.x),
                        row.bounds.y.saturating_add(12),
                        5,
                        5,
                    ),
                    self.theme.foreground,
                );
            }
        }
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        let children = self
            .layout
            .rows
            .iter()
            .filter_map(|row| row.node_id.clone())
            .collect::<Vec<_>>();
        let mut nodes = vec![
            AccessibilityNode::new(self.id.clone(), AccessibilityRole::Menu, self.layout.panel)
                .with_label(format!("{} menu", self.definition.title))
                .with_children(children),
        ];
        for row in &self.layout.rows {
            let Some(node_id) = row.node_id.clone() else {
                continue;
            };
            let value = match (row.is_checked, row.shortcut.is_empty(), row.has_submenu) {
                (_, _, true) => "Submenu".to_owned(),
                (true, true, false) => "Checked".to_owned(),
                (true, false, false) => format!("Checked, {}", row.shortcut),
                (false, true, false) => String::new(),
                (false, false, false) => row.shortcut.clone(),
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
            .or_else(|| self.contains(point).then_some(self.id.clone()))
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
    state: &DropdownMenuState,
) -> Result<DropdownMenuLayout, NodeIdError> {
    let panel = panel_bounds(viewport, anchor, definition, false);
    let mut panels = vec![panel];
    let mut rows = panel_rows(id, panel, definition, 0, state.selected_index)?;
    if let Some(submenu_index) = state.active_submenu_index
        && let Some(submenu) = definition
            .items
            .get(submenu_index)
            .and_then(MenuItem::as_submenu)
        && let Some(parent_row) = rows
            .iter()
            .find(|row| row.menu_depth == 0 && row.item_index == submenu_index)
    {
        let submenu_anchor = RectI::new(
            i32::try_from(parent_row.bounds.right()).unwrap_or(parent_row.bounds.x),
            parent_row.bounds.y,
            1,
            parent_row.bounds.height,
        );
        let submenu_panel = panel_bounds(viewport, submenu_anchor, submenu, true);
        panels.push(submenu_panel);
        rows.extend(panel_rows(
            id,
            submenu_panel,
            submenu,
            1,
            state.submenu_selected_index,
        )?);
    }
    Ok(DropdownMenuLayout {
        panel,
        panels,
        rows,
    })
}

fn panel_bounds(
    viewport: RectI,
    anchor: RectI,
    definition: &MenuDefinition,
    submenu: bool,
) -> RectI {
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
                    MenuItem::Command(_) | MenuItem::Submenu(_) => COMMAND_ROW_HEIGHT,
                    MenuItem::Separator => SEPARATOR_HEIGHT,
                })
            });
    let panel_height = desired_height.min(viewport.height.saturating_sub(8).max(1));
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
    let preferred_x = if submenu {
        anchor.x
    } else {
        anchor.x.clamp(minimum_x, maximum_x)
    };
    let panel_x = if submenu && preferred_x > maximum_x {
        anchor
            .x
            .saturating_sub(i32::try_from(panel_width.saturating_add(2)).unwrap_or(i32::MAX))
            .clamp(minimum_x, maximum_x)
    } else {
        preferred_x.clamp(minimum_x, maximum_x)
    };
    let preferred_y = if submenu {
        anchor
            .y
            .saturating_sub(i32::try_from(PANEL_PADDING).unwrap_or(0))
    } else {
        i32::try_from(anchor.bottom()).unwrap_or(i32::MAX)
    };
    let maximum_y = i32::try_from(
        viewport
            .bottom()
            .saturating_sub(i64::from(panel_height))
            .saturating_sub(4),
    )
    .unwrap_or(viewport.y)
    .max(viewport.y);
    RectI::new(
        panel_x,
        preferred_y.clamp(viewport.y.saturating_add(4), maximum_y),
        panel_width,
        panel_height,
    )
}

fn panel_rows(
    id: &NodeId,
    panel: RectI,
    definition: &MenuDefinition,
    menu_depth: usize,
    selected_index: usize,
) -> Result<Vec<DropdownMenuRowFrame>, NodeIdError> {
    let mut rows = Vec::new();
    let mut y = panel
        .y
        .saturating_add(i32::try_from(PANEL_PADDING).unwrap_or(0));
    let content_bottom = panel.bottom().saturating_sub(i64::from(PANEL_PADDING));
    for (item_index, item) in definition.items.iter().enumerate() {
        let desired_row_height = match item {
            MenuItem::Command(_) | MenuItem::Submenu(_) => COMMAND_ROW_HEIGHT,
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
        let prefix = format!("depth-{menu_depth}");
        match item {
            MenuItem::Command(command) => rows.push(DropdownMenuRowFrame {
                menu_depth,
                item_index,
                command_id: Some(command.id.clone()),
                submenu_id: None,
                node_id: Some(
                    id.child(&prefix)?
                        .child(&format!("command-{}", command.id))?,
                ),
                title: command.title.clone(),
                shortcut: command.shortcut.clone(),
                bounds,
                is_separator: false,
                has_submenu: false,
                is_enabled: command.is_enabled,
                is_checked: command.is_checked,
                is_selected: item_index == selected_index,
            }),
            MenuItem::Submenu(submenu) => rows.push(DropdownMenuRowFrame {
                menu_depth,
                item_index,
                command_id: None,
                submenu_id: Some(submenu.id.clone()),
                node_id: Some(
                    id.child(&prefix)?
                        .child(&format!("submenu-{}", submenu.id))?,
                ),
                title: submenu.title.clone(),
                shortcut: String::new(),
                bounds,
                is_separator: false,
                has_submenu: true,
                is_enabled: submenu.first_enabled_index().is_some(),
                is_checked: false,
                is_selected: item_index == selected_index,
            }),
            MenuItem::Separator => rows.push(DropdownMenuRowFrame {
                menu_depth,
                item_index,
                command_id: None,
                submenu_id: None,
                node_id: None,
                title: String::new(),
                shortcut: String::new(),
                bounds,
                is_separator: true,
                has_submenu: false,
                is_enabled: false,
                is_checked: false,
                is_selected: false,
            }),
        }
        y = y.saturating_add(i32::try_from(row_height).unwrap_or(i32::MAX));
    }
    Ok(rows)
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
                MenuItem::command(
                    MenuCommand::new("new-file", "New File", "Ctrl+N").with_mnemonic('n'),
                ),
                MenuItem::command(
                    MenuCommand::new("open", "Open…", "Ctrl+O")
                        .with_enabled(false)
                        .with_mnemonic('o'),
                ),
                MenuItem::submenu(
                    MenuDefinition::new(
                        "recent",
                        "Open Recent",
                        vec![MenuItem::command(
                            MenuCommand::new("recent-one", "one.txt", "").with_mnemonic('1'),
                        )],
                    )
                    .with_mnemonic('r'),
                ),
                MenuItem::Separator,
                MenuItem::command(MenuCommand::new("exit", "Exit", "").with_mnemonic('x')),
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
        assert!(state.open_selected_submenu(&definition));
        assert_eq!(state.selected_command(&definition), Some("recent-one"));
        assert!(state.close_submenu());
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
    fn nested_menu_projects_second_panel_and_resolves_command() -> Result<(), Box<dyn Error>> {
        let definition = file_menu();
        let mut state = DropdownMenuState::default();
        state.open(&definition);
        state.selected_index = 2;
        assert!(state.open_selected_submenu(&definition));
        let menu = DropdownMenu::new_with_state(
            NodeId::new("dropdown")?,
            RectI::new(0, 0, 800, 500),
            RectI::new(20, 0, 40, 28),
            Theme::luna_dark(),
            definition,
            &state,
        )?;
        assert_eq!(menu.layout().panels.len(), 2);
        let command_row = menu
            .layout()
            .rows
            .iter()
            .find(|row| row.command_id.as_deref() == Some("recent-one"))
            .ok_or("submenu row missing")?;
        assert_eq!(
            menu.command_at(PointI::new(
                command_row.bounds.x + 1,
                command_row.bounds.y + 1
            )),
            Some("recent-one")
        );
        Ok(())
    }

    #[test]
    fn mnemonics_open_submenus_and_execute_commands() {
        let definition = file_menu();
        let mut state = DropdownMenuState::default();
        state.open(&definition);
        assert_eq!(state.activate_mnemonic(&definition, 'r'), None);
        assert!(state.submenu_is_open());
        assert_eq!(
            state.activate_mnemonic(&definition, '1').as_deref(),
            Some("recent-one")
        );
    }

    #[test]
    fn dropdown_accessibility_exposes_menu_and_rows() -> Result<(), Box<dyn Error>> {
        let menu = DropdownMenu::new(
            NodeId::new("dropdown")?,
            RectI::new(0, 0, 600, 400),
            RectI::new(10, 0, 40, 28),
            Theme::luna_dark(),
            file_menu(),
            0,
        )?;
        let nodes = menu.accessibility_nodes();
        assert!(
            nodes
                .iter()
                .any(|node| node.role == AccessibilityRole::Menu)
        );
        assert!(
            nodes
                .iter()
                .any(|node| node.role == AccessibilityRole::MenuItem)
        );
        Ok(())
    }
}
