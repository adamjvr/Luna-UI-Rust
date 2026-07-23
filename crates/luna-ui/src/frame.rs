// SPDX-License-Identifier: MPL-2.0

use crate::Widget;
use luna_accessibility::{AccessibilityTree, AccessibilityTreeError};
use luna_render::DisplayList;
use luna_theme::Rgba8;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Immutable product-neutral snapshot consumed by host adapters and renderers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiFrame {
    /// Ordered backend-neutral paint operations.
    pub display_list: DisplayList,
    /// Validated semantic accessibility tree.
    pub accessibility_tree: AccessibilityTree,
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
        let accessibility_tree = AccessibilityTree::new(root, nodes)?;
        Ok(Self {
            display_list,
            accessibility_tree,
        })
    }

    /// Builds one frame from a root widget.
    pub fn build(root: &impl Widget, clear_color: Rgba8) -> Result<Self, UiFrameError> {
        let mut display_list = DisplayList::new();
        display_list.clear(clear_color);
        root.build_display_list(&mut display_list);

        let accessibility_tree =
            AccessibilityTree::new(root.id().clone(), root.accessibility_nodes())?;

        Ok(Self {
            display_list,
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
