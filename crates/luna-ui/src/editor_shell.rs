// SPDX-License-Identifier: MPL-2.0

use crate::Widget;
use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{NodeId, NodeIdError, PointI, RectI};
use luna_render::DisplayList;
use luna_theme::Theme;

/// One top-level application menu shown by the editor demonstration shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellMenu {
    /// Stable product-neutral menu ID.
    pub id: String,
    /// Visible menu title.
    pub title: String,
}

impl ShellMenu {
    /// Creates a menu definition.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
        }
    }
}

/// One document tab in the editor shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellTab {
    /// Stable tab identifier.
    pub id: String,
    /// Visible document title.
    pub title: String,
    /// Whether the backing document differs from its saved revision.
    pub is_dirty: bool,
    /// Whether the tab may be closed.
    pub is_closable: bool,
}

impl ShellTab {
    /// Creates a closable shell tab.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            is_dirty: false,
            is_closable: true,
        }
    }
}

/// Product-neutral kind for a project/sidebar row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SidebarItemKind {
    /// Expandable project or folder row.
    Folder,
    /// File/document row.
    File,
}

/// One flattened visible project/sidebar row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SidebarItem {
    /// Stable item identifier.
    pub id: String,
    /// Visible row title.
    pub title: String,
    /// Hierarchy depth used for indentation.
    pub depth: u16,
    /// Row kind.
    pub kind: SidebarItemKind,
    /// Whether an expandable row is open.
    pub is_expanded: bool,
}

impl SidebarItem {
    /// Creates a file row.
    #[must_use]
    pub fn file(id: impl Into<String>, title: impl Into<String>, depth: u16) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            depth,
            kind: SidebarItemKind::File,
            is_expanded: false,
        }
    }

    /// Creates a folder row.
    #[must_use]
    pub fn folder(
        id: impl Into<String>,
        title: impl Into<String>,
        depth: u16,
        is_expanded: bool,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            depth,
            kind: SidebarItemKind::Folder,
            is_expanded,
        }
    }
}

/// Application-supplied editor-shell state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorShellState {
    /// Top-level menu definitions.
    pub menus: Vec<ShellMenu>,
    /// Open document tabs.
    pub tabs: Vec<ShellTab>,
    /// Active top-level dropdown menu ID, or `None` when menus are closed.
    pub active_menu_id: Option<String>,
    /// Active tab ID.
    pub active_tab_id: Option<String>,
    /// Visible sidebar rows.
    pub sidebar_items: Vec<SidebarItem>,
    /// Selected sidebar row ID.
    pub selected_sidebar_id: Option<String>,
    /// Whether the sidebar is visible.
    pub sidebar_is_visible: bool,
    /// Preferred sidebar width.
    pub sidebar_width: u32,
    /// Left status text.
    pub status_left: String,
    /// Right status text.
    pub status_right: String,
    /// Semantic children mounted inside the editor content lane by the application.
    pub editor_children: Vec<NodeId>,
}

impl Default for EditorShellState {
    fn default() -> Self {
        Self {
            menus: vec![
                ShellMenu::new("file", "File"),
                ShellMenu::new("edit", "Edit"),
                ShellMenu::new("find", "Find"),
                ShellMenu::new("view", "View"),
                ShellMenu::new("help", "Help"),
            ],
            tabs: Vec::new(),
            active_menu_id: None,
            active_tab_id: None,
            sidebar_items: Vec::new(),
            selected_sidebar_id: None,
            sidebar_is_visible: true,
            sidebar_width: 236,
            status_left: String::new(),
            status_right: String::new(),
            editor_children: Vec::new(),
        }
    }
}

