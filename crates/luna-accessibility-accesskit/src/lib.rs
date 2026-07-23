// SPDX-License-Identifier: MPL-2.0

//! Translation from Luna's platform-neutral semantic tree to AccessKit.
//!
//! This crate is intentionally a leaf adapter. Luna widgets know only about
//! `luna-accessibility`; native hosts opt into AccessKit here. The bridge retains a stable mapping
//! from readable Luna node IDs to AccessKit's numeric IDs so repeated full snapshots preserve
//! native accessibility identity across frames.

use accesskit::{
    Action, Node as AccessNode, NodeId as AccessNodeId, Rect as AccessRect, Role as AccessRole,
    Tree, TreeId, TreeUpdate,
};
use luna_accessibility::{AccessibilityNode, AccessibilityRole, AccessibilityTree};
use luna_core::NodeId;
use std::collections::BTreeMap;

/// Stateful stable-ID translator and full-snapshot builder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessKitBridge {
    ids: BTreeMap<NodeId, AccessNodeId>,
    luna_ids: BTreeMap<AccessNodeId, NodeId>,
    next_id: u64,
}

impl Default for AccessKitBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessKitBridge {
    /// Creates an empty bridge.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ids: BTreeMap::new(),
            luna_ids: BTreeMap::new(),
            // Zero is valid in AccessKit examples, but starting at one makes accidental default
            // values visually distinct in debugger output and leaves zero available for tooling.
            next_id: 1,
        }
    }

    /// Returns the stable AccessKit ID assigned to a Luna node, allocating it when first seen.
    pub fn id_for(&mut self, id: &NodeId) -> AccessNodeId {
        if let Some(existing) = self.ids.get(id) {
            return *existing;
        }

        let assigned = AccessNodeId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.ids.insert(id.clone(), assigned);
        self.luna_ids.insert(assigned, id.clone());
        assigned
    }

    /// Resolves a previously assigned AccessKit ID back to Luna's readable node ID.
    #[must_use]
    pub fn luna_id_for(&self, id: AccessNodeId) -> Option<&NodeId> {
        self.luna_ids.get(&id)
    }

    /// Builds a complete AccessKit tree update from one validated Luna snapshot.
    ///
    /// AccessKit expects physical-pixel coordinates relative to the window. Luna stores logical
    /// integer coordinates, so this boundary performs the DPI conversion exactly once. The host
    /// may send this full update for initial activation and ordinary M1 frames; a later optimization
    /// can diff immutable snapshots without changing widget APIs.
    pub fn full_update(&mut self, tree: &AccessibilityTree, scale_factor: f64) -> TreeUpdate {
        // Allocate every ID before translating children. This guarantees that child lookup never
        // depends on iteration order and keeps the conversion deterministic.
        for node in tree.nodes() {
            let _ = self.id_for(&node.id);
        }

        let root = self.id_for(tree.root());
        let focus = tree
            .nodes()
            .find(|node| node.is_focused)
            .map(|node| self.id_for(&node.id))
            .unwrap_or(root);
        let nodes = tree
            .nodes()
            .map(|node| {
                let id = self.id_for(&node.id);
                (id, self.translate_node(node, scale_factor))
            })
            .collect();

        let mut metadata = Tree::new(root);
        metadata.toolkit_name = Some("Luna-UI-Rust".to_owned());
        metadata.toolkit_version = Some(env!("CARGO_PKG_VERSION").to_owned());

        TreeUpdate {
            nodes,
            tree: Some(metadata),
            tree_id: TreeId::ROOT,
            focus,
        }
    }

    fn translate_node(&mut self, source: &AccessibilityNode, scale_factor: f64) -> AccessNode {
        let mut node = AccessNode::new(map_role(source.role));
        if let Some(label) = &source.label {
            node.set_label(label.clone());
        }
        if let Some(value) = &source.value {
            node.set_value(value.clone());
        }
        node.set_bounds(scale_bounds(source.bounds, scale_factor));
        node.set_children(
            source
                .children
                .iter()
                .map(|child| self.id_for(child))
                .collect::<Vec<_>>(),
        );

        if source.is_disabled {
            node.set_disabled();
        }

        // Actions communicate what native assistive technology may request. Application behavior
        // still routes through the host and Luna command system; this adapter never executes it.
        match source.role {
            AccessibilityRole::Button
            | AccessibilityRole::CheckBox
            | AccessibilityRole::MenuItem
            | AccessibilityRole::Tab
            | AccessibilityRole::TreeItem => {
                node.add_action(Action::Click);
                node.add_action(Action::Focus);
            }
            AccessibilityRole::TextField | AccessibilityRole::TextArea => {
                node.add_action(Action::Focus);
            }
            AccessibilityRole::Window
            | AccessibilityRole::Group
            | AccessibilityRole::Label
            | AccessibilityRole::List
            | AccessibilityRole::ListItem
            | AccessibilityRole::ProgressIndicator
            | AccessibilityRole::MenuBar
            | AccessibilityRole::Menu
            | AccessibilityRole::TabList
            | AccessibilityRole::Tree
            | AccessibilityRole::Status
            | AccessibilityRole::Dialog => {}
        }

        node
    }
}

