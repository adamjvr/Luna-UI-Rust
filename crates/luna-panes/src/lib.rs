// SPDX-License-Identifier: MPL-2.0

//! Product-neutral recursive editor-pane trees.
//!
//! The model owns pane topology, pane-local tab membership, focus order, split ratios, and
//! deterministic geometry. It deliberately does not own document text, commands, rendering, or
//! platform input. Applications associate each [`DocumentViewId`] with independent caret,
//! selection, scroll, and presentation state while multiple views may share one document buffer.

use luna_core::{PointI, RectI};
use luna_documents::DocumentViewId;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// Stable application-local identity for one leaf or split node in a pane tree.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PaneId(u64);

impl PaneId {
    /// Returns the underlying monotonically assigned value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Returns a stable string suitable for widget and accessibility keys.
    #[must_use]
    pub fn stable_key(self) -> String {
        format!("pane-{}", self.0)
    }
}

impl Display for PaneId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "pane-{}", self.0)
    }
}

/// Direction in which a split divides its available rectangle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneAxis {
    /// Side-by-side panes separated by a vertical splitter.
    Horizontal,
    /// Stacked panes separated by a horizontal splitter.
    Vertical,
}

/// One leaf pane with pane-local tab ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneLeaf {
    id: PaneId,
    views: Vec<DocumentViewId>,
    active_view: DocumentViewId,
}

impl PaneLeaf {
    /// Returns the stable pane identity.
    #[must_use]
    pub const fn id(&self) -> PaneId {
        self.id
    }

    /// Returns pane-local views in tab order.
    #[must_use]
    pub fn views(&self) -> &[DocumentViewId] {
        &self.views
    }

    /// Returns the active view for this pane.
    #[must_use]
    pub const fn active_view(&self) -> DocumentViewId {
        self.active_view
    }
}

/// One recursive split node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneSplit {
    id: PaneId,
    axis: PaneAxis,
    ratio_milli: u16,
    first: Box<PaneNode>,
    second: Box<PaneNode>,
}

impl PaneSplit {
    /// Returns the stable split identity.
    #[must_use]
    pub const fn id(&self) -> PaneId {
        self.id
    }

    /// Returns the split direction.
    #[must_use]
    pub const fn axis(&self) -> PaneAxis {
        self.axis
    }

    /// Returns the first-child share in thousandths.
    #[must_use]
    pub const fn ratio_milli(&self) -> u16 {
        self.ratio_milli
    }

    /// Returns the first child.
    #[must_use]
    pub fn first(&self) -> &PaneNode {
        &self.first
    }

    /// Returns the second child.
    #[must_use]
    pub fn second(&self) -> &PaneNode {
        &self.second
    }
}

/// One node in a recursive pane tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaneNode {
    /// A terminal editor pane.
    Leaf(PaneLeaf),
    /// A recursive split containing two child nodes.
    Split(PaneSplit),
}

impl PaneNode {
    /// Returns the stable node identity.
    #[must_use]
    pub const fn id(&self) -> PaneId {
        match self {
            Self::Leaf(leaf) => leaf.id,
            Self::Split(split) => split.id,
        }
    }
}

/// Geometry constants used when projecting a pane tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaneLayoutMetrics {
    /// Height reserved for each pane-local tab strip.
    pub tab_strip_height: u32,
    /// Thickness of draggable splitters.
    pub splitter_thickness: u32,
    /// Minimum complete width or height of a leaf pane.
    pub minimum_pane_extent: u32,
}

impl Default for PaneLayoutMetrics {
    fn default() -> Self {
        Self {
            tab_strip_height: 30,
            splitter_thickness: 6,
            minimum_pane_extent: 140,
        }
    }
}

/// Shared geometry for one visible leaf pane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaneLeafFrame {
    /// Stable leaf identity.
    pub pane_id: PaneId,
    /// Complete pane bounds.
    pub bounds: RectI,
    /// Pane-local tab strip.
    pub tab_strip: RectI,
    /// Editor viewport below the tab strip.
    pub editor: RectI,
    /// Whether this pane owns keyboard focus.
    pub is_focused: bool,
}

