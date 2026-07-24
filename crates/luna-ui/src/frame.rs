// SPDX-License-Identifier: MPL-2.0

use crate::Widget;
use luna_accessibility::{AccessibilityTree, AccessibilityTreeError};
use luna_core::RectI;
use luna_render::DisplayList;
use luna_theme::Rgba8;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

/// One retained static paint layer reused across related frames.
///
/// The native host rasterizes this display list only when its revision, framebuffer dimensions, or
/// scale factor changes. On later frames it restores [`Self::dirty_bounds`] from the cached static
/// framebuffer before painting the frame's ordinary dynamic display list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedDisplayList {
    /// Application-owned revision for the complete static scene.
    pub revision: u64,
    /// Immutable static paint operations shared by related frames.
    pub display_list: Arc<DisplayList>,
    /// Logical region that dynamic paint may modify and must restore before the next sample.
    pub dirty_bounds: RectI,
}

impl RetainedDisplayList {
    /// Creates a retained static display-list snapshot.
    #[must_use]
    pub fn new(revision: u64, display_list: Arc<DisplayList>, dirty_bounds: RectI) -> Self {
        Self {
            revision,
            display_list,
            dirty_bounds,
        }
    }
}

/// Immutable product-neutral snapshot consumed by host adapters and renderers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiFrame {
    /// Ordered backend-neutral paint operations for this frame.
    ///
    /// When [`Self::retained_display_list`] is present, these commands are the dynamic layer painted
    /// after the host restores the retained layer's dirty region.
    pub display_list: DisplayList,
    /// Optional retained static paint layer.
    pub retained_display_list: Option<RetainedDisplayList>,
    /// Validated semantic accessibility tree shared across paint-only frames.
    pub accessibility_tree: Arc<AccessibilityTree>,
}

impl UiFrame {
    /// Builds one frame from preassembled paint and semantic parts.
    ///
    /// Complex applications often compose several sibling widgets—editor shell, text view, and
    /// transient overlays—under one application-owned window node. This constructor preserves the
    /// same accessibility validation as [`Self::build`] without forcing those siblings into a
    /// retained trait-object tree.
    pub fn from_parts(
        display_list: DisplayList,
        root: luna_core::NodeId,
        nodes: impl IntoIterator<Item = luna_accessibility::AccessibilityNode>,
    ) -> Result<Self, UiFrameError> {
        let accessibility_tree = Arc::new(AccessibilityTree::new(root, nodes)?);
        Ok(Self {
            display_list,
            retained_display_list: None,
            accessibility_tree,
        })
    }

    /// Builds a frame from a retained static layer, a dynamic layer, and validated semantics.
    ///
    /// Applications use this constructor after caching scene geometry and accessibility snapshots.
    /// The retained revision must change whenever any static paint command changes.
    #[must_use]
    pub fn from_retained_snapshots(
        retained_display_list: RetainedDisplayList,
        dynamic_display_list: DisplayList,
        accessibility_tree: Arc<AccessibilityTree>,
    ) -> Self {
        Self {
            display_list: dynamic_display_list,
            retained_display_list: Some(retained_display_list),
            accessibility_tree,
        }
    }

    /// Builds one frame from a root widget.
    pub fn build(root: &impl Widget, clear_color: Rgba8) -> Result<Self, UiFrameError> {
        let mut display_list = DisplayList::new();
        display_list.clear(clear_color);
        root.build_display_list(&mut display_list);

        let accessibility_tree = Arc::new(AccessibilityTree::new(
            root.id().clone(),
            root.accessibility_nodes(),
        )?);

        Ok(Self {
            display_list,
            retained_display_list: None,
            accessibility_tree,
        })
    }
}

/// Errors produced while assembling an immutable UI frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiFrameError {
    /// Accessibility semantics did not form a valid tree.
    Accessibility(AccessibilityTreeError),
}

impl Display for UiFrameError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Accessibility(error) => write!(formatter, "invalid accessibility tree: {error}"),
        }
    }
}

impl Error for UiFrameError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Accessibility(error) => Some(error),
        }
    }
}

impl From<AccessibilityTreeError> for UiFrameError {
    fn from(value: AccessibilityTreeError) -> Self {
        Self::Accessibility(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{RetainedDisplayList, UiFrame};
    use luna_accessibility::{AccessibilityNode, AccessibilityRole, AccessibilityTree};
    use luna_core::{NodeId, RectI};
    use luna_render::DisplayList;
    use std::error::Error;
    use std::sync::Arc;

    #[test]
    fn retained_frame_shares_static_paint_and_semantics() -> Result<(), Box<dyn Error>> {
        let root = NodeId::new("root")?;
        let tree = Arc::new(AccessibilityTree::new(
            root.clone(),
            [AccessibilityNode::new(
                root,
                AccessibilityRole::Window,
                RectI::new(0, 0, 100, 80),
            )],
        )?);
        let static_list = Arc::new(DisplayList::new());
        let frame = UiFrame::from_retained_snapshots(
            RetainedDisplayList::new(7, Arc::clone(&static_list), RectI::new(10, 10, 20, 20)),
            DisplayList::new(),
            Arc::clone(&tree),
        );

        assert_eq!(
            frame
                .retained_display_list
                .as_ref()
                .map(|retained| retained.revision),
            Some(7)
        );
        assert!(Arc::ptr_eq(&frame.accessibility_tree, &tree));
        assert!(Arc::ptr_eq(
            &frame
                .retained_display_list
                .as_ref()
                .ok_or_else(|| std::io::Error::other("retained layer missing"))?
                .display_list,
            &static_list
        ));
        Ok(())
    }
}