/// Editor shell metrics ported from the Swift Luna editor-test harness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EditorShellMetrics {
    /// Menu-bar height.
    pub menu_bar_height: u32,
    /// Document tab-strip height.
    pub tab_strip_height: u32,
    /// Bottom status-bar height.
    pub status_bar_height: u32,
    /// Sidebar header height.
    pub sidebar_header_height: u32,
    /// Sidebar row height.
    pub sidebar_row_height: u32,
    /// Minimum sidebar width.
    pub sidebar_minimum_width: u32,
    /// Maximum sidebar width.
    pub sidebar_maximum_width: u32,
    /// Width of a top-level menu entry.
    pub menu_width: u32,
    /// Minimum tab width.
    pub tab_minimum_width: u32,
    /// Maximum tab width.
    pub tab_maximum_width: u32,
    /// Sidebar indentation per hierarchy level.
    pub sidebar_indent: u32,
    /// One-pixel separator width.
    pub separator_width: u32,
}

impl Default for EditorShellMetrics {
    fn default() -> Self {
        Self {
            menu_bar_height: 28,
            tab_strip_height: 30,
            status_bar_height: 26,
            sidebar_header_height: 26,
            sidebar_row_height: 22,
            sidebar_minimum_width: 160,
            sidebar_maximum_width: 360,
            menu_width: 64,
            tab_minimum_width: 92,
            tab_maximum_width: 210,
            sidebar_indent: 14,
            separator_width: 1,
        }
    }
}

/// Geometry for one menu, tab, or sidebar row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellItemFrame {
    /// Stable source ID.
    pub id: String,
    /// Stable Luna semantic ID.
    pub node_id: NodeId,
    /// Visible title.
    pub title: String,
    /// Shared paint/hit/accessibility bounds.
    pub bounds: RectI,
    /// Optional secondary action geometry, currently used for tab close buttons.
    pub accessory_bounds: Option<RectI>,
    /// Whether this row is active or selected.
    pub is_selected: bool,
    /// Hierarchy depth for sidebar rows.
    pub depth: u16,
}

/// Complete editor-shell geometry shared across painting, labels, hit tests, and accessibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorShellLayout {
    /// Complete shell bounds.
    pub bounds: RectI,
    /// Top menu bar.
    pub menu_bar: RectI,
    /// Menu entry frames.
    pub menus: Vec<ShellItemFrame>,
    /// Document tab strip.
    pub tab_strip: RectI,
    /// Tab frames.
    pub tabs: Vec<ShellItemFrame>,
    /// Content area between tabs and status.
    pub content: RectI,
    /// Sidebar pane, or an empty rectangle when hidden.
    pub sidebar: RectI,
    /// Sidebar header.
    pub sidebar_header: RectI,
    /// Sidebar visible row frames.
    pub sidebar_rows: Vec<ShellItemFrame>,
    /// Main editor content viewport.
    pub editor: RectI,
    /// Bottom status bar.
    pub status_bar: RectI,
    /// Left status label bounds.
    pub status_left: RectI,
    /// Right status label bounds.
    pub status_right: RectI,
}

/// Semantic hit result for editor-shell interaction routing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditorShellHit {
    /// Top-level menu entry.
    Menu(String),
    /// Document tab body.
    Tab(String),
    /// Document tab close accessory.
    CloseTab(String),
    /// Sidebar row.
    SidebarItem(String),
    /// Main editor surface.
    Editor,
}

/// Reusable editor window anatomy used by the native editor demonstration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorShell {
    id: NodeId,
    bounds: RectI,
    theme: Theme,
    state: EditorShellState,
    metrics: EditorShellMetrics,
    menu_bar_id: NodeId,
    tab_list_id: NodeId,
    sidebar_id: NodeId,
    editor_id: NodeId,
    status_id: NodeId,
    layout: EditorShellLayout,
}