/// Shared geometry for one draggable splitter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaneSplitterFrame {
    /// Stable split-node identity.
    pub split_id: PaneId,
    /// Split direction.
    pub axis: PaneAxis,
    /// Draggable splitter rectangle.
    pub bounds: RectI,
    /// Complete bounds allocated to this split.
    pub container: RectI,
}

/// Complete immutable pane geometry snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneLayoutSnapshot {
    /// Complete bounds supplied by the application.
    pub bounds: RectI,
    /// Leaf panes in deterministic depth-first order.
    pub leaves: Vec<PaneLeafFrame>,
    /// Splitters in deterministic depth-first order.
    pub splitters: Vec<PaneSplitterFrame>,
}

impl PaneLayoutSnapshot {
    /// Returns the leaf containing `point`.
    #[must_use]
    pub fn leaf_at(&self, point: PointI) -> Option<PaneLeafFrame> {
        self.leaves
            .iter()
            .copied()
            .find(|leaf| leaf.bounds.contains(point))
    }

    /// Returns the editor viewport containing `point`.
    #[must_use]
    pub fn editor_at(&self, point: PointI) -> Option<PaneLeafFrame> {
        self.leaves
            .iter()
            .copied()
            .find(|leaf| leaf.editor.contains(point))
    }

    /// Returns the splitter containing `point`.
    #[must_use]
    pub fn splitter_at(&self, point: PointI) -> Option<PaneSplitterFrame> {
        self.splitters
            .iter()
            .rev()
            .copied()
            .find(|splitter| splitter.bounds.contains(point))
    }
}

/// Result of closing one pane or pane-local view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneCloseResult {
    /// Views removed from the tree.
    pub removed_views: Vec<DocumentViewId>,
    /// Leaf that receives focus after reconciliation.
    pub focused_pane: PaneId,
}

/// Errors produced by pane-tree operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaneError {
    /// The requested pane identity is not present.
    UnknownPane(PaneId),
    /// The requested view identity is not owned by the requested pane.
    UnknownView(DocumentViewId),
    /// The final remaining pane cannot be closed.
    CannotCloseLastPane,
    /// A tab operation would leave a leaf with no active view before reconciliation.
    EmptyPane,
}

impl Display for PaneError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPane(id) => write!(formatter, "unknown pane: {id}"),
            Self::UnknownView(_) => write!(formatter, "unknown document view"),
            Self::CannotCloseLastPane => write!(formatter, "cannot close the final editor pane"),
            Self::EmptyPane => write!(formatter, "pane must own at least one document view"),
        }
    }
}

impl Error for PaneError {}

/// Recursive pane topology and pane-local tab state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneTree {
    next_id: u64,
    root: PaneNode,
    focused_pane: PaneId,
}

impl PaneTree {
    /// Creates a one-pane tree containing `initial_view`.
    #[must_use]
    pub fn new(initial_view: DocumentViewId) -> Self {
        let root_id = PaneId(1);
        Self {
            next_id: 2,
            root: PaneNode::Leaf(PaneLeaf {
                id: root_id,
                views: vec![initial_view],
                active_view: initial_view,
            }),
            focused_pane: root_id,
        }
    }

    /// Returns the recursive root node.
    #[must_use]
    pub const fn root(&self) -> &PaneNode {
        &self.root
    }

    /// Returns the currently focused leaf pane.
    #[must_use]
    pub const fn focused_pane(&self) -> PaneId {
        self.focused_pane
    }

    /// Returns the focused pane's active view.
    #[must_use]
    pub fn focused_view(&self) -> DocumentViewId {
        self.leaf(self.focused_pane)
            .map_or_else(|| first_leaf(&self.root).active_view, PaneLeaf::active_view)
    }

    /// Returns a leaf by identity.
    #[must_use]
    pub fn leaf(&self, pane_id: PaneId) -> Option<&PaneLeaf> {
        find_leaf(&self.root, pane_id)
    }

    /// Returns all leaves in deterministic depth-first order.
    #[must_use]
    pub fn leaves(&self) -> Vec<&PaneLeaf> {
        let mut leaves = Vec::new();
        collect_leaves(&self.root, &mut leaves);
        leaves
    }

