// SPDX-License-Identifier: MPL-2.0

//! Platform-neutral accessibility semantics.
//!
//! Widgets describe meaning here at the same time they describe paint and hit testing. Host
//! adapters will translate a validated tree into AccessKit and native platform accessibility APIs.

use luna_core::{CodedError, ErrorCode, NodeId, RectI};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Product-neutral semantic role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    /// Binary on/off control.
    CheckBox,
    /// Progress or completion indicator.
    ProgressIndicator,
    /// Menu bar container.
    MenuBar,
    /// Popup or drop-down menu container.
    Menu,
    /// Activatable menu entry.
    MenuItem,
    /// Tab-strip container.
    TabList,
    /// Individual document or panel tab.
    Tab,
    /// Hierarchical tree container.
    Tree,
    /// Hierarchical tree row.
    TreeItem,
    /// Application status region.
    Status,
    /// Modal or modeless dialog surface.
    Dialog,
}

/// Product-neutral assistive-technology action exposed by a semantic node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AccessibilityAction {
    /// Activate the control's default behavior.
    Click,
    /// Move semantic and keyboard focus to the node.
    Focus,
    /// Replace the currently selected editable text.
    ReplaceSelectedText,
    /// Change the editable text selection.
    SetTextSelection,
    /// Replace the complete value exposed by the node.
    SetValue,
    /// Open the node's context menu.
    ShowContextMenu,
    /// Increment a range-like value.
    Increment,
    /// Decrement a range-like value.
    Decrement,
}

