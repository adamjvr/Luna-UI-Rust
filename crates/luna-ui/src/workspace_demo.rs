// SPDX-License-Identifier: MPL-2.0

use crate::Widget;
use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{InsetsI, NodeId, NodeIdError, PointI, RectI, SizeI};
use luna_layout::{
    Axis, CrossAlignment, LinearItem, LinearLayout, SplitLayout, StackItem, StackLayout,
};
use luna_render::DisplayList;
use luna_theme::Theme;

/// Mutable visual state supplied to the M1 workspace proof widget.
///
/// The state is intentionally tiny. It exists to prove that native input and typed commands can
/// update a deterministic widget tree without putting application callbacks inside Luna widgets.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceDemoState {
    /// Whether the sidebar and command button use the active accent treatment.
    pub sidebar_is_accented: bool,
    /// Optional semantic node that owns keyboard focus.
    pub focused_node: Option<NodeId>,
}

/// Complete geometry shared by paint, hit testing, and accessibility for [`WorkspaceDemo`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceDemoLayout {
    /// Top title/toolbar strip.
    pub header: RectI,
    /// Main body below the header.
    pub body: RectI,
    /// Left project/sidebar pane.
    pub sidebar: RectI,
    /// Draggable split divider proof surface.
    pub divider: RectI,
    /// Main editor/work area.
    pub editor: RectI,
    /// Command proof button inside the sidebar.
    pub command_button: RectI,
    /// Small stacked overlay in the editor.
    pub editor_overlay: RectI,
    /// Bottom status strip.
    pub status: RectI,
}

/// M1 composite editor-shell proof widget.
///
/// This is not Moth Text product UI. It is a product-neutral executable fixture demonstrating how
/// Luna's row/column, split, and stack layouts produce one immutable geometry snapshot that drives
/// paint, hit testing, and accessibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceDemo {
    id: NodeId,
    bounds: RectI,
    theme: Theme,
    state: WorkspaceDemoState,
    header_id: NodeId,
    body_id: NodeId,
    sidebar_id: NodeId,
    editor_id: NodeId,
    command_button_id: NodeId,
    status_id: NodeId,
    layout: WorkspaceDemoLayout,
}

impl WorkspaceDemo {
    /// Creates the M1 workspace proof widget and calculates its complete geometry snapshot.
    pub fn new(
        id: NodeId,
        bounds: RectI,
        theme: Theme,
        state: WorkspaceDemoState,
    ) -> Result<Self, NodeIdError> {
        let header_id = id.child("header")?;
        let body_id = id.child("body")?;
        let sidebar_id = id.child("sidebar")?;
        let editor_id = id.child("editor")?;
        let command_button_id = sidebar_id.child("command")?;
        let status_id = id.child("status")?;
        let layout = calculate_layout(
            bounds,
            &header_id,
            &body_id,
            &sidebar_id,
            &editor_id,
            &command_button_id,
            &status_id,
        );

        Ok(Self {
            id,
            bounds,
            theme,
            state,
            header_id,
            body_id,
            sidebar_id,
            editor_id,
            command_button_id,
            status_id,
            layout,
        })
    }

    /// Returns the immutable layout snapshot used by every widget lane.
    #[must_use]
    pub const fn layout(&self) -> WorkspaceDemoLayout {
        self.layout
    }

    /// Returns the semantic ID of the command proof button.
    #[must_use]
    pub const fn command_button_id(&self) -> &NodeId {
        &self.command_button_id
    }
}

impl Widget for WorkspaceDemo {
    fn id(&self) -> &NodeId {
        &self.id
    }

    fn bounds(&self) -> RectI {
        self.bounds
    }

    fn build_display_list(&self, display_list: &mut DisplayList) {
        display_list.fill_rect(self.layout.header, self.theme.panel_header);
        display_list.fill_rect(
            self.layout.sidebar,
            if self.state.sidebar_is_accented {
                self.theme.accent
            } else {
                self.theme.panel_header
            },
        );
        display_list.fill_rect(self.layout.divider, self.theme.background);
        display_list.fill_rect(self.layout.editor, self.theme.panel);
        display_list.fill_rect(
            self.layout.command_button,
            if self.state.sidebar_is_accented {
                self.theme.panel
            } else {
                self.theme.accent
            },
        );
        display_list.fill_rect(self.layout.editor_overlay, self.theme.accent);
        display_list.fill_rect(self.layout.status, self.theme.panel_header);
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        let command_is_focused = self.state.focused_node.as_ref() == Some(&self.command_button_id);
        vec![
            AccessibilityNode::new(self.id.clone(), AccessibilityRole::Window, self.bounds)
                .with_label("Luna UI Rust M1 native workspace")
                .with_children(vec![
                    self.header_id.clone(),
                    self.body_id.clone(),
                    self.status_id.clone(),
                ]),
            AccessibilityNode::new(
                self.header_id.clone(),
                AccessibilityRole::Label,
                self.layout.header,
            )
            .with_label("Luna UI Rust M1"),
            AccessibilityNode::new(
                self.body_id.clone(),
                AccessibilityRole::Group,
                self.layout.body,
            )
            .with_label("Workspace")
            .with_children(vec![self.sidebar_id.clone(), self.editor_id.clone()]),
            AccessibilityNode::new(
                self.sidebar_id.clone(),
                AccessibilityRole::List,
                self.layout.sidebar,
            )
            .with_label("Project sidebar")
            .with_children(vec![self.command_button_id.clone()]),
            AccessibilityNode::new(
                self.command_button_id.clone(),
                AccessibilityRole::Button,
                self.layout.command_button,
            )
            .with_label("Toggle sidebar accent, Control P")
            .with_focused(command_is_focused),
            AccessibilityNode::new(
                self.editor_id.clone(),
                AccessibilityRole::Group,
                self.layout.editor,
            )
            .with_label("Editor surface proof"),
            AccessibilityNode::new(
                self.status_id.clone(),
                AccessibilityRole::Label,
                self.layout.status,
            )
            .with_label("M1 native host ready"),
        ]
    }