    /// Returns the leaf pane that owns `view_id`.
    #[must_use]
    pub fn pane_for_view(&self, view_id: DocumentViewId) -> Option<PaneId> {
        self.leaves()
            .into_iter()
            .find(|leaf| leaf.views.contains(&view_id))
            .map(|leaf| leaf.id)
    }

    /// Returns all split nodes in deterministic depth-first order.
    #[must_use]
    pub fn splits(&self) -> Vec<&PaneSplit> {
        let mut splits = Vec::new();
        collect_splits(&self.root, &mut splits);
        splits
    }

    /// Sets keyboard focus to an existing leaf pane.
    pub fn focus(&mut self, pane_id: PaneId) -> Result<(), PaneError> {
        if self.leaf(pane_id).is_none() {
            return Err(PaneError::UnknownPane(pane_id));
        }
        self.focused_pane = pane_id;
        Ok(())
    }

    /// Moves focus to the next leaf in depth-first order, wrapping at the end.
    #[must_use]
    pub fn focus_next(&mut self) -> PaneId {
        self.move_focus(1)
    }

    /// Moves focus to the previous leaf in depth-first order, wrapping at the beginning.
    #[must_use]
    pub fn focus_previous(&mut self) -> PaneId {
        self.move_focus(-1)
    }

    /// Adds `view_id` to a pane and makes it active there.
    pub fn add_view(&mut self, pane_id: PaneId, view_id: DocumentViewId) -> Result<(), PaneError> {
        let leaf = find_leaf_mut(&mut self.root, pane_id).ok_or(PaneError::UnknownPane(pane_id))?;
        if !leaf.views.contains(&view_id) {
            leaf.views.push(view_id);
        }
        leaf.active_view = view_id;
        self.focused_pane = pane_id;
        Ok(())
    }

    /// Makes one pane-local view active.
    pub fn activate_view(
        &mut self,
        pane_id: PaneId,
        view_id: DocumentViewId,
    ) -> Result<(), PaneError> {
        let leaf = find_leaf_mut(&mut self.root, pane_id).ok_or(PaneError::UnknownPane(pane_id))?;
        if !leaf.views.contains(&view_id) {
            return Err(PaneError::UnknownView(view_id));
        }
        leaf.active_view = view_id;
        self.focused_pane = pane_id;
        Ok(())
    }

    /// Splits the focused pane and places `new_view` in the newly created second pane.
    pub fn split_focused(&mut self, axis: PaneAxis, new_view: DocumentViewId) -> PaneId {
        let source = self.focused_pane;
        self.split(source, axis, new_view)
            .unwrap_or(self.focused_pane)
    }

    /// Splits one leaf and places `new_view` in the newly created second pane.
    pub fn split(
        &mut self,
        pane_id: PaneId,
        axis: PaneAxis,
        new_view: DocumentViewId,
    ) -> Result<PaneId, PaneError> {
        if self.leaf(pane_id).is_none() {
            return Err(PaneError::UnknownPane(pane_id));
        }
        let split_id = self.allocate_id();
        let new_leaf_id = self.allocate_id();
        let new_leaf = PaneNode::Leaf(PaneLeaf {
            id: new_leaf_id,
            views: vec![new_view],
            active_view: new_view,
        });
        let replaced = replace_leaf_with_split(&mut self.root, pane_id, split_id, axis, new_leaf);
        if !replaced {
            return Err(PaneError::UnknownPane(pane_id));
        }
        self.focused_pane = new_leaf_id;
        Ok(new_leaf_id)
    }

