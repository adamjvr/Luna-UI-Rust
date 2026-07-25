// SPDX-License-Identifier: MPL-2.0

use crate::Widget;
use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{NodeId, NodeIdError, PointI, RectI};
use luna_documents::DocumentViewId;
use luna_panes::{PaneAxis, PaneId, PaneLayoutMetrics, PaneLayoutSnapshot, PaneTree};
use luna_render::DisplayList;
use luna_theme::Theme;

/// One document tab projected into a pane-local tab strip.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneTab {
    /// Stable document-view identity.
    pub view_id: DocumentViewId,
    /// Visible document title.
    pub title: String,
    /// Whether the shared document buffer differs from its saved revision.
    pub is_dirty: bool,
    /// Whether the tab may be closed.
    pub is_closable: bool,
}

impl PaneTab {
    /// Creates a closable pane-local tab.
    #[must_use]
    pub fn new(view_id: DocumentViewId, title: impl Into<String>) -> Self {
        Self {
            view_id,
            title: title.into(),
            is_dirty: false,
            is_closable: true,
        }
    }
}

/// Application projection for one leaf pane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PanePresentation {
    /// Stable pane identity.
    pub pane_id: PaneId,
    /// Pane-local tabs in display order.
    pub tabs: Vec<PaneTab>,
    /// Active tab in this pane.
    pub active_view: DocumentViewId,
    /// Semantic child mounted in the pane's editor viewport.
    pub editor_child: NodeId,
}

/// Application-supplied state for a recursive editor-pane surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorPaneSurfaceState {
    /// Product-neutral recursive topology.
    pub tree: PaneTree,
    /// Per-leaf tabs and semantic children.
    pub panes: Vec<PanePresentation>,
}

/// Shared geometry for one pane-local tab.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneTabFrame {
    /// Owning pane.
    pub pane_id: PaneId,
    /// Stable document view.
    pub view_id: DocumentViewId,
    /// Stable semantic node ID.
    pub node_id: NodeId,
    /// Visible title.
    pub title: String,
    /// Shared paint, hit, and semantic bounds.
    pub bounds: RectI,
    /// Optional close-button semantic identity.
    pub close_node_id: Option<NodeId>,
    /// Optional close-button bounds.
    pub close_bounds: Option<RectI>,
    /// Whether this tab is active in its pane.
    pub is_active: bool,
    /// Whether the shared document is dirty.
    pub is_dirty: bool,
}

/// Complete pane-surface geometry shared by paint, hit testing, labels, and accessibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorPaneSurfaceLayout {
    /// Recursive leaf and splitter geometry.
    pub panes: PaneLayoutSnapshot,
    /// Pane-local tab frames.
    pub tabs: Vec<PaneTabFrame>,
}

/// Semantic pointer target inside a pane surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorPaneSurfaceHit {
    /// Pane-local tab body.
    Tab {
        /// Owning pane.
        pane_id: PaneId,
        /// Activated view.
        view_id: DocumentViewId,
    },
    /// Pane-local tab close accessory.
    CloseTab {
        /// Owning pane.
        pane_id: PaneId,
        /// Closed view.
        view_id: DocumentViewId,
    },
    /// Editor viewport for one pane.
    Editor(PaneId),
    /// Draggable splitter.
    Splitter(PaneId),
}

/// Reusable recursive editor-pane chrome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorPaneSurface {
    id: NodeId,
    bounds: RectI,
    theme: Theme,
    state: EditorPaneSurfaceState,
    layout: EditorPaneSurfaceLayout,
}

