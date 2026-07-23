// SPDX-License-Identifier: MPL-2.0

//! Platform-neutral accessibility semantics.
//!
//! Widgets describe meaning here at the same time they describe paint and hit testing. Host
//! adapters will translate a validated tree into AccessKit and native platform accessibility APIs.

use luna_core::{NodeId, RectI};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Product-neutral semantic role.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AccessibilityRole {
    /// Application/window root.
    Window,
    /// Generic grouping container.
    Group,
    /// Static text label.
    Label,
    /// Pressable control.
    Button,
    /// Single-line editable text input.
    TextField,
    /// Multi-line editable text area.
    TextArea,
    /// List container.
    List,
    /// List item.
    ListItem,
}

/// UTF-8 byte range exposed by a text-bearing semantic node.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct AccessibilityTextRange {
    /// Absolute UTF-8 byte offset.
    pub utf8_offset: usize,
    /// UTF-8 byte length.
    pub utf8_length: usize,
}

impl AccessibilityTextRange {
    /// Creates a UTF-8 accessibility range.
    #[must_use]
    pub const fn new(utf8_offset: usize, utf8_length: usize) -> Self {
        Self {
            utf8_offset,
            utf8_length,
        }
    }
}

/// One semantic node in an accessibility tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessibilityNode {
    /// Stable ID shared with the widget/hit-testing layers.
    pub id: NodeId,
    /// Semantic role.
    pub role: AccessibilityRole,
    /// Human-readable accessible name.
    pub label: Option<String>,
    /// Textual value for text-bearing controls.
    pub value: Option<String>,
    /// Bounds in Luna logical coordinates.
    pub bounds: RectI,
    /// Child IDs in semantic traversal order.
    pub children: Vec<NodeId>,
    /// Whether this node is disabled.
    pub is_disabled: bool,
    /// Whether this node currently owns keyboard focus.
    pub is_focused: bool,
    /// Whether a text-bearing node permits edits.
    pub is_editable: bool,
    /// Complete text range represented by this node.
    pub text_range: Option<AccessibilityTextRange>,
    /// Focused insertion range, normally zero length.
    pub caret_range: Option<AccessibilityTextRange>,
    /// Current selected range.
    pub selected_range: Option<AccessibilityTextRange>,
    /// Portion of the complete text range currently visible.
    pub visible_range: Option<AccessibilityTextRange>,
}

impl AccessibilityNode {
    /// Creates a semantic node with no children or state flags.
    #[must_use]
    pub fn new(id: NodeId, role: AccessibilityRole, bounds: RectI) -> Self {
        Self {
            id,
            role,
            label: None,
            value: None,
            bounds,
            children: Vec::new(),
            is_disabled: false,
            is_focused: false,
            is_editable: false,
            text_range: None,
            caret_range: None,
            selected_range: None,
            visible_range: None,
        }
    }

    /// Sets the accessible label using builder syntax.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the textual value using builder syntax.
    #[must_use]
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Sets child traversal order using builder syntax.
    #[must_use]
    pub fn with_children(mut self, children: Vec<NodeId>) -> Self {
        self.children = children;
        self
    }

    /// Sets the disabled state using builder syntax.
    #[must_use]
    pub const fn with_disabled(mut self, is_disabled: bool) -> Self {
        self.is_disabled = is_disabled;
        self
    }

    /// Sets keyboard-focus ownership using builder syntax.
    #[must_use]
    pub const fn with_focused(mut self, is_focused: bool) -> Self {
        self.is_focused = is_focused;
        self
    }

    /// Sets editable text state using builder syntax.
    #[must_use]
    pub const fn with_editable(mut self, is_editable: bool) -> Self {
        self.is_editable = is_editable;
        self
    }

    /// Sets complete, caret, selected, and visible text ranges.
    #[must_use]
    pub const fn with_text_ranges(
        mut self,
        text_range: Option<AccessibilityTextRange>,
        caret_range: Option<AccessibilityTextRange>,
        selected_range: Option<AccessibilityTextRange>,
        visible_range: Option<AccessibilityTextRange>,
    ) -> Self {
        self.text_range = text_range;
        self.caret_range = caret_range;
        self.selected_range = selected_range;
        self.visible_range = visible_range;
        self
    }
}

