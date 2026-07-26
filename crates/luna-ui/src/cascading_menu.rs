// SPDX-License-Identifier: MPL-2.0

//! Arbitrary-depth cascading desktop-menu state and pointer-intent geometry.
//!
//! The existing dropdown widget remains a compatibility projection. This module supplies the
//! recursive state machine used by editor-class applications that need more than one submenu level.

use crate::{MenuDefinition, MenuItem};
use luna_core::{PointI, RectI};

const PANEL_WIDTH: u32 = 252;
const PANEL_PADDING: u32 = 4;
const ROW_HEIGHT: u32 = 28;
const SEPARATOR_HEIGHT: u32 = 9;

/// Row path from the root definition to one selected item.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MenuPath(Vec<usize>);

impl MenuPath {
    /// Creates a path from row indices at each open depth.
    #[must_use]
    pub fn new(indices: Vec<usize>) -> Self {
        Self(indices)
    }

    /// Returns row indices from root to deepest selection.
    #[must_use]
    pub fn indices(&self) -> &[usize] {
        &self.0
    }

    /// Returns the path depth.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.0.len()
    }

    fn truncate(&mut self, length: usize) {
        self.0.truncate(length);
    }

    fn push(&mut self, index: usize) {
        self.0.push(index);
    }
}

/// Deferred pointer selection used to avoid collapsing a submenu while moving toward its child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuHoverIntent {
    /// Candidate path under the pointer.
    pub candidate: MenuPath,
    /// Logical millisecond when the candidate first became hovered.
    pub started_at_millis: u64,
}

/// Application-owned recursive cascading-menu interaction state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CascadingMenuState {
    selected: MenuPath,
    open_depth: usize,
    hover_intent: Option<MenuHoverIntent>,
}

impl CascadingMenuState {
    /// Opens a menu and selects its first selectable root row.
    pub fn open(&mut self, definition: &MenuDefinition) {
        self.selected = MenuPath::default();
        if let Some(index) = first_selectable(definition) {
            self.selected.push(index);
            self.open_depth = 1;
        } else {
            self.open_depth = 0;
        }
        self.hover_intent = None;
    }

    /// Closes every panel and clears deferred pointer state.
    pub fn close(&mut self) {
        self.selected = MenuPath::default();
        self.open_depth = 0;
        self.hover_intent = None;
    }

    /// Returns whether at least the root panel is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open_depth > 0
    }

    /// Returns the selected row path.
    #[must_use]
    pub const fn selected_path(&self) -> &MenuPath {
        &self.selected
    }

    /// Returns the number of currently open menu panels.
    #[must_use]
    pub const fn open_depth(&self) -> usize {
        self.open_depth
    }

    /// Selects the next selectable row at the deepest open depth.
    pub fn select_next(&mut self, definition: &MenuDefinition) {
        self.move_selection(definition, 1);
    }

    /// Selects the previous selectable row at the deepest open depth.
    pub fn select_previous(&mut self, definition: &MenuDefinition) {
        self.move_selection(definition, -1);
    }

    /// Opens the selected submenu, if any, at arbitrary depth.
    pub fn open_selected_submenu(&mut self, definition: &MenuDefinition) -> bool {
        let Some(item) = item_at_path(definition, self.selected.indices()) else {
            return false;
        };
        let Some(submenu) = item.as_submenu() else {
            return false;
        };
        let Some(index) = first_selectable(submenu) else {
            return false;
        };
        self.selected.push(index);
        self.open_depth = self.selected.depth();
        self.hover_intent = None;
        true
    }

    /// Closes one child panel and returns selection to its parent row.
    pub fn close_child(&mut self) -> bool {
        if self.open_depth <= 1 || self.selected.depth() <= 1 {
            return false;
        }
        self.selected
            .truncate(self.selected.depth().saturating_sub(1));
        self.open_depth = self.selected.depth();
        self.hover_intent = None;
        true
    }

    /// Selects an exact hit-tested path and optionally opens its submenu.
    pub fn select_path(
        &mut self,
        definition: &MenuDefinition,
        path: MenuPath,
        open_submenu: bool,
    ) -> bool {
        let Some(item) = item_at_path(definition, path.indices()) else {
            return false;
        };
        if !is_selectable(item) {
            return false;
        }
        self.selected = path;
        self.open_depth = self.selected.depth();
        if open_submenu {
            let _ = self.open_selected_submenu(definition);
        }
        self.hover_intent = None;
        true
    }

    /// Begins or updates a delayed hover candidate.
    ///
    /// Returns `true` when the candidate matured and was selected. The application supplies logical
    /// time so replay tests remain deterministic.
    pub fn update_hover(
        &mut self,
        definition: &MenuDefinition,
        candidate: MenuPath,
        now_millis: u64,
        delay_millis: u64,
        preserve_open_child: bool,
    ) -> bool {
        if preserve_open_child {
            return false;
        }
        let same_candidate = self
            .hover_intent
            .as_ref()
            .is_some_and(|intent| intent.candidate == candidate);
        if !same_candidate {
            self.hover_intent = Some(MenuHoverIntent {
                candidate,
                started_at_millis: now_millis,
            });
            return false;
        }
        let matured = self.hover_intent.as_ref().is_some_and(|intent| {
            now_millis.saturating_sub(intent.started_at_millis) >= delay_millis
        });
        if !matured {
            return false;
        }
        let candidate = self.hover_intent.take().map(|intent| intent.candidate);
        candidate.is_some_and(|path| self.select_path(definition, path, true))
    }

    /// Returns the selected enabled command, when the deepest row is activatable.
    #[must_use]
    pub fn selected_command<'a>(
        &self,
        definition: &'a MenuDefinition,
    ) -> Option<&'a crate::MenuCommand> {
        item_at_path(definition, self.selected.indices())
            .and_then(MenuItem::as_command)
            .filter(|command| command.is_enabled)
    }

    fn move_selection(&mut self, definition: &MenuDefinition, delta: i32) {
        let depth = self.open_depth.max(1);
        let parent_path_length = depth.saturating_sub(1);
        let parent_path = &self.selected.indices()[..parent_path_length.min(self.selected.depth())];
        let Some(menu) = menu_at_parent_path(definition, parent_path) else {
            return;
        };
        let current = self
            .selected
            .indices()
            .get(parent_path_length)
            .copied()
            .unwrap_or(0);
        let Some(next) = selectable_offset(menu, current, delta) else {
            return;
        };
        self.selected.truncate(parent_path_length);
        self.selected.push(next);
        self.open_depth = self.selected.depth();
        self.hover_intent = None;
    }
}