impl EditorShell {
    /// Creates and lays out an editor shell.
    pub fn new(
        id: NodeId,
        bounds: RectI,
        theme: Theme,
        state: EditorShellState,
        metrics: EditorShellMetrics,
    ) -> Result<Self, NodeIdError> {
        let menu_bar_id = id.child("menu-bar")?;
        let tab_list_id = id.child("tabs")?;
        let sidebar_id = id.child("sidebar")?;
        let editor_id = id.child("editor")?;
        let status_id = id.child("status")?;
        let layout = calculate_layout(
            bounds,
            &state,
            metrics,
            &menu_bar_id,
            &tab_list_id,
            &sidebar_id,
        );
        Ok(Self {
            id,
            bounds,
            theme,
            state,
            metrics,
            menu_bar_id,
            tab_list_id,
            sidebar_id,
            editor_id,
            status_id,
            layout,
        })
    }

    /// Returns the immutable geometry snapshot.
    #[must_use]
    pub const fn layout(&self) -> &EditorShellLayout {
        &self.layout
    }

    /// Resolves a pointer position into a semantic shell target.
    #[must_use]
    pub fn semantic_hit_test(&self, point: PointI) -> Option<EditorShellHit> {
        for frame in &self.layout.menus {
            if frame.bounds.contains(point) {
                return Some(EditorShellHit::Menu(frame.id.clone()));
            }
        }
        for frame in &self.layout.tabs {
            if frame
                .accessory_bounds
                .is_some_and(|bounds| bounds.contains(point))
            {
                return Some(EditorShellHit::CloseTab(frame.id.clone()));
            }
            if frame.bounds.contains(point) {
                return Some(EditorShellHit::Tab(frame.id.clone()));
            }
        }
        for frame in &self.layout.sidebar_rows {
            if frame.bounds.contains(point) {
                return Some(EditorShellHit::SidebarItem(frame.id.clone()));
            }
        }
        self.layout
            .editor
            .contains(point)
            .then_some(EditorShellHit::Editor)
    }

    /// Resolves a stable semantic node ID back into an application-level shell target.
    #[must_use]
    pub fn semantic_target(&self, node_id: &NodeId) -> Option<EditorShellHit> {
        self.layout
            .menus
            .iter()
            .find(|frame| &frame.node_id == node_id)
            .map(|frame| EditorShellHit::Menu(frame.id.clone()))
            .or_else(|| {
                self.layout
                    .tabs
                    .iter()
                    .find(|frame| &frame.node_id == node_id)
                    .map(|frame| EditorShellHit::Tab(frame.id.clone()))
            })
            .or_else(|| {
                self.layout
                    .sidebar_rows
                    .iter()
                    .find(|frame| &frame.node_id == node_id)
                    .map(|frame| EditorShellHit::SidebarItem(frame.id.clone()))
            })
            .or_else(|| (node_id == &self.editor_id).then_some(EditorShellHit::Editor))
    }

    /// Returns the root semantic ID of the editor content lane.
    #[must_use]
    pub const fn editor_node_id(&self) -> &NodeId {
        &self.editor_id
    }
}

impl Widget for EditorShell {
    fn id(&self) -> &NodeId {
        &self.id
    }

    fn bounds(&self) -> RectI {
        self.bounds
    }