    /// Removes one pane-local view, collapsing an empty pane when another pane exists.
    pub fn close_view(
        &mut self,
        pane_id: PaneId,
        view_id: DocumentViewId,
    ) -> Result<PaneCloseResult, PaneError> {
        let (view_index, view_count) = self
            .leaf(pane_id)
            .ok_or(PaneError::UnknownPane(pane_id))
            .and_then(|leaf| {
                leaf.views
                    .iter()
                    .position(|candidate| *candidate == view_id)
                    .map(|index| (index, leaf.views.len()))
                    .ok_or(PaneError::UnknownView(view_id))
            })?;
        if view_count == 1 && self.leaves().len() == 1 {
            return Err(PaneError::CannotCloseLastPane);
        }
        if view_count > 1 {
            let leaf =
                find_leaf_mut(&mut self.root, pane_id).ok_or(PaneError::UnknownPane(pane_id))?;
            leaf.views.remove(view_index);
            let next_index = view_index.min(leaf.views.len().saturating_sub(1));
            if leaf.active_view == view_id {
                leaf.active_view = leaf.views[next_index];
            }
            self.focused_pane = pane_id;
            return Ok(PaneCloseResult {
                removed_views: vec![view_id],
                focused_pane: pane_id,
            });
        }
        let focused =
            collapse_leaf(&mut self.root, pane_id).ok_or(PaneError::UnknownPane(pane_id))?;
        self.focused_pane = focused;
        Ok(PaneCloseResult {
            removed_views: vec![view_id],
            focused_pane: focused,
        })
    }

    /// Closes the focused leaf and removes all of its pane-local views.
    pub fn close_focused_pane(&mut self) -> Result<PaneCloseResult, PaneError> {
        if self.leaves().len() == 1 {
            return Err(PaneError::CannotCloseLastPane);
        }
        let pane_id = self.focused_pane;
        let removed_views = self
            .leaf(pane_id)
            .ok_or(PaneError::UnknownPane(pane_id))?
            .views
            .clone();
        let focused =
            collapse_leaf(&mut self.root, pane_id).ok_or(PaneError::UnknownPane(pane_id))?;
        self.focused_pane = focused;
        Ok(PaneCloseResult {
            removed_views,
            focused_pane: focused,
        })
    }

    /// Updates one split ratio, clamped to ten through ninety percent.
    pub fn set_split_ratio(&mut self, split_id: PaneId, ratio_milli: u16) -> Result<(), PaneError> {
        let split =
            find_split_mut(&mut self.root, split_id).ok_or(PaneError::UnknownPane(split_id))?;
        split.ratio_milli = ratio_milli.clamp(100, 900);
        Ok(())
    }

    /// Updates one split ratio from a pointer position in the supplied container.
    pub fn set_split_ratio_from_point(
        &mut self,
        split_id: PaneId,
        container: RectI,
        point: PointI,
    ) -> Result<(), PaneError> {
        let split =
            find_split_mut(&mut self.root, split_id).ok_or(PaneError::UnknownPane(split_id))?;
        let ratio = match split.axis {
            PaneAxis::Horizontal => ratio_from_coordinate(point.x, container.x, container.width),
            PaneAxis::Vertical => ratio_from_coordinate(point.y, container.y, container.height),
        };
        split.ratio_milli = ratio.clamp(100, 900);
        Ok(())
    }

    /// Builds deterministic leaf and splitter geometry.
    #[must_use]
    pub fn layout(&self, bounds: RectI, metrics: PaneLayoutMetrics) -> PaneLayoutSnapshot {
        let mut snapshot = PaneLayoutSnapshot {
            bounds,
            leaves: Vec::new(),
            splitters: Vec::new(),
        };
        layout_node(
            &self.root,
            bounds,
            self.focused_pane,
            metrics,
            &mut snapshot,
        );
        snapshot
    }

    fn allocate_id(&mut self) -> PaneId {
        let id = PaneId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    fn move_focus(&mut self, delta: i32) -> PaneId {
        let leaf_ids = self
            .leaves()
            .into_iter()
            .map(PaneLeaf::id)
            .collect::<Vec<_>>();
        let current = leaf_ids
            .iter()
            .position(|pane_id| *pane_id == self.focused_pane)
            .unwrap_or(0);
        let count = i32::try_from(leaf_ids.len()).unwrap_or(i32::MAX).max(1);
        let next = (i32::try_from(current).unwrap_or(0) + delta).rem_euclid(count);
        self.focused_pane = leaf_ids[usize::try_from(next).unwrap_or(0)];
        self.focused_pane
    }
}

fn first_leaf(node: &PaneNode) -> &PaneLeaf {
    match node {
        PaneNode::Leaf(leaf) => leaf,
        PaneNode::Split(split) => first_leaf(&split.first),
    }
}

fn collect_leaves<'a>(node: &'a PaneNode, leaves: &mut Vec<&'a PaneLeaf>) {
    match node {
        PaneNode::Leaf(leaf) => leaves.push(leaf),
        PaneNode::Split(split) => {
            collect_leaves(&split.first, leaves);
            collect_leaves(&split.second, leaves);
        }
    }
}

