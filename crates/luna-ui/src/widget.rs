// SPDX-License-Identifier: MPL-2.0

use luna_accessibility::AccessibilityNode;
use luna_core::{NodeId, PointI, RectI};
use luna_render::DisplayList;

/// Deterministic contract implemented by every Luna widget.
///
/// A widget is not a retained native control and does not draw directly through a graphics API.
/// It describes an immutable paint snapshot, semantic nodes, and hit-testing behavior from the
/// same state and bounds. The host can then render and expose accessibility independently.
pub trait Widget {
    /// Returns the stable root ID for this widget.
    fn id(&self) -> &NodeId;

    /// Returns the widget's complete logical bounds.
    fn bounds(&self) -> RectI;

    /// Appends backend-neutral paint commands.
    fn build_display_list(&self, display_list: &mut DisplayList);

    /// Returns the widget's semantic nodes, beginning with its root node.
    fn accessibility_nodes(&self) -> Vec<AccessibilityNode>;

    /// Returns the deepest interactive node at `point`.
    ///
    /// M0 provides a conservative default that returns the widget root. Composite widgets will
    /// override this and traverse child geometry from front to back.
    fn hit_test(&self, point: PointI) -> Option<NodeId> {
        self.bounds().contains(point).then(|| self.id().clone())
    }
}