fn map_role(role: AccessibilityRole) -> AccessRole {
    match role {
        AccessibilityRole::Window => AccessRole::Window,
        AccessibilityRole::Group => AccessRole::Pane,
        AccessibilityRole::Label => AccessRole::Label,
        AccessibilityRole::Button => AccessRole::Button,
        AccessibilityRole::TextField => AccessRole::TextInput,
        AccessibilityRole::TextArea => AccessRole::MultilineTextInput,
        AccessibilityRole::List => AccessRole::List,
        AccessibilityRole::ListItem => AccessRole::ListItem,
        AccessibilityRole::CheckBox => AccessRole::CheckBox,
        AccessibilityRole::ProgressIndicator => AccessRole::ProgressIndicator,
        AccessibilityRole::MenuBar => AccessRole::MenuBar,
        AccessibilityRole::Menu => AccessRole::Menu,
        AccessibilityRole::MenuItem => AccessRole::MenuItem,
        AccessibilityRole::TabList => AccessRole::TabList,
        AccessibilityRole::Tab => AccessRole::Tab,
        AccessibilityRole::Tree => AccessRole::Tree,
        AccessibilityRole::TreeItem => AccessRole::TreeItem,
        AccessibilityRole::Status => AccessRole::Status,
        AccessibilityRole::Dialog => AccessRole::Dialog,
    }
}

fn scale_bounds(bounds: luna_core::RectI, scale_factor: f64) -> AccessRect {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    let x0 = f64::from(bounds.x) * scale;
    let y0 = f64::from(bounds.y) * scale;
    AccessRect {
        x0,
        y0,
        x1: x0 + f64::from(bounds.width) * scale,
        y1: y0 + f64::from(bounds.height) * scale,
    }
}

#[cfg(test)]
mod tests {
    use super::AccessKitBridge;
    use accesskit::{Role, TreeId};
    use luna_accessibility::{AccessibilityNode, AccessibilityRole, AccessibilityTree};
    use luna_core::{NodeId, RectI};
    use std::error::Error;

    #[test]
    fn full_update_preserves_ids_and_scales_bounds() -> Result<(), Box<dyn Error>> {
        let root = NodeId::new("root")?;
        let child = root.child("button")?;
        let tree = AccessibilityTree::new(
            root.clone(),
            [
                AccessibilityNode::new(
                    root.clone(),
                    AccessibilityRole::Window,
                    RectI::new(0, 0, 100, 80),
                )
                .with_children(vec![child.clone()]),
                AccessibilityNode::new(
                    child.clone(),
                    AccessibilityRole::Button,
                    RectI::new(10, 20, 30, 40),
                )
                .with_label("Run"),
            ],
        )?;
        let mut bridge = AccessKitBridge::new();
        let first = bridge.full_update(&tree, 2.0);
        let second = bridge.full_update(&tree, 2.0);

        assert_eq!(first.tree_id, TreeId::ROOT);
        assert_eq!(
            first.focus,
            first.tree.as_ref().map_or(first.focus, |value| value.root)
        );
        assert_eq!(first.nodes[0].0, second.nodes[0].0);
        let button = first
            .nodes
            .iter()
            .find(|(_, node)| node.role() == Role::Button)
            .map(|(_, node)| node)
            .ok_or_else(|| std::io::Error::other("translated button missing"))?;
        assert_eq!(button.bounds().map(|bounds| bounds.x0), Some(20.0));
        assert_eq!(button.bounds().map(|bounds| bounds.y1), Some(120.0));
        Ok(())
    }
}