fn collect_splits<'a>(node: &'a PaneNode, splits: &mut Vec<&'a PaneSplit>) {
    if let PaneNode::Split(split) = node {
        splits.push(split);
        collect_splits(&split.first, splits);
        collect_splits(&split.second, splits);
    }
}

fn find_leaf(node: &PaneNode, pane_id: PaneId) -> Option<&PaneLeaf> {
    match node {
        PaneNode::Leaf(leaf) => (leaf.id == pane_id).then_some(leaf),
        PaneNode::Split(split) => {
            find_leaf(&split.first, pane_id).or_else(|| find_leaf(&split.second, pane_id))
        }
    }
}

fn find_leaf_mut(node: &mut PaneNode, pane_id: PaneId) -> Option<&mut PaneLeaf> {
    match node {
        PaneNode::Leaf(leaf) => (leaf.id == pane_id).then_some(leaf),
        PaneNode::Split(split) => find_leaf_mut(&mut split.first, pane_id)
            .or_else(|| find_leaf_mut(&mut split.second, pane_id)),
    }
}

fn find_split_mut(node: &mut PaneNode, split_id: PaneId) -> Option<&mut PaneSplit> {
    match node {
        PaneNode::Leaf(_) => None,
        PaneNode::Split(split) => {
            if split.id == split_id {
                return Some(split);
            }

            if let Some(found) = find_split_mut(&mut split.first, split_id) {
                return Some(found);
            }

            find_split_mut(&mut split.second, split_id)
        }
    }
}

fn replace_leaf_with_split(
    node: &mut PaneNode,
    pane_id: PaneId,
    split_id: PaneId,
    axis: PaneAxis,
    new_leaf: PaneNode,
) -> bool {
    match node {
        PaneNode::Leaf(leaf) if leaf.id == pane_id => {
            let original = node.clone();
            *node = PaneNode::Split(PaneSplit {
                id: split_id,
                axis,
                ratio_milli: 500,
                first: Box::new(original),
                second: Box::new(new_leaf),
            });
            true
        }
        PaneNode::Leaf(_) => false,
        PaneNode::Split(split) => {
            replace_leaf_with_split(&mut split.first, pane_id, split_id, axis, new_leaf.clone())
                || replace_leaf_with_split(&mut split.second, pane_id, split_id, axis, new_leaf)
        }
    }
}

fn collapse_leaf(node: &mut PaneNode, pane_id: PaneId) -> Option<PaneId> {
    let replacement = match node {
        PaneNode::Split(split) if matches!(split.first.as_ref(), PaneNode::Leaf(leaf) if leaf.id == pane_id) => {
            Some((*split.second).clone())
        }
        PaneNode::Split(split) if matches!(split.second.as_ref(), PaneNode::Leaf(leaf) if leaf.id == pane_id) => {
            Some((*split.first).clone())
        }
        PaneNode::Leaf(_) | PaneNode::Split(_) => None,
    };
    if let Some(replacement) = replacement {
        let focused = first_leaf(&replacement).id;
        *node = replacement;
        return Some(focused);
    }
    match node {
        PaneNode::Leaf(_) => None,
        PaneNode::Split(split) => collapse_leaf(&mut split.first, pane_id)
            .or_else(|| collapse_leaf(&mut split.second, pane_id)),
    }
}

fn ratio_from_coordinate(coordinate: i32, origin: i32, extent: u32) -> u16 {
    if extent == 0 {
        return 500;
    }
    let relative = i64::from(coordinate).saturating_sub(i64::from(origin));
    let numerator = relative.saturating_mul(1_000);
    let ratio = numerator / i64::from(extent);
    u16::try_from(ratio.clamp(0, 1_000)).unwrap_or(500)
}