/// UTF-8 byte range exposed by a text-bearing semantic node.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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
    /// Explicit actions native assistive technology may request.
    pub actions: Vec<AccessibilityAction>,
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
            actions: Vec::new(),
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

    /// Sets the explicit assistive-technology actions supported by this node.
    #[must_use]
    pub fn with_actions(mut self, actions: impl IntoIterator<Item = AccessibilityAction>) -> Self {
        let mut actions = actions.into_iter().collect::<Vec<_>>();
        actions.sort_unstable();
        actions.dedup();
        self.actions = actions;
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
    fingerprint: u64,
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

        let fingerprint = accessibility_fingerprint(&root, &by_id);
        Ok(Self {
            root,
            nodes: by_id,
            fingerprint,
        })
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

    /// Returns a deterministic fingerprint of every semantic field in the validated snapshot.
    ///
    /// Hosts use this value to avoid rebuilding native accessibility trees when paint changes but
    /// semantics remain identical. Node storage is ordered by stable [`NodeId`], so equivalent trees
    /// produce the same value regardless of the order in which nodes were supplied to [`Self::new`].
    #[must_use]
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

fn accessibility_fingerprint(root: &NodeId, nodes: &BTreeMap<NodeId, AccessibilityNode>) -> u64 {
    let mut fingerprint = StableFingerprint::new();
    fingerprint.write_node_id(root);
    fingerprint.write_usize(nodes.len());
    for (id, node) in nodes {
        fingerprint.write_node_id(id);
        fingerprint.write_u8(accessibility_role_tag(node.role));
        fingerprint.write_optional_string(node.label.as_deref());
        fingerprint.write_optional_string(node.value.as_deref());
        fingerprint.write_rect(node.bounds);
        fingerprint.write_usize(node.children.len());
        for child in &node.children {
            fingerprint.write_node_id(child);
        }
        fingerprint.write_bool(node.is_disabled);
        fingerprint.write_bool(node.is_focused);
        fingerprint.write_bool(node.is_editable);
        fingerprint.write_optional_text_range(node.text_range);
        fingerprint.write_optional_text_range(node.caret_range);
        fingerprint.write_optional_text_range(node.selected_range);
        fingerprint.write_optional_text_range(node.visible_range);
        fingerprint.write_usize(node.actions.len());
        for action in &node.actions {
            fingerprint.write_u8(accessibility_action_tag(*action));
        }
    }
    fingerprint.finish()
}

const fn accessibility_action_tag(action: AccessibilityAction) -> u8 {
    match action {
        AccessibilityAction::Click => 0,
        AccessibilityAction::Focus => 1,
        AccessibilityAction::ReplaceSelectedText => 2,
        AccessibilityAction::SetTextSelection => 3,
        AccessibilityAction::SetValue => 4,
        AccessibilityAction::ShowContextMenu => 5,
        AccessibilityAction::Increment => 6,
        AccessibilityAction::Decrement => 7,
    }
}

const fn accessibility_role_tag(role: AccessibilityRole) -> u8 {
    match role {
        AccessibilityRole::Window => 0,
        AccessibilityRole::Group => 1,
        AccessibilityRole::Label => 2,
        AccessibilityRole::Button => 3,
        AccessibilityRole::TextField => 4,
        AccessibilityRole::TextArea => 5,
        AccessibilityRole::List => 6,
        AccessibilityRole::ListItem => 7,
        AccessibilityRole::CheckBox => 8,
        AccessibilityRole::ProgressIndicator => 9,
        AccessibilityRole::MenuBar => 10,
        AccessibilityRole::Menu => 11,
        AccessibilityRole::MenuItem => 12,
        AccessibilityRole::TabList => 13,
        AccessibilityRole::Tab => 14,
        AccessibilityRole::Tree => 15,
        AccessibilityRole::TreeItem => 16,
        AccessibilityRole::Status => 17,
        AccessibilityRole::Dialog => 18,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StableFingerprint(u64);

impl StableFingerprint {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    const fn new() -> Self {
        Self(Self::OFFSET_BASIS)
    }

    const fn finish(self) -> u64 {
        self.0
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.write_bytes(&[value]);
    }

    fn write_u32(&mut self, value: u32) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn write_i32(&mut self, value: i32) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn write_usize(&mut self, value: usize) {
        self.write_u64(u64::try_from(value).unwrap_or(u64::MAX));
    }

    fn write_bool(&mut self, value: bool) {
        self.write_u8(u8::from(value));
    }

    fn write_string(&mut self, value: &str) {
        self.write_usize(value.len());
        self.write_bytes(value.as_bytes());
    }

    fn write_optional_string(&mut self, value: Option<&str>) {
        match value {
            Some(value) => {
                self.write_u8(1);
                self.write_string(value);
            }
            None => self.write_u8(0),
        }
    }

    fn write_node_id(&mut self, value: &NodeId) {
        self.write_string(value.as_str());
    }

    fn write_rect(&mut self, value: RectI) {
        self.write_i32(value.x);
        self.write_i32(value.y);
        self.write_u32(value.width);
        self.write_u32(value.height);
    }

    fn write_optional_text_range(&mut self, value: Option<AccessibilityTextRange>) {
        match value {
            Some(value) => {
                self.write_u8(1);
                self.write_usize(value.utf8_offset);
                self.write_usize(value.utf8_length);
            }
            None => self.write_u8(0),
        }
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

impl CodedError for AccessibilityTreeError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self {
            Self::DuplicateNode(_) => "accessibility.duplicate_node",
            Self::MissingRoot(_) => "accessibility.missing_root",
            Self::MissingChild { .. } => "accessibility.missing_child",
            Self::Cycle(_) => "accessibility.cycle",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{AccessibilityAction, AccessibilityNode, AccessibilityRole, AccessibilityTree};
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
    #[test]
    fn fingerprint_is_order_independent_and_semantic() -> Result<(), Box<dyn Error>> {
        let root = NodeId::new("root")?;
        let child = root.child("child")?;
        let root_node = AccessibilityNode::new(
            root.clone(),
            AccessibilityRole::Window,
            RectI::new(0, 0, 100, 80),
        )
        .with_children(vec![child.clone()]);
        let child_node = AccessibilityNode::new(
            child.clone(),
            AccessibilityRole::Button,
            RectI::new(10, 10, 30, 20),
        )
        .with_label("Run");
        let first = AccessibilityTree::new(root.clone(), [root_node.clone(), child_node.clone()])?;
        let second = AccessibilityTree::new(root.clone(), [child_node.clone(), root_node])?;
        let changed = AccessibilityTree::new(
            root,
            [
                AccessibilityNode::new(
                    NodeId::new("root")?,
                    AccessibilityRole::Window,
                    RectI::new(0, 0, 100, 80),
                )
                .with_children(vec![child.clone()]),
                child_node.with_label("Stop"),
            ],
        )?;

        assert_eq!(first.fingerprint(), second.fingerprint());
        assert_ne!(first.fingerprint(), changed.fingerprint());
        Ok(())
    }

    #[test]
    fn explicit_actions_participate_in_semantic_fingerprint() -> Result<(), Box<dyn Error>> {
        let root = NodeId::new("root")?;
        let without = AccessibilityTree::new(
            root.clone(),
            [AccessibilityNode::new(
                root.clone(),
                AccessibilityRole::TextArea,
                RectI::default(),
            )],
        )?;
        let with = AccessibilityTree::new(
            root.clone(),
            [
                AccessibilityNode::new(root, AccessibilityRole::TextArea, RectI::default())
                    .with_actions([
                        AccessibilityAction::Focus,
                        AccessibilityAction::ReplaceSelectedText,
                    ]),
            ],
        )?;
        assert_ne!(without.fingerprint(), with.fingerprint());
        Ok(())
    }
}