/// One panel and its row rectangles at a specific menu depth.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CascadingMenuPanel {
    /// Zero-based depth.
    pub depth: usize,
    /// Row indices that locate this panel's definition below the root.
    pub path_prefix: MenuPath,
    /// Complete panel bounds.
    pub bounds: RectI,
    /// Row bounds matching the definition at this depth.
    pub rows: Vec<RectI>,
}

/// Immutable geometry for every open cascading panel.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CascadingMenuLayout {
    /// Panels from root to deepest open child.
    pub panels: Vec<CascadingMenuPanel>,
}

impl CascadingMenuLayout {
    /// Calculates viewport-clamped panels for the current recursive state.
    #[must_use]
    pub fn calculate(
        definition: &MenuDefinition,
        state: &CascadingMenuState,
        root_anchor: RectI,
        viewport: RectI,
    ) -> Self {
        if !state.is_open() {
            return Self::default();
        }
        let mut panels = Vec::new();
        let mut menu = definition;
        let mut anchor = root_anchor;
        let mut path_prefix = MenuPath::default();
        for depth in 0..state.open_depth() {
            let parent_panel = panels.last().map(|panel: &CascadingMenuPanel| panel.bounds);
            let panel = panel_geometry(
                menu,
                depth,
                path_prefix.clone(),
                anchor,
                viewport,
                parent_panel,
            );
            let selected_index = state.selected_path().indices().get(depth).copied();
            let child_anchor = selected_index
                .and_then(|index| panel.rows.get(index).copied())
                .unwrap_or(panel.bounds);
            panels.push(panel);
            let Some(index) = selected_index else {
                break;
            };
            let Some(submenu) = menu.items.get(index).and_then(MenuItem::as_submenu) else {
                break;
            };
            path_prefix.push(index);
            menu = submenu;
            anchor = child_anchor;
        }
        Self { panels }
    }

    /// Returns the deepest selectable row path containing `point`.
    #[must_use]
    pub fn path_at(&self, definition: &MenuDefinition, point: PointI) -> Option<MenuPath> {
        for panel in self.panels.iter().rev() {
            let Some(index) = panel.rows.iter().position(|row| row.contains(point)) else {
                continue;
            };
            let menu = menu_at_parent_path(definition, panel.path_prefix.indices())?;
            if is_selectable(menu.items.get(index)?) {
                let mut indices = panel.path_prefix.indices().to_vec();
                indices.push(index);
                return Some(MenuPath::new(indices));
            }
        }
        None
    }