    fn build_display_list(&self, display_list: &mut DisplayList) {
        display_list.fill_rect(self.layout.menu_bar, self.theme.panel_header);
        for frame in &self.layout.menus {
            if frame.is_selected {
                display_list.fill_rect(frame.bounds, self.theme.hover_surface());
                display_list.fill_rect(
                    RectI::new(
                        frame.bounds.x,
                        i32::try_from(frame.bounds.bottom().saturating_sub(2))
                            .unwrap_or(frame.bounds.y),
                        frame.bounds.width,
                        2,
                    ),
                    self.theme.accent,
                );
            }
        }
        display_list.fill_rect(self.layout.tab_strip, self.theme.panel);
        for frame in &self.layout.tabs {
            let color = if frame.is_selected {
                self.theme.background
            } else {
                self.theme.panel
            };
            display_list.fill_rect(frame.bounds, color);
            if frame.is_selected {
                display_list.fill_rect(
                    RectI::new(frame.bounds.x, frame.bounds.y, frame.bounds.width, 2),
                    self.theme.accent,
                );
            }
            if let Some(accessory) = frame.accessory_bounds {
                display_list.fill_rect(accessory, self.theme.panel_header);
            }
        }
        if self.state.sidebar_is_visible {
            display_list.fill_rect(self.layout.sidebar, self.theme.panel);
            display_list.fill_rect(self.layout.sidebar_header, self.theme.panel_header);
            for frame in &self.layout.sidebar_rows {
                if frame.is_selected {
                    display_list.fill_rect(frame.bounds, self.theme.selection());
                }
            }
            let separator_x = i32::try_from(self.layout.sidebar.right()).unwrap_or(i32::MAX);
            display_list.fill_rect(
                RectI::new(
                    separator_x
                        .saturating_sub(i32::try_from(self.metrics.separator_width).unwrap_or(1)),
                    self.layout.sidebar.y,
                    self.metrics.separator_width,
                    self.layout.sidebar.height,
                ),
                self.theme.border(),
            );
        }
        display_list.fill_rect(self.layout.editor, self.theme.background);
        display_list.fill_rect(self.layout.status_bar, self.theme.panel_header);
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        let mut nodes = Vec::new();
        let root_children = vec![
            self.menu_bar_id.clone(),
            self.tab_list_id.clone(),
            self.sidebar_id.clone(),
            self.editor_id.clone(),
            self.status_id.clone(),
        ];
        nodes.push(
            AccessibilityNode::new(self.id.clone(), AccessibilityRole::Group, self.bounds)
                .with_label("Luna UI Rust editor demonstration")
                .with_children(root_children),
        );
        nodes.push(
            AccessibilityNode::new(
                self.menu_bar_id.clone(),
                AccessibilityRole::MenuBar,
                self.layout.menu_bar,
            )
            .with_label("Application menu")
            .with_children(
                self.layout
                    .menus
                    .iter()
                    .map(|frame| frame.node_id.clone())
                    .collect(),
            ),
        );
        for frame in &self.layout.menus {
            nodes.push(
                AccessibilityNode::new(
                    frame.node_id.clone(),
                    AccessibilityRole::MenuItem,
                    frame.bounds,
                )
                .with_label(frame.title.clone())
                .with_value(if frame.is_selected {
                    "Expanded"
                } else {
                    "Collapsed"
                }),
            );
        }
        nodes.push(
            AccessibilityNode::new(
                self.tab_list_id.clone(),
                AccessibilityRole::TabList,
                self.layout.tab_strip,
            )
            .with_label("Open documents")
            .with_children(
                self.layout
                    .tabs
                    .iter()
                    .map(|frame| frame.node_id.clone())
                    .collect(),
            ),
        );
        for frame in &self.layout.tabs {
            let dirty = self
                .state
                .tabs
                .iter()
                .find(|tab| tab.id == frame.id)
                .is_some_and(|tab| tab.is_dirty);
            nodes.push(
                AccessibilityNode::new(frame.node_id.clone(), AccessibilityRole::Tab, frame.bounds)
                    .with_label(frame.title.clone())
                    .with_value(if dirty { "Modified" } else { "Saved" })
                    .with_focused(frame.is_selected),
            );
        }
        nodes.push(
            AccessibilityNode::new(
                self.sidebar_id.clone(),
                AccessibilityRole::Tree,
                self.layout.sidebar,
            )
            .with_label("Project files")
            .with_children(
                self.layout
                    .sidebar_rows
                    .iter()
                    .map(|frame| frame.node_id.clone())
                    .collect(),
            ),
        );
        for frame in &self.layout.sidebar_rows {
            nodes.push(
                AccessibilityNode::new(
                    frame.node_id.clone(),
                    AccessibilityRole::TreeItem,
                    frame.bounds,
                )
                .with_label(frame.title.clone())
                .with_focused(frame.is_selected),
            );
        }
        nodes.push(
            AccessibilityNode::new(
                self.editor_id.clone(),
                AccessibilityRole::Group,
                self.layout.editor,
            )
            .with_label("Editor content")
            .with_children(self.state.editor_children.clone()),
        );
        nodes.push(
            AccessibilityNode::new(
                self.status_id.clone(),
                AccessibilityRole::Status,
                self.layout.status_bar,
            )
            .with_label("Editor status")
            .with_value(format!(
                "{} {}",
                self.state.status_left, self.state.status_right
            )),
        );
        nodes
    }