impl EditorPaneSurface {
    /// Creates and lays out pane chrome.
    pub fn new(
        id: NodeId,
        bounds: RectI,
        theme: Theme,
        state: EditorPaneSurfaceState,
        metrics: PaneLayoutMetrics,
    ) -> Result<Self, NodeIdError> {
        let pane_layout = state.tree.layout(bounds, metrics);
        let mut tabs = Vec::new();
        for leaf in &pane_layout.leaves {
            let Some(presentation) = state
                .panes
                .iter()
                .find(|presentation| presentation.pane_id == leaf.pane_id)
            else {
                continue;
            };
            let tab_count = u32::try_from(presentation.tabs.len())
                .unwrap_or(u32::MAX)
                .max(1);
            let tab_width = (leaf.tab_strip.width / tab_count).clamp(82, 210);
            let mut tab_x = leaf.tab_strip.x;
            for tab in &presentation.tabs {
                let remaining =
                    u32::try_from(leaf.tab_strip.right().saturating_sub(i64::from(tab_x)))
                        .unwrap_or(0);
                let width = tab_width.min(remaining);
                if width == 0 {
                    break;
                }
                let pane_node = id.child(&leaf.pane_id.stable_key())?;
                let tab_node = pane_node.child("tabs")?.child(&tab.view_id.stable_key())?;
                let close_node_id = if tab.is_closable {
                    Some(tab_node.child("close")?)
                } else {
                    None
                };
                let close_size = 14_u32.min(leaf.tab_strip.height.saturating_sub(8));
                let close_x = i32::try_from(
                    i64::from(tab_x)
                        .saturating_add(i64::from(width))
                        .saturating_sub(i64::from(close_size) + 7),
                )
                .unwrap_or(i32::MAX);
                let close_y = leaf.tab_strip.y.saturating_add(
                    i32::try_from(leaf.tab_strip.height.saturating_sub(close_size) / 2)
                        .unwrap_or(0),
                );
                tabs.push(PaneTabFrame {
                    pane_id: leaf.pane_id,
                    view_id: tab.view_id,
                    node_id: tab_node,
                    title: tab.title.clone(),
                    bounds: RectI::new(tab_x, leaf.tab_strip.y, width, leaf.tab_strip.height),
                    close_node_id,
                    close_bounds: tab
                        .is_closable
                        .then_some(RectI::new(close_x, close_y, close_size, close_size)),
                    is_active: presentation.active_view == tab.view_id,
                    is_dirty: tab.is_dirty,
                });
                tab_x = tab_x.saturating_add(i32::try_from(width).unwrap_or(i32::MAX));
            }
        }
        Ok(Self {
            id,
            bounds,
            theme,
            state,
            layout: EditorPaneSurfaceLayout {
                panes: pane_layout,
                tabs,
            },
        })
    }

    /// Returns the immutable pane geometry.
    #[must_use]
    pub const fn layout(&self) -> &EditorPaneSurfaceLayout {
        &self.layout
    }

    /// Resolves a pointer position into a pane target.
    #[must_use]
    pub fn semantic_hit_test(&self, point: PointI) -> Option<EditorPaneSurfaceHit> {
        for tab in &self.layout.tabs {
            if tab
                .close_bounds
                .is_some_and(|bounds| bounds.contains(point))
            {
                return Some(EditorPaneSurfaceHit::CloseTab {
                    pane_id: tab.pane_id,
                    view_id: tab.view_id,
                });
            }
            if tab.bounds.contains(point) {
                return Some(EditorPaneSurfaceHit::Tab {
                    pane_id: tab.pane_id,
                    view_id: tab.view_id,
                });
            }
        }
        if let Some(splitter) = self.layout.panes.splitter_at(point) {
            return Some(EditorPaneSurfaceHit::Splitter(splitter.split_id));
        }
        self.layout
            .panes
            .editor_at(point)
            .map(|leaf| EditorPaneSurfaceHit::Editor(leaf.pane_id))
    }

    /// Resolves a semantic node ID into an application target.
    #[must_use]
    pub fn semantic_target(&self, node_id: &NodeId) -> Option<EditorPaneSurfaceHit> {
        self.layout
            .tabs
            .iter()
            .find(|frame| frame.close_node_id.as_ref() == Some(node_id))
            .map(|frame| EditorPaneSurfaceHit::CloseTab {
                pane_id: frame.pane_id,
                view_id: frame.view_id,
            })
            .or_else(|| {
                self.layout
                    .tabs
                    .iter()
                    .find(|frame| &frame.node_id == node_id)
                    .map(|frame| EditorPaneSurfaceHit::Tab {
                        pane_id: frame.pane_id,
                        view_id: frame.view_id,
                    })
            })
            .or_else(|| {
                self.layout.panes.leaves.iter().find_map(|leaf| {
                    self.editor_node_id(leaf.pane_id)
                        .ok()
                        .filter(|id| id == node_id)
                        .map(|_| EditorPaneSurfaceHit::Editor(leaf.pane_id))
                })
            })
            .or_else(|| {
                self.layout.panes.splitters.iter().find_map(|splitter| {
                    self.splitter_node_id(splitter.split_id)
                        .ok()
                        .filter(|id| id == node_id)
                        .map(|_| EditorPaneSurfaceHit::Splitter(splitter.split_id))
                })
            })
    }