    fn hit_test(&self, point: PointI) -> Option<NodeId> {
        // Front-most and most-specific geometry is checked first. This order mirrors paint order
        // and makes the hit-test contract explicit instead of relying on incidental tree order.
        if self.layout.command_button.contains(point) {
            return Some(self.command_button_id.clone());
        }
        if self.layout.sidebar.contains(point) {
            return Some(self.sidebar_id.clone());
        }
        if self.layout.editor.contains(point) {
            return Some(self.editor_id.clone());
        }
        if self.layout.status.contains(point) {
            return Some(self.status_id.clone());
        }
        if self.layout.header.contains(point) {
            return Some(self.header_id.clone());
        }
        self.bounds.contains(point).then(|| self.id.clone())
    }
}

#[allow(clippy::too_many_arguments)]
fn calculate_layout(
    bounds: RectI,
    header_id: &NodeId,
    body_id: &NodeId,
    sidebar_id: &NodeId,
    editor_id: &NodeId,
    command_button_id: &NodeId,
    status_id: &NodeId,
) -> WorkspaceDemoLayout {
    let shell = LinearLayout {
        axis: Axis::Vertical,
        bounds,
        padding: InsetsI::default(),
        gap: 1,
        cross_alignment: CrossAlignment::Stretch,
        items: vec![
            LinearItem::fixed(header_id.clone(), 46),
            LinearItem::flex(body_id.clone(), 1),
            LinearItem::fixed(status_id.clone(), 26),
        ],
    }
    .calculate();
    let header = shell.bounds(header_id).unwrap_or_default();
    let body = shell.bounds(body_id).unwrap_or_default();
    let status = shell.bounds(status_id).unwrap_or_default();

    let split = SplitLayout {
        bounds: body,
        axis: Axis::Horizontal,
        ratio_per_mille: 245,
        divider_extent: 3,
        minimum_first: 180,
        minimum_second: 280,
    }
    .calculate();

    let sidebar_column = LinearLayout {
        axis: Axis::Vertical,
        bounds: split.first,
        padding: InsetsI::symmetric(12, 12),
        gap: 10,
        cross_alignment: CrossAlignment::Stretch,
        items: vec![
            LinearItem::fixed(sidebar_id.clone(), 28),
            LinearItem::fixed(command_button_id.clone(), 42),
            LinearItem::flex(editor_id.clone(), 1),
        ],
    }
    .calculate();
    let command_button = sidebar_column.bounds(command_button_id).unwrap_or_default();

    let editor_stack = StackLayout {
        bounds: split.second,
        padding: InsetsI::symmetric(16, 16),
        items: vec![
            StackItem {
                id: editor_id.clone(),
                size: None,
                horizontal: CrossAlignment::Stretch,
                vertical: CrossAlignment::Stretch,
            },
            StackItem {
                id: command_button_id.clone(),
                size: Some(SizeI::new(184, 34)),
                horizontal: CrossAlignment::End,
                vertical: CrossAlignment::Start,
            },
        ],
    }
    .calculate();
    let editor_overlay = editor_stack
        .frames()
        .last()
        .map_or(RectI::default(), |frame| frame.bounds);

    WorkspaceDemoLayout {
        header,
        body,
        sidebar: split.first,
        divider: split.divider,
        editor: split.second,
        command_button,
        editor_overlay,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkspaceDemo, WorkspaceDemoState};
    use crate::{UiFrame, Widget};
    use luna_core::{NodeId, PointI, RectI};
    use luna_theme::Theme;
    use std::error::Error;

    #[test]
    fn composite_geometry_drives_all_three_lanes() -> Result<(), Box<dyn Error>> {
        let widget = WorkspaceDemo::new(
            NodeId::new("workspace")?,
            RectI::new(0, 0, 960, 540),
            Theme::luna_dark(),
            WorkspaceDemoState::default(),
        )?;
        let frame = UiFrame::build(&widget, Theme::luna_dark().background)?;
        let button = widget.layout().command_button;
        let point = PointI::new(button.x.saturating_add(1), button.y.saturating_add(1));

        assert_eq!(
            widget.hit_test(point),
            Some(widget.command_button_id().clone())
        );
        assert_eq!(
            frame
                .accessibility_tree
                .node(widget.command_button_id())
                .map(|node| node.bounds),
            Some(button)
        );
        assert!(frame.display_list.commands().len() >= 8);
        Ok(())
    }

    #[test]
    fn minimum_panes_remain_inside_small_window() -> Result<(), Box<dyn Error>> {
        let widget = WorkspaceDemo::new(
            NodeId::new("workspace")?,
            RectI::new(0, 0, 320, 180),
            Theme::luna_dark(),
            WorkspaceDemoState::default(),
        )?;
        let layout = widget.layout();

        assert_eq!(layout.sidebar.right(), i64::from(layout.divider.x));
        assert_eq!(layout.editor.right(), 320);
        assert!(layout.editor.bottom() <= 180);
        Ok(())
    }
}