    fn hit_test(&self, point: PointI) -> Option<NodeId> {
        match self.semantic_hit_test(point) {
            Some(EditorShellHit::Menu(id)) => self
                .layout
                .menus
                .iter()
                .find(|frame| frame.id == id)
                .map(|frame| frame.node_id.clone()),
            Some(EditorShellHit::Tab(id) | EditorShellHit::CloseTab(id)) => self
                .layout
                .tabs
                .iter()
                .find(|frame| frame.id == id)
                .map(|frame| frame.node_id.clone()),
            Some(EditorShellHit::SidebarItem(id)) => self
                .layout
                .sidebar_rows
                .iter()
                .find(|frame| frame.id == id)
                .map(|frame| frame.node_id.clone()),
            Some(EditorShellHit::Editor) => Some(self.editor_id.clone()),
            None => self.bounds.contains(point).then_some(self.id.clone()),
        }
    }
}

fn calculate_layout(
    bounds: RectI,
    state: &EditorShellState,
    metrics: EditorShellMetrics,
    menu_bar_id: &NodeId,
    tab_list_id: &NodeId,
    sidebar_id: &NodeId,
) -> EditorShellLayout {
    let menu_height = metrics.menu_bar_height.min(bounds.height);
    let menu_bar = RectI::new(bounds.x, bounds.y, bounds.width, menu_height);
    let tab_y = bounds
        .y
        .saturating_add(i32::try_from(menu_height).unwrap_or(i32::MAX));
    let remaining_after_menu = bounds.height.saturating_sub(menu_height);
    let tab_height = metrics.tab_strip_height.min(remaining_after_menu);
    let tab_strip = RectI::new(bounds.x, tab_y, bounds.width, tab_height);
    let status_height = metrics
        .status_bar_height
        .min(remaining_after_menu.saturating_sub(tab_height));
    let content_y = tab_y.saturating_add(i32::try_from(tab_height).unwrap_or(i32::MAX));
    let content_height = remaining_after_menu
        .saturating_sub(tab_height)
        .saturating_sub(status_height);
    let content = RectI::new(bounds.x, content_y, bounds.width, content_height);
    let status_y = content_y.saturating_add(i32::try_from(content_height).unwrap_or(i32::MAX));
    let status_bar = RectI::new(bounds.x, status_y, bounds.width, status_height);

    let mut menus = Vec::new();
    let mut menu_x = menu_bar.x.saturating_add(8);
    for definition in &state.menus {
        let width = metrics
            .menu_width
            .min(u32::try_from(menu_bar.right().saturating_sub(i64::from(menu_x))).unwrap_or(0));
        if width == 0 {
            break;
        }
        if let Ok(node_id) = menu_bar_id.child(&definition.id) {
            menus.push(ShellItemFrame {
                id: definition.id.clone(),
                node_id,
                title: definition.title.clone(),
                bounds: RectI::new(menu_x, menu_bar.y, width, menu_bar.height),
                accessory_bounds: None,
                is_selected: state.active_menu_id.as_ref() == Some(&definition.id),
                depth: 0,
            });
        }
        menu_x = menu_x.saturating_add(i32::try_from(width).unwrap_or(i32::MAX));
    }

    let tab_count = u32::try_from(state.tabs.len()).unwrap_or(u32::MAX).max(1);
    let available_tab_width = tab_strip.width / tab_count;
    let minimum_tab_width = metrics
        .tab_minimum_width
        .min(metrics.tab_maximum_width)
        .min(tab_strip.width);
    let maximum_tab_width = metrics
        .tab_maximum_width
        .max(minimum_tab_width)
        .min(tab_strip.width);
    let tab_width = available_tab_width
        .clamp(minimum_tab_width, maximum_tab_width)
        .min(tab_strip.width);
    let mut tabs = Vec::new();
    let mut tab_x = tab_strip.x;
    for tab in &state.tabs {
        let remaining =
            u32::try_from(tab_strip.right().saturating_sub(i64::from(tab_x))).unwrap_or(0);
        let width = tab_width.min(remaining);
        if width == 0 {
            break;
        }
        if let Ok(node_id) = tab_list_id.child(&tab.id) {
            let accessory_size = 14_u32.min(tab_strip.height.saturating_sub(8));
            let accessory_x = i32::try_from(
                i64::from(tab_x)
                    .saturating_add(i64::from(width))
                    .saturating_sub(i64::from(accessory_size) + 8),
            )
            .unwrap_or(i32::MAX);
            let accessory_y = tab_strip.y.saturating_add(
                i32::try_from(tab_strip.height.saturating_sub(accessory_size) / 2).unwrap_or(0),
            );
            tabs.push(ShellItemFrame {
                id: tab.id.clone(),
                node_id,
                title: tab.title.clone(),
                bounds: RectI::new(tab_x, tab_strip.y, width, tab_strip.height),
                accessory_bounds: tab.is_closable.then_some(RectI::new(
                    accessory_x,
                    accessory_y,
                    accessory_size,
                    accessory_size,
                )),
                is_selected: state.active_tab_id.as_ref() == Some(&tab.id),
                depth: 0,
            });
        }
        tab_x = tab_x.saturating_add(i32::try_from(width).unwrap_or(i32::MAX));
    }

    let sidebar_width = if state.sidebar_is_visible {
        let minimum_sidebar_width = metrics
            .sidebar_minimum_width
            .min(metrics.sidebar_maximum_width);
        let maximum_sidebar_width = metrics.sidebar_maximum_width.max(minimum_sidebar_width);
        state
            .sidebar_width
            .clamp(minimum_sidebar_width, maximum_sidebar_width)
            .min(content.width)
    } else {
        0
    };
    let sidebar = RectI::new(content.x, content.y, sidebar_width, content.height);
    let sidebar_header_height = metrics.sidebar_header_height.min(sidebar.height);
    let sidebar_header = RectI::new(sidebar.x, sidebar.y, sidebar.width, sidebar_header_height);
    let editor_x = content
        .x
        .saturating_add(i32::try_from(sidebar_width).unwrap_or(i32::MAX));
    let editor = RectI::new(
        editor_x,
        content.y,
        content.width.saturating_sub(sidebar_width),
        content.height,
    );

    let mut sidebar_rows = Vec::new();
    let mut row_y = sidebar_header
        .y
        .saturating_add(i32::try_from(sidebar_header.height).unwrap_or(i32::MAX));
    let sidebar_bottom = sidebar.bottom();
    for item in &state.sidebar_items {
        if i64::from(row_y) >= sidebar_bottom {
            break;
        }
        let remaining = u32::try_from(sidebar_bottom.saturating_sub(i64::from(row_y))).unwrap_or(0);
        let row_height = metrics.sidebar_row_height.min(remaining);
        if row_height == 0 {
            break;
        }
        if let Ok(node_id) = sidebar_id.child(&item.id) {
            sidebar_rows.push(ShellItemFrame {
                id: item.id.clone(),
                node_id,
                title: item.title.clone(),
                bounds: RectI::new(sidebar.x, row_y, sidebar.width, row_height),
                accessory_bounds: None,
                is_selected: state.selected_sidebar_id.as_ref() == Some(&item.id),
                depth: item.depth,
            });
        }
        row_y = row_y.saturating_add(i32::try_from(row_height).unwrap_or(i32::MAX));
    }

    let status_padding = 10_u32.min(status_bar.width / 2);
    let status_left = RectI::new(
        status_bar
            .x
            .saturating_add(i32::try_from(status_padding).unwrap_or(i32::MAX)),
        status_bar.y,
        status_bar
            .width
            .saturating_sub(status_padding.saturating_mul(2))
            / 2,
        status_bar.height,
    );
    let right_width = status_bar
        .width
        .saturating_sub(status_padding.saturating_mul(2))
        / 2;
    let status_right = RectI::new(
        i32::try_from(
            status_bar
                .right()
                .saturating_sub(i64::from(right_width + status_padding)),
        )
        .unwrap_or(i32::MAX),
        status_bar.y,
        right_width,
        status_bar.height,
    );

    EditorShellLayout {
        bounds,
        menu_bar,
        menus,
        tab_strip,
        tabs,
        content,
        sidebar,
        sidebar_header,
        sidebar_rows,
        editor,
        status_bar,
        status_left,
        status_right,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EditorShell, EditorShellHit, EditorShellMetrics, EditorShellState, ShellTab, SidebarItem,
    };
    use crate::Widget;
    use luna_core::{NodeId, PointI, RectI};
    use luna_theme::Theme;
    use std::error::Error;

    #[test]
    fn shell_geometry_drives_tabs_sidebar_and_editor_hits() -> Result<(), Box<dyn Error>> {
        let state = EditorShellState {
            tabs: vec![ShellTab::new("readme", "README.md")],
            active_tab_id: Some("readme".to_owned()),
            sidebar_items: vec![SidebarItem::file("readme", "README.md", 0)],
            selected_sidebar_id: Some("readme".to_owned()),
            ..EditorShellState::default()
        };
        let shell = EditorShell::new(
            NodeId::new("shell")?,
            RectI::new(0, 0, 1_000, 700),
            Theme::luna_dark(),
            state,
            EditorShellMetrics::default(),
        )?;

        let tab = &shell.layout().tabs[0];
        assert_eq!(
            shell.semantic_hit_test(PointI::new(tab.bounds.x + 2, tab.bounds.y + 2)),
            Some(EditorShellHit::Tab("readme".to_owned()))
        );
        assert!(shell.layout().editor.width > 0);
        assert_eq!(
            shell.semantic_target(&tab.node_id),
            Some(EditorShellHit::Tab("readme".to_owned()))
        );
        assert_eq!(shell.accessibility_nodes()[0].bounds, shell.bounds());
        Ok(())
    }

    #[test]
    fn active_menu_heading_is_selected_and_semantically_expanded() -> Result<(), Box<dyn Error>> {
        let shell = EditorShell::new(
            NodeId::new("shell")?,
            RectI::new(0, 0, 800, 600),
            Theme::luna_dark(),
            EditorShellState {
                active_menu_id: Some("edit".to_owned()),
                ..EditorShellState::default()
            },
            EditorShellMetrics::default(),
        )?;
        let edit_frame = shell
            .layout()
            .menus
            .iter()
            .find(|frame| frame.id == "edit")
            .ok_or_else(|| std::io::Error::other("edit menu frame missing"))?;
        assert!(edit_frame.is_selected);
        assert!(shell.accessibility_nodes().iter().any(|node| {
            node.id == edit_frame.node_id && node.value.as_deref() == Some("Expanded")
        }));
        Ok(())
    }

    #[test]
    fn hidden_sidebar_returns_all_content_width_to_editor() -> Result<(), Box<dyn Error>> {
        let state = EditorShellState {
            sidebar_is_visible: false,
            ..EditorShellState::default()
        };
        let shell = EditorShell::new(
            NodeId::new("shell")?,
            RectI::new(0, 0, 800, 600),
            Theme::luna_dark(),
            state,
            EditorShellMetrics::default(),
        )?;

        assert_eq!(shell.layout().sidebar.width, 0);
        assert_eq!(shell.layout().editor.width, shell.layout().content.width);
        Ok(())
    }
}