    /// Returns the semantic editor-group ID for one leaf pane.
    pub fn editor_node_id(&self, pane_id: PaneId) -> Result<NodeId, NodeIdError> {
        self.id.child(&pane_id.stable_key())?.child("editor")
    }

    /// Returns the semantic split-handle ID.
    pub fn splitter_node_id(&self, split_id: PaneId) -> Result<NodeId, NodeIdError> {
        self.id.child(&split_id.stable_key())?.child("splitter")
    }
}

impl Widget for EditorPaneSurface {
    fn id(&self) -> &NodeId {
        &self.id
    }

    fn bounds(&self) -> RectI {
        self.bounds
    }

    fn build_display_list(&self, display_list: &mut DisplayList) {
        display_list.fill_rect(self.bounds, self.theme.background);
        for leaf in &self.layout.panes.leaves {
            display_list.fill_rect(leaf.tab_strip, self.theme.panel);
            if leaf.is_focused {
                display_list.fill_rect(
                    RectI::new(leaf.bounds.x, leaf.bounds.y, leaf.bounds.width, 2),
                    self.theme.accent,
                );
            }
        }
        for tab in &self.layout.tabs {
            display_list.fill_rect(
                tab.bounds,
                if tab.is_active {
                    self.theme.background
                } else {
                    self.theme.panel
                },
            );
            if tab.is_active {
                display_list.fill_rect(
                    RectI::new(tab.bounds.x, tab.bounds.y, tab.bounds.width, 2),
                    self.theme.accent,
                );
            }
            if let Some(close) = tab.close_bounds {
                display_list.fill_rect(close, self.theme.panel_header);
            }
        }
        for splitter in &self.layout.panes.splitters {
            display_list.fill_rect(splitter.bounds, self.theme.border());
        }
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        let mut nodes = Vec::new();
        let mut root_children = Vec::new();
        for leaf in &self.layout.panes.leaves {
            if let Ok(pane_id) = self.id.child(&leaf.pane_id.stable_key()) {
                root_children.push(pane_id.clone());
                let tab_list_id = pane_id.child("tabs").ok();
                let editor_id = pane_id.child("editor").ok();
                let mut pane_children = Vec::new();
                if let Some(tab_list_id) = &tab_list_id {
                    pane_children.push(tab_list_id.clone());
                }
                if let Some(editor_id) = &editor_id {
                    pane_children.push(editor_id.clone());
                }
                nodes.push(
                    AccessibilityNode::new(pane_id, AccessibilityRole::Group, leaf.bounds)
                        .with_label(format!("Editor pane {}", leaf.pane_id.value()))
                        .with_value(if leaf.is_focused {
                            "Focused"
                        } else {
                            "Inactive"
                        })
                        .with_children(pane_children),
                );
                if let Some(tab_list_id) = tab_list_id {
                    let tab_children = self
                        .layout
                        .tabs
                        .iter()
                        .filter(|tab| tab.pane_id == leaf.pane_id)
                        .map(|tab| tab.node_id.clone())
                        .collect();
                    nodes.push(
                        AccessibilityNode::new(
                            tab_list_id,
                            AccessibilityRole::TabList,
                            leaf.tab_strip,
                        )
                        .with_label(format!("Tabs in pane {}", leaf.pane_id.value()))
                        .with_children(tab_children),
                    );
                }
                for tab in self
                    .layout
                    .tabs
                    .iter()
                    .filter(|tab| tab.pane_id == leaf.pane_id)
                {
                    let close_children = tab.close_node_id.clone().into_iter().collect();
                    nodes.push(
                        AccessibilityNode::new(
                            tab.node_id.clone(),
                            AccessibilityRole::Tab,
                            tab.bounds,
                        )
                        .with_label(tab.title.clone())
                        .with_value(if tab.is_dirty { "Modified" } else { "Saved" })
                        .with_focused(leaf.is_focused && tab.is_active)
                        .with_children(close_children),
                    );
                    if let (Some(close_node_id), Some(close_bounds)) =
                        (tab.close_node_id.as_ref(), tab.close_bounds)
                    {
                        nodes.push(
                            AccessibilityNode::new(
                                close_node_id.clone(),
                                AccessibilityRole::Button,
                                close_bounds,
                            )
                            .with_label(format!("Close {}", tab.title)),
                        );
                    }
                }
                if let Some(editor_id) = editor_id
                    && let Some(presentation) = self
                        .state
                        .panes
                        .iter()
                        .find(|pane| pane.pane_id == leaf.pane_id)
                {
                    nodes.push(
                        AccessibilityNode::new(editor_id, AccessibilityRole::Group, leaf.editor)
                            .with_label(format!("Editor content in pane {}", leaf.pane_id.value()))
                            .with_children(vec![presentation.editor_child.clone()]),
                    );
                }
            }
        }
        for splitter in &self.layout.panes.splitters {
            if let Ok(node_id) = self.splitter_node_id(splitter.split_id) {
                root_children.push(node_id.clone());
                nodes.push(
                    AccessibilityNode::new(node_id, AccessibilityRole::Group, splitter.bounds)
                        .with_label(match splitter.axis {
                            PaneAxis::Horizontal => "Vertical pane splitter",
                            PaneAxis::Vertical => "Horizontal pane splitter",
                        })
                        .with_value("Draggable"),
                );
            }
        }
        nodes.insert(
            0,
            AccessibilityNode::new(self.id.clone(), AccessibilityRole::Group, self.bounds)
                .with_label("Editor panes")
                .with_children(root_children),
        );
        nodes
    }