    /// Returns whether pointer travel should preserve an already-open child panel.
    ///
    /// The corridor joins the selected parent row to the complete child panel and tolerates a small
    /// horizontal/vertical margin. This deterministic approximation avoids submenu flicker without
    /// depending on platform pointer history APIs.
    #[must_use]
    pub fn preserves_child_pointer_intent(
        &self,
        parent_depth: usize,
        pointer: PointI,
        margin: u32,
    ) -> bool {
        let Some(parent) = self.panels.get(parent_depth) else {
            return false;
        };
        let Some(child) = self.panels.get(parent_depth.saturating_add(1)) else {
            return false;
        };
        let Some(selected_index) = child.path_prefix.indices().last().copied() else {
            return false;
        };
        let Some(selected_row) = parent.rows.get(selected_index).copied() else {
            return false;
        };
        let expanded_parent = expand_rect(selected_row, margin);
        let expanded_child = expand_rect(child.bounds, margin);
        let left = expanded_parent.x.min(expanded_child.x);
        let top = expanded_parent.y.min(expanded_child.y);
        let right = expanded_parent.right().max(expanded_child.right());
        let bottom = expanded_parent.bottom().max(expanded_child.bottom());
        RectI::new(
            left,
            top,
            u32::try_from(right.saturating_sub(i64::from(left))).unwrap_or(u32::MAX),
            u32::try_from(bottom.saturating_sub(i64::from(top))).unwrap_or(u32::MAX),
        )
        .contains(pointer)
    }
}

fn panel_geometry(
    definition: &MenuDefinition,
    depth: usize,
    path_prefix: MenuPath,
    anchor: RectI,
    viewport: RectI,
    parent_panel: Option<RectI>,
) -> CascadingMenuPanel {
    let content_height = definition.items.iter().fold(0_u32, |height, item| {
        height.saturating_add(if matches!(item, MenuItem::Separator) {
            SEPARATOR_HEIGHT
        } else {
            ROW_HEIGHT
        })
    });
    let height = content_height.saturating_add(PANEL_PADDING.saturating_mul(2));
    let preferred_x = if parent_panel.is_none() {
        anchor.x
    } else {
        i32::try_from(anchor.right()).unwrap_or(i32::MAX)
    };
    let right_x = i32::try_from(viewport.right().saturating_sub(i64::from(PANEL_WIDTH)))
        .unwrap_or(viewport.x);
    let fallback_x = parent_panel
        .map_or(anchor.x, |parent| parent.x)
        .saturating_sub(i32::try_from(PANEL_WIDTH.saturating_add(2)).unwrap_or(i32::MAX));
    let x = if preferred_x <= right_x {
        preferred_x
    } else {
        fallback_x.max(viewport.x)
    };
    let maximum_y =
        i32::try_from(viewport.bottom().saturating_sub(i64::from(height))).unwrap_or(viewport.y);
    let preferred_y = if parent_panel.is_none() {
        i32::try_from(anchor.bottom()).unwrap_or(i32::MAX)
    } else {
        anchor.y
    };
    let y = preferred_y.clamp(viewport.y, maximum_y.max(viewport.y));
    let bounds = RectI::new(
        x,
        y,
        PANEL_WIDTH.min(viewport.width),
        height.min(viewport.height),
    );
    let mut rows = Vec::with_capacity(definition.items.len());
    let mut row_y = bounds
        .y
        .saturating_add(i32::try_from(PANEL_PADDING).unwrap_or(i32::MAX));
    for item in &definition.items {
        let row_height = if matches!(item, MenuItem::Separator) {
            SEPARATOR_HEIGHT
        } else {
            ROW_HEIGHT
        };
        rows.push(RectI::new(
            bounds
                .x
                .saturating_add(i32::try_from(PANEL_PADDING).unwrap_or(i32::MAX)),
            row_y,
            bounds.width.saturating_sub(PANEL_PADDING.saturating_mul(2)),
            row_height,
        ));
        row_y = row_y.saturating_add(i32::try_from(row_height).unwrap_or(i32::MAX));
    }
    CascadingMenuPanel {
        depth,
        path_prefix,
        bounds,
        rows,
    }
}

fn expand_rect(rect: RectI, margin: u32) -> RectI {
    let delta = i32::try_from(margin).unwrap_or(i32::MAX);
    RectI::new(
        rect.x.saturating_sub(delta),
        rect.y.saturating_sub(delta),
        rect.width.saturating_add(margin.saturating_mul(2)),
        rect.height.saturating_add(margin.saturating_mul(2)),
    )
}