fn layout_node(
    node: &PaneNode,
    bounds: RectI,
    focused_pane: PaneId,
    metrics: PaneLayoutMetrics,
    snapshot: &mut PaneLayoutSnapshot,
) {
    match node {
        PaneNode::Leaf(leaf) => {
            let tab_height = metrics.tab_strip_height.min(bounds.height);
            let editor_y = bounds
                .y
                .saturating_add(i32::try_from(tab_height).unwrap_or(i32::MAX));
            snapshot.leaves.push(PaneLeafFrame {
                pane_id: leaf.id,
                bounds,
                tab_strip: RectI::new(bounds.x, bounds.y, bounds.width, tab_height),
                editor: RectI::new(
                    bounds.x,
                    editor_y,
                    bounds.width,
                    bounds.height.saturating_sub(tab_height),
                ),
                is_focused: leaf.id == focused_pane,
            });
        }
        PaneNode::Split(split) => {
            let (first_bounds, splitter, second_bounds) = split_bounds(bounds, split, metrics);
            snapshot.splitters.push(PaneSplitterFrame {
                split_id: split.id,
                axis: split.axis,
                bounds: splitter,
                container: bounds,
            });
            layout_node(&split.first, first_bounds, focused_pane, metrics, snapshot);
            layout_node(
                &split.second,
                second_bounds,
                focused_pane,
                metrics,
                snapshot,
            );
        }
    }
}