    fn hit_test(&self, point: PointI) -> Option<NodeId> {
        match self.semantic_hit_test(point) {
            Some(EditorPaneSurfaceHit::CloseTab { view_id, .. }) => self
                .layout
                .tabs
                .iter()
                .find(|tab| tab.view_id == view_id)
                .and_then(|tab| tab.close_node_id.clone()),
            Some(EditorPaneSurfaceHit::Tab { view_id, .. }) => self
                .layout
                .tabs
                .iter()
                .find(|tab| tab.view_id == view_id && tab.bounds.contains(point))
                .map(|tab| tab.node_id.clone()),
            Some(EditorPaneSurfaceHit::Editor(pane_id)) => self.editor_node_id(pane_id).ok(),
            Some(EditorPaneSurfaceHit::Splitter(split_id)) => self.splitter_node_id(split_id).ok(),
            None => self.bounds.contains(point).then(|| self.id.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EditorPaneSurface, EditorPaneSurfaceHit, EditorPaneSurfaceState, PanePresentation, PaneTab,
    };
    use crate::Widget;
    use luna_core::{NodeId, PointI, RectI};
    use luna_documents::{DocumentRegistry, DocumentViewRegistry};
    use luna_panes::{PaneAxis, PaneLayoutMetrics, PaneTree};
    use luna_theme::Theme;
    use std::error::Error;

    #[test]
    fn pane_tabs_and_splitters_share_hit_geometry() -> Result<(), Box<dyn Error>> {
        let mut documents = DocumentRegistry::new();
        let one = documents.register_virtual("one", "One", 0)?;
        let two = documents.register_virtual("two", "Two", 0)?;
        let mut views = DocumentViewRegistry::new();
        let first_view = views.create_view(one);
        let second_view = views.create_view(two);
        let mut tree = PaneTree::new(first_view);
        let second_pane = tree.split_focused(PaneAxis::Horizontal, second_view);
        let first_pane = tree.leaves()[0].id();
        let surface = EditorPaneSurface::new(
            NodeId::new("panes")?,
            RectI::new(0, 0, 800, 500),
            Theme::luna_dark(),
            EditorPaneSurfaceState {
                tree,
                panes: vec![
                    PanePresentation {
                        pane_id: first_pane,
                        tabs: vec![PaneTab::new(first_view, "One")],
                        active_view: first_view,
                        editor_child: NodeId::new("text-one")?,
                    },
                    PanePresentation {
                        pane_id: second_pane,
                        tabs: vec![PaneTab::new(second_view, "Two")],
                        active_view: second_view,
                        editor_child: NodeId::new("text-two")?,
                    },
                ],
            },
            PaneLayoutMetrics::default(),
        )?;

        assert_eq!(surface.layout().panes.leaves.len(), 2);
        let first_tab = &surface.layout().tabs[0];
        assert!(matches!(
            surface.semantic_hit_test(PointI::new(first_tab.bounds.x + 2, first_tab.bounds.y + 5)),
            Some(EditorPaneSurfaceHit::Tab { .. })
        ));
        let splitter = surface.layout().panes.splitters[0];
        assert_eq!(
            surface.semantic_hit_test(PointI::new(splitter.bounds.x, splitter.bounds.y)),
            Some(EditorPaneSurfaceHit::Splitter(splitter.split_id))
        );
        assert!(surface.accessibility_nodes().len() >= 9);
        Ok(())
    }
}