fn menu_at_parent_path<'a>(
    definition: &'a MenuDefinition,
    parent_path: &[usize],
) -> Option<&'a MenuDefinition> {
    let mut menu = definition;
    for index in parent_path {
        menu = menu.items.get(*index)?.as_submenu()?;
    }
    Some(menu)
}

fn item_at_path<'a>(definition: &'a MenuDefinition, path: &[usize]) -> Option<&'a MenuItem> {
    let (last, parents) = path.split_last()?;
    menu_at_parent_path(definition, parents)?.items.get(*last)
}

fn is_selectable(item: &MenuItem) -> bool {
    match item {
        MenuItem::Command(command) => command.is_enabled,
        MenuItem::Submenu(definition) => first_selectable(definition).is_some(),
        MenuItem::Separator => false,
    }
}

fn first_selectable(definition: &MenuDefinition) -> Option<usize> {
    definition.items.iter().position(is_selectable)
}

fn selectable_offset(definition: &MenuDefinition, current: usize, delta: i32) -> Option<usize> {
    if definition.items.is_empty() {
        return None;
    }
    let count = i32::try_from(definition.items.len())
        .unwrap_or(i32::MAX)
        .max(1);
    for step in 1..=definition.items.len() {
        let signed_step = i32::try_from(step)
            .unwrap_or(i32::MAX)
            .saturating_mul(delta.signum());
        let index = (i32::try_from(current).unwrap_or(0) + signed_step).rem_euclid(count);
        let index = usize::try_from(index).unwrap_or(0);
        if definition.items.get(index).is_some_and(is_selectable) {
            return Some(index);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{CascadingMenuLayout, CascadingMenuState, MenuPath};
    use crate::{MenuCommand, MenuDefinition, MenuItem};
    use luna_core::{PointI, RectI};

    fn definition() -> MenuDefinition {
        MenuDefinition::new(
            "root",
            "Root",
            vec![MenuItem::submenu(MenuDefinition::new(
                "level-one",
                "Level One",
                vec![MenuItem::submenu(MenuDefinition::new(
                    "level-two",
                    "Level Two",
                    vec![MenuItem::command(MenuCommand::new("deep", "Deep", ""))],
                ))],
            ))],
        )
    }

    #[test]
    fn arbitrary_depth_keyboard_path_reaches_deep_command() {
        let definition = definition();
        let mut state = CascadingMenuState::default();
        state.open(&definition);
        assert!(state.open_selected_submenu(&definition));
        assert!(state.open_selected_submenu(&definition));
        assert_eq!(state.open_depth(), 3);
        assert_eq!(state.selected_path().indices(), &[0, 0, 0]);
        assert_eq!(
            state
                .selected_command(&definition)
                .map(|command| command.id.as_str()),
            Some("deep")
        );
        assert!(state.close_child());
        assert_eq!(state.selected_path().indices(), &[0, 0]);
    }

    #[test]
    fn hover_selection_waits_for_logical_delay() {
        let definition = definition();
        let mut state = CascadingMenuState::default();
        state.open(&definition);
        let candidate = MenuPath::new(vec![0]);
        assert!(!state.update_hover(&definition, candidate.clone(), 100, 150, false));
        assert!(!state.update_hover(&definition, candidate.clone(), 249, 150, false));
        assert!(state.update_hover(&definition, candidate, 250, 150, false));
        assert_eq!(state.open_depth(), 2);
    }

    #[test]
    fn layout_projects_three_clamped_panels_and_pointer_corridor() {
        let definition = definition();
        let mut state = CascadingMenuState::default();
        state.open(&definition);
        assert!(state.open_selected_submenu(&definition));
        assert!(state.open_selected_submenu(&definition));
        let layout = CascadingMenuLayout::calculate(
            &definition,
            &state,
            RectI::new(4, 4, 40, 20),
            RectI::new(0, 0, 900, 500),
        );
        assert_eq!(layout.panels.len(), 3);
        let deepest_row = layout.panels[2].rows[0];
        assert_eq!(
            layout.path_at(
                &definition,
                PointI::new(
                    deepest_row.x.saturating_add(1),
                    deepest_row.y.saturating_add(1)
                ),
            ),
            Some(MenuPath::new(vec![0, 0, 0]))
        );
        assert!(layout.preserves_child_pointer_intent(0, PointI::new(260, 50), 8));
    }
}