fn split_bounds(
    bounds: RectI,
    split: &PaneSplit,
    metrics: PaneLayoutMetrics,
) -> (RectI, RectI, RectI) {
    let horizontal = split.axis == PaneAxis::Horizontal;
    let extent = if horizontal {
        bounds.width
    } else {
        bounds.height
    };
    let splitter_extent = metrics.splitter_thickness.min(extent);
    let available = extent.saturating_sub(splitter_extent);
    let requested = available.saturating_mul(u32::from(split.ratio_milli)) / 1_000;
    let minimum = metrics.minimum_pane_extent.min(available / 2);
    let first_extent = requested.clamp(minimum, available.saturating_sub(minimum));
    let second_extent = available.saturating_sub(first_extent);
    if horizontal {
        let splitter_x = bounds
            .x
            .saturating_add(i32::try_from(first_extent).unwrap_or(i32::MAX));
        let second_x =
            splitter_x.saturating_add(i32::try_from(splitter_extent).unwrap_or(i32::MAX));
        (
            RectI::new(bounds.x, bounds.y, first_extent, bounds.height),
            RectI::new(splitter_x, bounds.y, splitter_extent, bounds.height),
            RectI::new(second_x, bounds.y, second_extent, bounds.height),
        )
    } else {
        let splitter_y = bounds
            .y
            .saturating_add(i32::try_from(first_extent).unwrap_or(i32::MAX));
        let second_y =
            splitter_y.saturating_add(i32::try_from(splitter_extent).unwrap_or(i32::MAX));
        (
            RectI::new(bounds.x, bounds.y, bounds.width, first_extent),
            RectI::new(bounds.x, splitter_y, bounds.width, splitter_extent),
            RectI::new(bounds.x, second_y, bounds.width, second_extent),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{PaneAxis, PaneError, PaneLayoutMetrics, PaneTree};
    use luna_core::{PointI, RectI};
    use luna_documents::{DocumentRegistry, DocumentViewRegistry};
    use std::error::Error;

    type TestResult = Result<(), Box<dyn Error>>;

    fn document(
        registry: &mut DocumentRegistry,
        key: &str,
    ) -> Result<luna_documents::DocumentId, Box<dyn Error>> {
        Ok(registry.register_virtual(key, key, 0)?)
    }

    fn views() -> Result<
        (
            DocumentRegistry,
            DocumentViewRegistry,
            luna_documents::DocumentViewId,
        ),
        Box<dyn Error>,
    > {
        let mut documents = DocumentRegistry::new();
        let document_id = document(&mut documents, "one")?;
        let mut registry = DocumentViewRegistry::new();
        let view = registry.create_view(document_id);
        Ok((documents, registry, view))
    }

    #[test]
    fn split_preserves_original_and_focuses_new_leaf() -> TestResult {
        let (mut documents, mut registry, first_view) = views()?;
        let second_view = registry.create_view(document(&mut documents, "two")?);
        let mut panes = PaneTree::new(first_view);
        let original = panes.focused_pane();
        let second = panes.split_focused(PaneAxis::Horizontal, second_view);

        assert_eq!(panes.leaves().len(), 2);
        assert_eq!(
            panes.leaf(original).map(|leaf| leaf.active_view()),
            Some(first_view)
        );
        assert_eq!(panes.focused_pane(), second);
        assert_eq!(panes.focused_view(), second_view);
        Ok(())
    }

    #[test]
    fn focus_traversal_wraps_depth_first() -> TestResult {
        let (mut documents, mut registry, first_view) = views()?;
        let second_view = registry.create_view(document(&mut documents, "two")?);
        let third_view = registry.create_view(document(&mut documents, "three")?);
        let mut panes = PaneTree::new(first_view);
        let second = panes.split_focused(PaneAxis::Horizontal, second_view);
        let third = panes
            .split(second, PaneAxis::Vertical, third_view)
            .unwrap_or(second);

        assert_eq!(panes.focused_pane(), third);
        assert_eq!(panes.focus_next(), panes.leaves()[0].id());
        assert_eq!(panes.focus_previous(), third);
        Ok(())
    }

    #[test]
    fn closing_leaf_collapses_parent_and_keeps_sibling() -> TestResult {
        let (mut documents, mut registry, first_view) = views()?;
        let second_view = registry.create_view(document(&mut documents, "two")?);
        let mut panes = PaneTree::new(first_view);
        let second = panes.split_focused(PaneAxis::Horizontal, second_view);
        let result = panes.close_focused_pane();

        assert!(result.is_ok());
        assert_eq!(panes.leaves().len(), 1);
        assert_eq!(panes.focused_view(), first_view);
        assert!(panes.leaf(second).is_none());
        assert_eq!(
            panes.close_focused_pane(),
            Err(PaneError::CannotCloseLastPane)
        );
        Ok(())
    }

    #[test]
    fn close_view_uses_neighboring_tab_before_collapsing() -> TestResult {
        let (mut documents, mut registry, first_view) = views()?;
        let second_view = registry.create_view(document(&mut documents, "two")?);
        let mut panes = PaneTree::new(first_view);
        let pane = panes.focused_pane();
        assert!(panes.add_view(pane, second_view).is_ok());
        assert!(panes.close_view(pane, second_view).is_ok());
        assert_eq!(panes.focused_view(), first_view);
        assert_eq!(panes.leaf(pane).map(|leaf| leaf.views().len()), Some(1));
        Ok(())
    }

    #[test]
    fn layout_respects_minimum_extent_and_tab_strips() -> TestResult {
        let (mut documents, mut registry, first_view) = views()?;
        let second_view = registry.create_view(document(&mut documents, "two")?);
        let mut panes = PaneTree::new(first_view);
        let _ = panes.split_focused(PaneAxis::Horizontal, second_view);
        let layout = panes.layout(RectI::new(0, 0, 500, 300), PaneLayoutMetrics::default());

        assert_eq!(layout.leaves.len(), 2);
        assert_eq!(layout.splitters.len(), 1);
        assert!(layout.leaves.iter().all(|leaf| leaf.bounds.width >= 140));
        assert!(layout.leaves.iter().all(|leaf| leaf.tab_strip.height == 30));
        assert!(layout.editor_at(PointI::new(20, 80)).is_some());
        Ok(())
    }

    #[test]
    fn splitter_ratio_tracks_pointer_and_clamps() -> TestResult {
        let (mut documents, mut registry, first_view) = views()?;
        let second_view = registry.create_view(document(&mut documents, "two")?);
        let mut panes = PaneTree::new(first_view);
        let _ = panes.split_focused(PaneAxis::Horizontal, second_view);
        let split_id = panes.splits()[0].id();
        assert!(
            panes
                .set_split_ratio_from_point(
                    split_id,
                    RectI::new(100, 0, 1_000, 400),
                    PointI::new(1_050, 40),
                )
                .is_ok()
        );
        assert_eq!(panes.splits()[0].ratio_milli(), 900);
        Ok(())
    }
}
