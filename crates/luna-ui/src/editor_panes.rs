// SPDX-License-Identifier: MPL-2.0

use crate::Widget;
use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{NodeId, NodeIdError, PointI, RectI};
use luna_documents::DocumentViewId;
use luna_panes::{PaneAxis, PaneId, PaneLayoutMetrics, PaneLayoutSnapshot, PaneTree};
use luna_render::DisplayList;
use luna_theme::Theme;

const PINNED_TAB_WIDTH: u32 = 44;
const REGULAR_TAB_WIDTH: u32 = 164;
const TAB_SCROLL_BUTTON_WIDTH: u32 = 22;

/// Direction in which a pane-local tab strip scrolls.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabScrollDirection {
    /// Reveal earlier regular tabs.
    Previous,
    /// Reveal later regular tabs.
    Next,
}

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
    /// Whether the tab belongs to the leading pinned partition.
    pub is_pinned: bool,
    /// Whether the tab is the pane-local transient preview.
    pub is_preview: bool,
}

impl PaneTab {
    /// Creates a closable regular pane-local tab.
    #[must_use]
    pub fn new(view_id: DocumentViewId, title: impl Into<String>) -> Self {
        Self {
            view_id,
            title: title.into(),
            is_dirty: false,
            is_closable: true,
            is_pinned: false,
            is_preview: false,
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
    /// First regular tab requested in the overflow viewport.
    pub tab_scroll_offset: usize,
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
    /// Source tab index in pane display order.
    pub tab_index: usize,
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
    /// Whether this is a pinned tab.
    pub is_pinned: bool,
    /// Whether this is a preview tab.
    pub is_preview: bool,
}

/// Shared geometry for one pane-local overflow strip.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneTabStripFrame {
    /// Owning pane.
    pub pane_id: PaneId,
    /// Complete tab strip bounds.
    pub bounds: RectI,
    /// Region used by regular tabs after the pinned partition.
    pub regular_viewport: RectI,
    /// Previous-tab button semantic identity.
    pub previous_node_id: Option<NodeId>,
    /// Previous-tab button bounds.
    pub previous_bounds: Option<RectI>,
    /// Next-tab button semantic identity.
    pub next_node_id: Option<NodeId>,
    /// Next-tab button bounds.
    pub next_bounds: Option<RectI>,
    /// Whether an earlier regular tab can be revealed.
    pub can_scroll_previous: bool,
    /// Whether a later regular tab can be revealed.
    pub can_scroll_next: bool,
    /// Effective regular-tab offset after active-tab visibility correction.
    pub effective_offset: usize,
    /// Number of visible regular tabs.
    pub visible_regular_count: usize,
}

/// Complete pane-surface geometry shared by paint, hit testing, labels, and accessibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorPaneSurfaceLayout {
    /// Recursive leaf and splitter geometry.
    pub panes: PaneLayoutSnapshot,
    /// Pane-local tab frames.
    pub tabs: Vec<PaneTabFrame>,
    /// Pane-local tab-strip overflow geometry.
    pub tab_strips: Vec<PaneTabStripFrame>,
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
    /// Pane-local overflow button.
    ScrollTabs {
        /// Owning pane.
        pane_id: PaneId,
        /// Scroll direction.
        direction: TabScrollDirection,
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
        let mut tab_strips = Vec::new();
        for leaf in &pane_layout.leaves {
            let Some(presentation) = state
                .panes
                .iter()
                .find(|presentation| presentation.pane_id == leaf.pane_id)
            else {
                continue;
            };
            let pane_node = id.child(&leaf.pane_id.stable_key())?;
            let pinned_count = presentation
                .tabs
                .iter()
                .take_while(|tab| tab.is_pinned)
                .count();
            let pinned_width = PINNED_TAB_WIDTH
                .saturating_mul(u32::try_from(pinned_count).unwrap_or(u32::MAX))
                .min(leaf.tab_strip.width);
            let regular_count = presentation.tabs.len().saturating_sub(pinned_count);
            let available_regular = leaf.tab_strip.width.saturating_sub(pinned_width);
            let overflow = REGULAR_TAB_WIDTH
                .saturating_mul(u32::try_from(regular_count).unwrap_or(u32::MAX))
                > available_regular;
            let controls_width = if overflow {
                TAB_SCROLL_BUTTON_WIDTH
                    .saturating_mul(2)
                    .min(available_regular)
            } else {
                0
            };
            let regular_width = available_regular.saturating_sub(controls_width);
            let visible_regular_count = if regular_count == 0 {
                0
            } else {
                usize::try_from((regular_width / REGULAR_TAB_WIDTH).max(1)).unwrap_or(1)
            }
            .min(regular_count);
            let max_offset = regular_count.saturating_sub(visible_regular_count);
            let mut effective_offset = presentation.tab_scroll_offset.min(max_offset);
            if let Some(active_regular) = presentation
                .tabs
                .iter()
                .skip(pinned_count)
                .position(|tab| tab.view_id == presentation.active_view)
            {
                if active_regular < effective_offset {
                    effective_offset = active_regular;
                } else if active_regular >= effective_offset.saturating_add(visible_regular_count) {
                    effective_offset = active_regular
                        .saturating_add(1)
                        .saturating_sub(visible_regular_count)
                        .min(max_offset);
                }
            }
            let controls_x = leaf.tab_strip.x.saturating_add(
                i32::try_from(leaf.tab_strip.width.saturating_sub(controls_width))
                    .unwrap_or(i32::MAX),
            );
            let previous_bounds = overflow.then_some(RectI::new(
                controls_x,
                leaf.tab_strip.y,
                TAB_SCROLL_BUTTON_WIDTH.min(controls_width),
                leaf.tab_strip.height,
            ));
            let next_width =
                controls_width.saturating_sub(TAB_SCROLL_BUTTON_WIDTH.min(controls_width));
            let next_bounds = overflow.then_some(RectI::new(
                controls_x.saturating_add(
                    i32::try_from(TAB_SCROLL_BUTTON_WIDTH.min(controls_width)).unwrap_or(i32::MAX),
                ),
                leaf.tab_strip.y,
                next_width,
                leaf.tab_strip.height,
            ));
            let previous_node_id = overflow
                .then(|| pane_node.child("tabs-scroll-previous"))
                .transpose()?;
            let next_node_id = overflow
                .then(|| pane_node.child("tabs-scroll-next"))
                .transpose()?;
            let regular_x = leaf
                .tab_strip
                .x
                .saturating_add(i32::try_from(pinned_width).unwrap_or(i32::MAX));
            let regular_viewport = RectI::new(
                regular_x,
                leaf.tab_strip.y,
                regular_width,
                leaf.tab_strip.height,
            );
            tab_strips.push(PaneTabStripFrame {
                pane_id: leaf.pane_id,
                bounds: leaf.tab_strip,
                regular_viewport,
                previous_node_id,
                previous_bounds,
                next_node_id,
                next_bounds,
                can_scroll_previous: effective_offset > 0,
                can_scroll_next: effective_offset < max_offset,
                effective_offset,
                visible_regular_count,
            });

            let mut pinned_x = leaf.tab_strip.x;
            for (tab_index, tab) in presentation.tabs.iter().take(pinned_count).enumerate() {
                let remaining =
                    u32::try_from(leaf.tab_strip.right().saturating_sub(i64::from(pinned_x)))
                        .unwrap_or(0);
                let width = PINNED_TAB_WIDTH.min(remaining);
                if width == 0 {
                    break;
                }
                tabs.push(tab_frame(
                    &pane_node,
                    leaf.pane_id,
                    tab_index,
                    tab,
                    presentation.active_view,
                    RectI::new(pinned_x, leaf.tab_strip.y, width, leaf.tab_strip.height),
                )?);
                pinned_x = pinned_x.saturating_add(i32::try_from(width).unwrap_or(i32::MAX));
            }

            let visible_start = pinned_count.saturating_add(effective_offset);
            let visible_end = visible_start
                .saturating_add(visible_regular_count)
                .min(presentation.tabs.len());
            let mut regular_x_cursor = regular_viewport.x;
            for (tab_index, tab) in presentation
                .tabs
                .iter()
                .enumerate()
                .take(visible_end)
                .skip(visible_start)
            {
                let remaining = u32::try_from(
                    regular_viewport
                        .right()
                        .saturating_sub(i64::from(regular_x_cursor)),
                )
                .unwrap_or(0);
                let width = REGULAR_TAB_WIDTH.min(remaining);
                if width == 0 {
                    break;
                }
                tabs.push(tab_frame(
                    &pane_node,
                    leaf.pane_id,
                    tab_index,
                    tab,
                    presentation.active_view,
                    RectI::new(
                        regular_x_cursor,
                        leaf.tab_strip.y,
                        width,
                        leaf.tab_strip.height,
                    ),
                )?);
                regular_x_cursor =
                    regular_x_cursor.saturating_add(i32::try_from(width).unwrap_or(i32::MAX));
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
                tab_strips,
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
        for strip in &self.layout.tab_strips {
            if strip
                .previous_bounds
                .is_some_and(|bounds| bounds.contains(point))
            {
                return Some(EditorPaneSurfaceHit::ScrollTabs {
                    pane_id: strip.pane_id,
                    direction: TabScrollDirection::Previous,
                });
            }
            if strip
                .next_bounds
                .is_some_and(|bounds| bounds.contains(point))
            {
                return Some(EditorPaneSurfaceHit::ScrollTabs {
                    pane_id: strip.pane_id,
                    direction: TabScrollDirection::Next,
                });
            }
        }
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

    /// Returns the pane and display insertion index for a tab drag position.
    #[must_use]
    pub fn tab_drop_target(&self, point: PointI) -> Option<(PaneId, usize)> {
        let strip = self
            .layout
            .tab_strips
            .iter()
            .find(|strip| strip.bounds.contains(point))?;
        let pane_tabs = self
            .layout
            .tabs
            .iter()
            .filter(|tab| tab.pane_id == strip.pane_id)
            .collect::<Vec<_>>();
        for tab in &pane_tabs {
            let midpoint = tab
                .bounds
                .x
                .saturating_add(i32::try_from(tab.bounds.width / 2).unwrap_or(0));
            if point.x < midpoint {
                return Some((strip.pane_id, tab.tab_index));
            }
        }
        let insertion_index = pane_tabs
            .last()
            .map_or(0, |tab| tab.tab_index.saturating_add(1));
        Some((strip.pane_id, insertion_index))
    }

    /// Resolves a semantic node ID into an application target.
    #[must_use]
    pub fn semantic_target(&self, node_id: &NodeId) -> Option<EditorPaneSurfaceHit> {
        self.layout
            .tab_strips
            .iter()
            .find_map(|strip| {
                if strip.previous_node_id.as_ref() == Some(node_id) {
                    Some(EditorPaneSurfaceHit::ScrollTabs {
                        pane_id: strip.pane_id,
                        direction: TabScrollDirection::Previous,
                    })
                } else if strip.next_node_id.as_ref() == Some(node_id) {
                    Some(EditorPaneSurfaceHit::ScrollTabs {
                        pane_id: strip.pane_id,
                        direction: TabScrollDirection::Next,
                    })
                } else {
                    None
                }
            })
            .or_else(|| {
                self.layout
                    .tabs
                    .iter()
                    .find(|frame| frame.close_node_id.as_ref() == Some(node_id))
                    .map(|frame| EditorPaneSurfaceHit::CloseTab {
                        pane_id: frame.pane_id,
                        view_id: frame.view_id,
                    })
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

fn tab_frame(
    pane_node: &NodeId,
    pane_id: PaneId,
    tab_index: usize,
    tab: &PaneTab,
    active_view: DocumentViewId,
    bounds: RectI,
) -> Result<PaneTabFrame, NodeIdError> {
    let tab_node = pane_node.child("tabs")?.child(&tab.view_id.stable_key())?;
    let close_node_id = if tab.is_closable {
        Some(tab_node.child("close")?)
    } else {
        None
    };
    let close_size = 14_u32.min(bounds.height.saturating_sub(8));
    let close_x = i32::try_from(
        bounds
            .right()
            .saturating_sub(i64::from(close_size.saturating_add(7))),
    )
    .unwrap_or(i32::MAX);
    let close_y = bounds
        .y
        .saturating_add(i32::try_from(bounds.height.saturating_sub(close_size) / 2).unwrap_or(0));
    Ok(PaneTabFrame {
        pane_id,
        tab_index,
        view_id: tab.view_id,
        node_id: tab_node,
        title: tab.title.clone(),
        bounds,
        close_node_id,
        close_bounds: tab
            .is_closable
            .then_some(RectI::new(close_x, close_y, close_size, close_size)),
        is_active: active_view == tab.view_id,
        is_dirty: tab.is_dirty,
        is_pinned: tab.is_pinned,
        is_preview: tab.is_preview,
    })
}

fn paint_close_glyph(display_list: &mut DisplayList, bounds: RectI, color: luna_theme::Rgba8) {
    let extent = bounds.width.min(bounds.height).min(10);
    if extent == 0 {
        return;
    }

    let stroke = extent.min(2);
    let max_offset = extent.saturating_sub(stroke);
    let origin_x = bounds
        .x
        .saturating_add(i32::try_from(bounds.width.saturating_sub(extent) / 2).unwrap_or(0));
    let origin_y = bounds
        .y
        .saturating_add(i32::try_from(bounds.height.saturating_sub(extent) / 2).unwrap_or(0));

    if max_offset == 0 {
        display_list.fill_rect(RectI::new(origin_x, origin_y, stroke, stroke), color);
        return;
    }

    for step in 0..=4_u32 {
        let forward = max_offset.saturating_mul(step) / 4;
        let backward = max_offset.saturating_sub(forward);
        let y = origin_y.saturating_add(i32::try_from(forward).unwrap_or(0));

        display_list.fill_rect(
            RectI::new(
                origin_x.saturating_add(i32::try_from(forward).unwrap_or(0)),
                y,
                stroke,
                stroke,
            ),
            color,
        );
        display_list.fill_rect(
            RectI::new(
                origin_x.saturating_add(i32::try_from(backward).unwrap_or(0)),
                y,
                stroke,
                stroke,
            ),
            color,
        );
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
            if tab.is_preview {
                display_list.fill_rect(
                    RectI::new(
                        tab.bounds.x,
                        i32::try_from(tab.bounds.bottom().saturating_sub(2))
                            .unwrap_or(tab.bounds.y),
                        tab.bounds.width,
                        2,
                    ),
                    self.theme.accent,
                );
            }
            if tab.is_pinned {
                display_list.fill_rect(
                    RectI::new(
                        tab.bounds.x.saturating_add(5),
                        tab.bounds.y.saturating_add(5),
                        4,
                        4,
                    ),
                    self.theme.accent,
                );
            }
            if let Some(close) = tab.close_bounds {
                display_list.fill_rect(close, self.theme.panel_header);
                paint_close_glyph(display_list, close, self.theme.foreground);
            }
        }
        for strip in &self.layout.tab_strips {
            if let Some(bounds) = strip.previous_bounds {
                display_list.fill_rect(bounds, self.theme.panel_header);
            }
            if let Some(bounds) = strip.next_bounds {
                display_list.fill_rect(bounds, self.theme.panel_header);
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
                let editor_id = self.editor_node_id(leaf.pane_id).ok();
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
                        .with_focused(leaf.is_focused)
                        .with_children(pane_children),
                );
                if let Some(tab_list_id) = tab_list_id {
                    let mut tab_children = self
                        .layout
                        .tabs
                        .iter()
                        .filter(|tab| tab.pane_id == leaf.pane_id)
                        .map(|tab| tab.node_id.clone())
                        .collect::<Vec<_>>();
                    if let Some(strip) = self
                        .layout
                        .tab_strips
                        .iter()
                        .find(|strip| strip.pane_id == leaf.pane_id)
                    {
                        if let Some(id) = &strip.previous_node_id {
                            tab_children.push(id.clone());
                        }
                        if let Some(id) = &strip.next_node_id {
                            tab_children.push(id.clone());
                        }
                    }
                    nodes.push(
                        AccessibilityNode::new(
                            tab_list_id,
                            AccessibilityRole::TabList,
                            leaf.tab_strip,
                        )
                        .with_label("Open tabs")
                        .with_children(tab_children),
                    );
                }
                for tab in self
                    .layout
                    .tabs
                    .iter()
                    .filter(|tab| tab.pane_id == leaf.pane_id)
                {
                    let mut label = tab.title.clone();
                    if tab.is_pinned {
                        label.push_str(", pinned");
                    }
                    if tab.is_preview {
                        label.push_str(", preview");
                    }
                    let mut children = Vec::new();
                    if let Some(close_id) = &tab.close_node_id {
                        children.push(close_id.clone());
                    }
                    nodes.push(
                        AccessibilityNode::new(
                            tab.node_id.clone(),
                            AccessibilityRole::Tab,
                            tab.bounds,
                        )
                        .with_label(label)
                        .with_value(if tab.is_dirty { "Modified" } else { "Saved" })
                        .with_focused(leaf.is_focused && tab.is_active)
                        .with_children(children),
                    );
                    if let (Some(close_id), Some(close_bounds)) =
                        (&tab.close_node_id, tab.close_bounds)
                    {
                        nodes.push(
                            AccessibilityNode::new(
                                close_id.clone(),
                                AccessibilityRole::Button,
                                close_bounds,
                            )
                            .with_label(format!("Close {}", tab.title)),
                        );
                    }
                }
                if let Some(strip) = self
                    .layout
                    .tab_strips
                    .iter()
                    .find(|strip| strip.pane_id == leaf.pane_id)
                {
                    if let (Some(id), Some(bounds)) =
                        (&strip.previous_node_id, strip.previous_bounds)
                    {
                        nodes.push(
                            AccessibilityNode::new(id.clone(), AccessibilityRole::Button, bounds)
                                .with_label("Reveal previous tabs")
                                .with_disabled(!strip.can_scroll_previous),
                        );
                    }
                    if let (Some(id), Some(bounds)) = (&strip.next_node_id, strip.next_bounds) {
                        nodes.push(
                            AccessibilityNode::new(id.clone(), AccessibilityRole::Button, bounds)
                                .with_label("Reveal next tabs")
                                .with_disabled(!strip.can_scroll_next),
                        );
                    }
                }
                if let (Some(editor_id), Some(presentation)) = (
                    editor_id,
                    self.state
                        .panes
                        .iter()
                        .find(|presentation| presentation.pane_id == leaf.pane_id),
                ) {
                    nodes.push(
                        AccessibilityNode::new(editor_id, AccessibilityRole::Group, leaf.editor)
                            .with_label("Editor content")
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
        nodes.push(
            AccessibilityNode::new(self.id.clone(), AccessibilityRole::Group, self.bounds)
                .with_label("Editor panes")
                .with_children(root_children),
        );
        nodes
    }

    fn hit_test(&self, point: PointI) -> Option<NodeId> {
        for strip in &self.layout.tab_strips {
            if let (Some(id), Some(bounds)) = (&strip.previous_node_id, strip.previous_bounds)
                && bounds.contains(point)
            {
                return Some(id.clone());
            }
            if let (Some(id), Some(bounds)) = (&strip.next_node_id, strip.next_bounds)
                && bounds.contains(point)
            {
                return Some(id.clone());
            }
        }
        self.layout
            .tabs
            .iter()
            .find_map(|tab| {
                if tab
                    .close_bounds
                    .is_some_and(|bounds| bounds.contains(point))
                {
                    tab.close_node_id.clone()
                } else if tab.bounds.contains(point) {
                    Some(tab.node_id.clone())
                } else {
                    None
                }
            })
            .or_else(|| {
                self.layout
                    .panes
                    .splitter_at(point)
                    .and_then(|splitter| self.splitter_node_id(splitter.split_id).ok())
            })
            .or_else(|| {
                self.layout
                    .panes
                    .editor_at(point)
                    .and_then(|leaf| self.editor_node_id(leaf.pane_id).ok())
            })
            .or_else(|| self.bounds.contains(point).then_some(self.id.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EditorPaneSurface, EditorPaneSurfaceHit, EditorPaneSurfaceState, PanePresentation, PaneTab,
        TabScrollDirection,
    };
    use crate::Widget;
    use luna_accessibility::AccessibilityRole;
    use luna_core::{NodeId, PointI, RectI};
    use luna_documents::{DocumentRegistry, DocumentViewRegistry};
    use luna_panes::{PaneAxis, PaneLayoutMetrics, PaneTree};
    use luna_render::{DisplayCommand, DisplayList};
    use luna_theme::Theme;
    use std::error::Error;

    type TestResult = Result<(), Box<dyn Error>>;

    fn surface_with_tabs(tab_count: usize, width: u32) -> TestResult {
        let mut documents = DocumentRegistry::new();
        let first_document = documents.register_virtual("one", "one", 0)?;
        let mut views = DocumentViewRegistry::new();
        let first = views.create_view(first_document);
        let mut tree = PaneTree::new(first);
        let pane = tree.focused_pane();
        let mut tabs = vec![PaneTab::new(first, "one")];
        for index in 1..tab_count {
            let document = documents.register_virtual(
                format!("doc-{index}"),
                format!("document-{index}"),
                0,
            )?;
            let view = views.create_view(document);
            tree.add_view(pane, view)?;
            tabs.push(PaneTab::new(view, format!("document-{index}")));
        }
        let active_view = tree.focused_view();
        let surface = EditorPaneSurface::new(
            NodeId::new("panes")?,
            RectI::new(0, 0, width, 400),
            Theme::luna_dark(),
            EditorPaneSurfaceState {
                tree,
                panes: vec![PanePresentation {
                    pane_id: pane,
                    tabs,
                    active_view,
                    tab_scroll_offset: 0,
                    editor_child: NodeId::new("text")?,
                }],
            },
            PaneLayoutMetrics::default(),
        )?;
        assert!(!surface.layout().tabs.is_empty());
        Ok(())
    }

    #[test]
    fn pane_surface_projects_tabs_splitters_and_semantics() -> TestResult {
        let mut documents = DocumentRegistry::new();
        let first_document = documents.register_virtual("one", "one", 0)?;
        let second_document = documents.register_virtual("two", "two", 0)?;
        let mut views = DocumentViewRegistry::new();
        let first = views.create_view(first_document);
        let second = views.create_view(second_document);
        let mut tree = PaneTree::new(first);
        let first_pane = tree.focused_pane();
        let second_pane = tree.split_focused(PaneAxis::Horizontal, second);
        let surface = EditorPaneSurface::new(
            NodeId::new("panes")?,
            RectI::new(0, 0, 900, 500),
            Theme::luna_dark(),
            EditorPaneSurfaceState {
                tree,
                panes: vec![
                    PanePresentation {
                        pane_id: first_pane,
                        tabs: vec![PaneTab::new(first, "one")],
                        active_view: first,
                        tab_scroll_offset: 0,
                        editor_child: NodeId::new("text-one")?,
                    },
                    PanePresentation {
                        pane_id: second_pane,
                        tabs: vec![PaneTab::new(second, "two")],
                        active_view: second,
                        tab_scroll_offset: 0,
                        editor_child: NodeId::new("text-two")?,
                    },
                ],
            },
            PaneLayoutMetrics::default(),
        )?;
        assert_eq!(surface.layout().panes.leaves.len(), 2);
        assert_eq!(surface.layout().panes.splitters.len(), 1);
        assert_eq!(surface.layout().tabs.len(), 2);
        assert!(surface.accessibility_nodes().iter().any(|node| {
            node.role == AccessibilityRole::Group && node.value.as_deref() == Some("Draggable")
        }));
        let tab = &surface.layout().tabs[0];
        assert_eq!(
            surface.semantic_hit_test(PointI::new(tab.bounds.x + 2, tab.bounds.y + 2)),
            Some(EditorPaneSurfaceHit::Tab {
                pane_id: tab.pane_id,
                view_id: tab.view_id,
            })
        );
        Ok(())
    }

    #[test]
    fn overflowing_tabs_expose_controls_and_keep_active_visible() -> TestResult {
        let mut documents = DocumentRegistry::new();
        let first_document = documents.register_virtual("one", "one", 0)?;
        let mut views = DocumentViewRegistry::new();
        let first = views.create_view(first_document);
        let mut tree = PaneTree::new(first);
        let pane = tree.focused_pane();
        let mut tabs = vec![PaneTab::new(first, "one")];
        for index in 1..7 {
            let document =
                documents.register_virtual(format!("d{index}"), format!("d{index}"), 0)?;
            let view = views.create_view(document);
            tree.add_view(pane, view)?;
            tabs.push(PaneTab::new(view, format!("document-{index}")));
        }
        let active = tree.focused_view();
        let surface = EditorPaneSurface::new(
            NodeId::new("panes")?,
            RectI::new(0, 0, 360, 300),
            Theme::luna_dark(),
            EditorPaneSurfaceState {
                tree,
                panes: vec![PanePresentation {
                    pane_id: pane,
                    tabs,
                    active_view: active,
                    tab_scroll_offset: 0,
                    editor_child: NodeId::new("text")?,
                }],
            },
            PaneLayoutMetrics::default(),
        )?;
        let strip = &surface.layout().tab_strips[0];
        assert!(strip.next_bounds.is_some());
        assert!(
            surface
                .layout()
                .tabs
                .iter()
                .any(|tab| tab.view_id == active)
        );
        let point = strip.next_bounds.ok_or("missing next button")?;
        assert_eq!(
            surface.semantic_hit_test(PointI::new(point.x + 1, point.y + 1)),
            Some(EditorPaneSurfaceHit::ScrollTabs {
                pane_id: pane,
                direction: TabScrollDirection::Next,
            })
        );
        Ok(())
    }

    #[test]
    fn pinned_tabs_remain_visible_before_regular_overflow() -> TestResult {
        let mut documents = DocumentRegistry::new();
        let first_document = documents.register_virtual("one", "one", 0)?;
        let second_document = documents.register_virtual("two", "two", 0)?;
        let mut views = DocumentViewRegistry::new();
        let first = views.create_view(first_document);
        let second = views.create_view(second_document);
        let mut tree = PaneTree::new(first);
        let pane = tree.focused_pane();
        tree.add_view(pane, second)?;
        tree.pin_view(pane, first)?;
        let mut first_tab = PaneTab::new(first, "one");
        first_tab.is_pinned = true;
        let surface = EditorPaneSurface::new(
            NodeId::new("panes")?,
            RectI::new(0, 0, 180, 300),
            Theme::luna_dark(),
            EditorPaneSurfaceState {
                tree,
                panes: vec![PanePresentation {
                    pane_id: pane,
                    tabs: vec![first_tab, PaneTab::new(second, "two")],
                    active_view: second,
                    tab_scroll_offset: 0,
                    editor_child: NodeId::new("text")?,
                }],
            },
            PaneLayoutMetrics::default(),
        )?;
        assert!(
            surface
                .layout()
                .tabs
                .iter()
                .any(|tab| tab.view_id == first && tab.is_pinned)
        );
        Ok(())
    }

    #[test]
    fn closable_tab_paints_foreground_close_glyph() -> TestResult {
        let mut documents = DocumentRegistry::new();
        let document = documents.register_virtual("one", "one", 0)?;
        let mut views = DocumentViewRegistry::new();
        let view = views.create_view(document);
        let tree = PaneTree::new(view);
        let pane = tree.focused_pane();
        let theme = Theme::luna_dark();
        let surface = EditorPaneSurface::new(
            NodeId::new("panes")?,
            RectI::new(0, 0, 320, 240),
            theme,
            EditorPaneSurfaceState {
                tree,
                panes: vec![PanePresentation {
                    pane_id: pane,
                    tabs: vec![PaneTab::new(view, "one")],
                    active_view: view,
                    tab_scroll_offset: 0,
                    editor_child: NodeId::new("text")?,
                }],
            },
            PaneLayoutMetrics::default(),
        )?;
        let close_bounds = surface.layout().tabs[0]
            .close_bounds
            .ok_or("missing close bounds")?;

        let mut display_list = DisplayList::new();
        surface.build_display_list(&mut display_list);

        let glyph_rectangles = display_list
            .commands()
            .iter()
            .filter_map(|command| match command {
                DisplayCommand::FillRect { bounds, color }
                    if *color == theme.foreground
                        && bounds.x >= close_bounds.x
                        && bounds.y >= close_bounds.y
                        && bounds.right() <= close_bounds.right()
                        && bounds.bottom() <= close_bounds.bottom() =>
                {
                    Some(bounds)
                }
                _ => None,
            })
            .count();

        assert!(
            glyph_rectangles >= 8,
            "expected a visible geometric X inside the close button"
        );
        Ok(())
    }

    #[test]
    fn tab_drop_targets_use_display_indices() -> TestResult {
        surface_with_tabs(3, 640)?;
        let mut documents = DocumentRegistry::new();
        let first_document = documents.register_virtual("one", "one", 0)?;
        let second_document = documents.register_virtual("two", "two", 0)?;
        let mut views = DocumentViewRegistry::new();
        let first = views.create_view(first_document);
        let second = views.create_view(second_document);
        let mut tree = PaneTree::new(first);
        let pane = tree.focused_pane();
        tree.add_view(pane, second)?;
        let surface = EditorPaneSurface::new(
            NodeId::new("panes")?,
            RectI::new(0, 0, 640, 300),
            Theme::luna_dark(),
            EditorPaneSurfaceState {
                tree,
                panes: vec![PanePresentation {
                    pane_id: pane,
                    tabs: vec![PaneTab::new(first, "one"), PaneTab::new(second, "two")],
                    active_view: second,
                    tab_scroll_offset: 0,
                    editor_child: NodeId::new("text")?,
                }],
            },
            PaneLayoutMetrics::default(),
        )?;
        let first_frame = &surface.layout().tabs[0];
        assert_eq!(
            surface.tab_drop_target(PointI::new(
                first_frame.bounds.x + 1,
                first_frame.bounds.y + 1
            )),
            Some((pane, 0))
        );
        Ok(())
    }
}