/// A validated semantic tree snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessibilityTree {
    root: NodeId,
    nodes: BTreeMap<NodeId, AccessibilityNode>,
}

impl AccessibilityTree {
    /// Builds and validates a semantic tree.
    ///
    /// Validation catches duplicate IDs, missing children, and cycles before a host adapter sees
    /// the data. This turns accessibility integrity into a deterministic unit-testable invariant.
    pub fn new(
        root: NodeId,
        nodes: impl IntoIterator<Item = AccessibilityNode>,
    ) -> Result<Self, AccessibilityTreeError> {
        let mut by_id = BTreeMap::new();
        for node in nodes {
            let id = node.id.clone();
            if by_id.insert(id.clone(), node).is_some() {
                return Err(AccessibilityTreeError::DuplicateNode(id));
            }
        }

        if !by_id.contains_key(&root) {
            return Err(AccessibilityTreeError::MissingRoot(root));
        }

        for node in by_id.values() {
            for child in &node.children {
                if !by_id.contains_key(child) {
                    return Err(AccessibilityTreeError::MissingChild {
                        parent: node.id.clone(),
                        child: child.clone(),
                    });
                }
            }
        }

        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        validate_acyclic(&root, &by_id, &mut visiting, &mut visited)?;

        Ok(Self { root, nodes: by_id })
    }

    /// Returns the root node ID.
    #[must_use]
    pub const fn root(&self) -> &NodeId {
        &self.root
    }

    /// Looks up a node by stable ID.
    #[must_use]
    pub fn node(&self, id: &NodeId) -> Option<&AccessibilityNode> {
        self.nodes.get(id)
    }

    /// Iterates nodes in stable identifier order.
    pub fn nodes(&self) -> impl Iterator<Item = &AccessibilityNode> {
        self.nodes.values()
    }
}

fn validate_acyclic(
    id: &NodeId,
    nodes: &BTreeMap<NodeId, AccessibilityNode>,
    visiting: &mut BTreeSet<NodeId>,
    visited: &mut BTreeSet<NodeId>,
) -> Result<(), AccessibilityTreeError> {
    if visited.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.clone()) {
        return Err(AccessibilityTreeError::Cycle(id.clone()));
    }

    if let Some(node) = nodes.get(id) {
        for child in &node.children {
            validate_acyclic(child, nodes, visiting, visited)?;
        }
    }

    visiting.remove(id);
    visited.insert(id.clone());
    Ok(())
}

/// Validation failures for an accessibility snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessibilityTreeError {
    /// More than one node used the same stable ID.
    DuplicateNode(NodeId),
    /// The requested root ID did not exist.
    MissingRoot(NodeId),
    /// A parent referenced a child ID that did not exist.
    MissingChild {
        /// Parent containing the invalid reference.
        parent: NodeId,
        /// Missing child ID.
        child: NodeId,
    },
    /// Child references formed a cycle.
    Cycle(NodeId),
}

impl Display for AccessibilityTreeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateNode(id) => write!(formatter, "duplicate accessibility node: {id}"),
            Self::MissingRoot(id) => write!(formatter, "missing accessibility root: {id}"),
            Self::MissingChild { parent, child } => {
                write!(
                    formatter,
                    "accessibility node {parent} references missing child {child}"
                )
            }
            Self::Cycle(id) => write!(formatter, "accessibility tree contains a cycle at {id}"),
        }
    }
}

impl Error for AccessibilityTreeError {}

#[cfg(test)]
mod tests {
    use super::{AccessibilityNode, AccessibilityRole, AccessibilityTree};
    use luna_core::{NodeId, RectI};
    use std::error::Error;

    #[test]
    fn a_simple_tree_validates() -> Result<(), Box<dyn Error>> {
        let root = NodeId::new("root")?;
        let child = root.child("child")?;
        let nodes = vec![
            AccessibilityNode::new(root.clone(), AccessibilityRole::Window, RectI::default())
                .with_children(vec![child.clone()]),
            AccessibilityNode::new(child, AccessibilityRole::Label, RectI::default()),
        ];

        let tree = AccessibilityTree::new(root.clone(), nodes)?;
        assert_eq!(tree.root(), &root);
        Ok(())
    }
}
