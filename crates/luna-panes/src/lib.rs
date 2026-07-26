// SPDX-License-Identifier: MPL-2.0

//! Product-neutral recursive editor-pane trees.
//!
//! The model owns pane topology, pane-local tab membership, focus order, split ratios, and
//! deterministic geometry. It deliberately does not own document text, commands, rendering, or
//! platform input. Applications associate each [`DocumentViewId`] with independent caret,
//! selection, scroll, and presentation state while multiple views may share one document buffer.

use luna_core::{PointI, RectI};
use luna_documents::DocumentViewId;
use std::collections::BTreeSet;
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
    pinned_views: Vec<DocumentViewId>,
    preview_view: Option<DocumentViewId>,
    tab_scroll_offset: usize,
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

    /// Returns pinned views in their display order.
    #[must_use]
    pub fn pinned_views(&self) -> &[DocumentViewId] {
        &self.pinned_views
    }

    /// Returns the pane-local preview view, when one exists.
    #[must_use]
    pub const fn preview_view(&self) -> Option<DocumentViewId> {
        self.preview_view
    }

    /// Returns whether `view_id` is pinned in this pane.
    #[must_use]
    pub fn is_pinned(&self, view_id: DocumentViewId) -> bool {
        self.pinned_views.contains(&view_id)
    }

    /// Returns whether `view_id` is the transient preview tab.
    #[must_use]
    pub fn is_preview(&self, view_id: DocumentViewId) -> bool {
        self.preview_view == Some(view_id)
    }

    /// Returns the first regular tab index projected into the overflow viewport.
    #[must_use]
    pub const fn tab_scroll_offset(&self) -> usize {
        self.tab_scroll_offset
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

/// Persistable product-neutral snapshot of one pane tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneTreeSnapshot {
    /// Focused leaf-pane numeric identity.
    pub focused_pane_key: u64,
    /// Recursive root snapshot.
    pub root: PaneNodeSnapshot,
}

/// Persistable recursive pane node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaneNodeSnapshot {
    /// Terminal pane with ordered application-defined view keys.
    Leaf(PaneLeafSnapshot),
    /// Recursive split.
    Split(PaneSplitSnapshot),
}

/// Persistable leaf-pane state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneLeafSnapshot {
    /// Stable pane numeric identity.
    pub pane_key: u64,
    /// Application-defined view keys in tab order.
    pub view_keys: Vec<u64>,
    /// Active application-defined view key.
    pub active_view_key: u64,
    /// Pinned application-defined view keys in display order.
    pub pinned_view_keys: Vec<u64>,
    /// Transient preview view key, when present.
    pub preview_view_key: Option<u64>,
    /// First regular tab projected into the overflow viewport.
    pub tab_scroll_offset: usize,
}

/// Persistable split state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneSplitSnapshot {
    /// Stable split numeric identity.
    pub split_key: u64,
    /// Split direction.
    pub axis: PaneAxis,
    /// First-child ratio in thousandths.
    pub ratio_milli: u16,
    /// First child.
    pub first: Box<PaneNodeSnapshot>,
    /// Second child.
    pub second: Box<PaneNodeSnapshot>,
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
    /// A pinned tab cannot be converted into a transient preview tab.
    CannotPreviewPinned,
    /// A persisted pane snapshot violates pane-tree invariants.
    InvalidSnapshot(&'static str),
    /// A persisted application-defined view key could not be resolved.
    UnknownViewKey(u64),
}

impl Display for PaneError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPane(id) => write!(formatter, "unknown pane: {id}"),
            Self::UnknownView(_) => write!(formatter, "unknown document view"),
            Self::CannotCloseLastPane => write!(formatter, "cannot close the final editor pane"),
            Self::EmptyPane => write!(formatter, "pane must own at least one document view"),
            Self::CannotPreviewPinned => write!(formatter, "pinned tabs cannot be previews"),
            Self::InvalidSnapshot(message) => write!(formatter, "invalid pane snapshot: {message}"),
            Self::UnknownViewKey(key) => write!(formatter, "unknown persisted view key: {key}"),
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
                pinned_views: Vec::new(),
                preview_view: None,
                tab_scroll_offset: 0,
            }),
            focused_pane: root_id,
        }
    }

    /// Returns the recursive root node.
    #[must_use]
    pub const fn root(&self) -> &PaneNode {
        &self.root
    }

    /// Captures topology and pane-local tab state using application-defined view keys.
    #[must_use]
    pub fn snapshot_with<F>(&self, mut view_key: F) -> PaneTreeSnapshot
    where
        F: FnMut(DocumentViewId) -> u64,
    {
        PaneTreeSnapshot {
            focused_pane_key: self.focused_pane.value(),
            root: snapshot_node(&self.root, &mut view_key),
        }
    }

    /// Restores a validated pane tree by resolving application-defined view keys.
    ///
    /// # Errors
    ///
    /// Returns [`PaneError`] when identities are duplicated, required views are missing, or tab
    /// metadata violates the pinned/preview/active invariants.
    pub fn restore_with<F>(
        snapshot: &PaneTreeSnapshot,
        mut resolve_view: F,
    ) -> Result<Self, PaneError>
    where
        F: FnMut(u64) -> Option<DocumentViewId>,
    {
        let mut pane_keys = BTreeSet::new();
        let mut view_keys = BTreeSet::new();
        let root = restore_node(
            &snapshot.root,
            &mut resolve_view,
            &mut pane_keys,
            &mut view_keys,
        )?;
        let focused_pane = PaneId(snapshot.focused_pane_key);
        if snapshot.focused_pane_key == 0 || find_leaf(&root, focused_pane).is_none() {
            return Err(PaneError::InvalidSnapshot("focused pane is not a leaf"));
        }
        let next_id = pane_keys
            .iter()
            .next_back()
            .copied()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1);
        Ok(Self {
            next_id,
            root,
            focused_pane,
        })
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

    /// Reorders a view inside one pane while preserving the pinned-first partition.
    pub fn reorder_view(
        &mut self,
        pane_id: PaneId,
        view_id: DocumentViewId,
        target_index: usize,
    ) -> Result<(), PaneError> {
        let leaf = find_leaf_mut(&mut self.root, pane_id).ok_or(PaneError::UnknownPane(pane_id))?;
        let current = leaf
            .views
            .iter()
            .position(|candidate| *candidate == view_id)
            .ok_or(PaneError::UnknownView(view_id))?;
        let is_pinned = leaf.pinned_views.contains(&view_id);
        let adjusted_target = if current < target_index {
            target_index.saturating_sub(1)
        } else {
            target_index
        };
        leaf.views.remove(current);
        let pinned_count = leaf.pinned_views.len();
        let insertion = if is_pinned {
            adjusted_target.min(pinned_count.saturating_sub(1))
        } else {
            adjusted_target.clamp(pinned_count, leaf.views.len())
        };
        leaf.views.insert(insertion, view_id);
        if is_pinned {
            leaf.pinned_views.retain(|candidate| *candidate != view_id);
            let pin_index = insertion.min(leaf.pinned_views.len());
            leaf.pinned_views.insert(pin_index, view_id);
        }
        leaf.active_view = view_id;
        leaf.tab_scroll_offset = leaf
            .tab_scroll_offset
            .min(leaf.views.len().saturating_sub(1));
        self.focused_pane = pane_id;
        Ok(())
    }

    /// Moves one view between panes, collapsing an emptied source pane.
    pub fn move_view(
        &mut self,
        source_pane: PaneId,
        target_pane: PaneId,
        view_id: DocumentViewId,
        target_index: usize,
    ) -> Result<PaneId, PaneError> {
        if source_pane == target_pane {
            self.reorder_view(source_pane, view_id, target_index)?;
            return Ok(source_pane);
        }
        if self.leaf(target_pane).is_none() {
            return Err(PaneError::UnknownPane(target_pane));
        }
        let (is_pinned, is_preview, source_will_empty) = {
            let source = self
                .leaf(source_pane)
                .ok_or(PaneError::UnknownPane(source_pane))?;
            if !source.views.contains(&view_id) {
                return Err(PaneError::UnknownView(view_id));
            }
            (
                source.is_pinned(view_id),
                source.is_preview(view_id),
                source.views.len() == 1,
            )
        };
        {
            let source = find_leaf_mut(&mut self.root, source_pane)
                .ok_or(PaneError::UnknownPane(source_pane))?;
            source.views.retain(|candidate| *candidate != view_id);
            source
                .pinned_views
                .retain(|candidate| *candidate != view_id);
            if source.preview_view == Some(view_id) {
                source.preview_view = None;
            }
            if !source.views.is_empty() && source.active_view == view_id {
                source.active_view = source.views[0];
            }
            source.tab_scroll_offset = source
                .tab_scroll_offset
                .min(source.views.len().saturating_sub(1));
        }
        if source_will_empty {
            let _ = collapse_leaf(&mut self.root, source_pane);
        }
        let target = find_leaf_mut(&mut self.root, target_pane)
            .ok_or(PaneError::UnknownPane(target_pane))?;
        let pinned_count = target.pinned_views.len();
        let insertion = if is_pinned {
            target_index.min(pinned_count)
        } else {
            target_index.clamp(pinned_count, target.views.len())
        };
        target.views.insert(insertion, view_id);
        if is_pinned {
            target
                .pinned_views
                .insert(insertion.min(target.pinned_views.len()), view_id);
        }
        if is_preview && !is_pinned {
            target.preview_view = Some(view_id);
        }
        target.active_view = view_id;
        target.tab_scroll_offset = target
            .tab_scroll_offset
            .min(target.views.len().saturating_sub(1));
        self.focused_pane = target_pane;
        Ok(target_pane)
    }

    /// Moves the focused pane's active tab one position left within its pin partition.
    pub fn move_active_tab_left(&mut self) -> Result<(), PaneError> {
        let pane_id = self.focused_pane;
        let leaf = self.leaf(pane_id).ok_or(PaneError::UnknownPane(pane_id))?;
        let view_id = leaf.active_view;
        let current = leaf
            .views
            .iter()
            .position(|candidate| *candidate == view_id)
            .ok_or(PaneError::UnknownView(view_id))?;
        self.reorder_view(pane_id, view_id, current.saturating_sub(1))
    }

    /// Moves the focused pane's active tab one position right within its pin partition.
    pub fn move_active_tab_right(&mut self) -> Result<(), PaneError> {
        let pane_id = self.focused_pane;
        let leaf = self.leaf(pane_id).ok_or(PaneError::UnknownPane(pane_id))?;
        let view_id = leaf.active_view;
        let current = leaf
            .views
            .iter()
            .position(|candidate| *candidate == view_id)
            .ok_or(PaneError::UnknownView(view_id))?;
        self.reorder_view(pane_id, view_id, current.saturating_add(2))
    }

    /// Moves the focused pane's active tab to the previous pane in depth-first order.
    pub fn move_active_tab_to_previous_pane(&mut self) -> Result<PaneId, PaneError> {
        self.move_active_tab_to_adjacent_pane(-1)
    }

    /// Moves the focused pane's active tab to the next pane in depth-first order.
    pub fn move_active_tab_to_next_pane(&mut self) -> Result<PaneId, PaneError> {
        self.move_active_tab_to_adjacent_pane(1)
    }

    /// Pins a view and moves it into the pane's leading pinned partition.
    pub fn pin_view(&mut self, pane_id: PaneId, view_id: DocumentViewId) -> Result<(), PaneError> {
        let leaf = find_leaf_mut(&mut self.root, pane_id).ok_or(PaneError::UnknownPane(pane_id))?;
        let index = leaf
            .views
            .iter()
            .position(|candidate| *candidate == view_id)
            .ok_or(PaneError::UnknownView(view_id))?;
        if leaf.pinned_views.contains(&view_id) {
            return Ok(());
        }
        leaf.views.remove(index);
        let insertion = leaf.pinned_views.len();
        leaf.views.insert(insertion, view_id);
        leaf.pinned_views.push(view_id);
        if leaf.preview_view == Some(view_id) {
            leaf.preview_view = None;
        }
        leaf.active_view = view_id;
        leaf.tab_scroll_offset = leaf
            .tab_scroll_offset
            .min(leaf.views.len().saturating_sub(1));
        self.focused_pane = pane_id;
        Ok(())
    }

    /// Unpins a view and moves it to the first regular-tab position.
    pub fn unpin_view(
        &mut self,
        pane_id: PaneId,
        view_id: DocumentViewId,
    ) -> Result<(), PaneError> {
        let leaf = find_leaf_mut(&mut self.root, pane_id).ok_or(PaneError::UnknownPane(pane_id))?;
        if !leaf.views.contains(&view_id) {
            return Err(PaneError::UnknownView(view_id));
        }
        if !leaf.pinned_views.contains(&view_id) {
            return Ok(());
        }
        leaf.views.retain(|candidate| *candidate != view_id);
        leaf.pinned_views.retain(|candidate| *candidate != view_id);
        let insertion = leaf.pinned_views.len();
        leaf.views.insert(insertion, view_id);
        leaf.active_view = view_id;
        leaf.tab_scroll_offset = 0;
        self.focused_pane = pane_id;
        Ok(())
    }

    /// Marks one unpinned view as the pane-local preview tab.
    pub fn set_preview_view(
        &mut self,
        pane_id: PaneId,
        view_id: DocumentViewId,
    ) -> Result<Option<DocumentViewId>, PaneError> {
        let leaf = find_leaf_mut(&mut self.root, pane_id).ok_or(PaneError::UnknownPane(pane_id))?;
        if !leaf.views.contains(&view_id) {
            return Err(PaneError::UnknownView(view_id));
        }
        if leaf.pinned_views.contains(&view_id) {
            return Err(PaneError::CannotPreviewPinned);
        }
        let previous = leaf.preview_view.replace(view_id);
        leaf.active_view = view_id;
        self.focused_pane = pane_id;
        Ok(previous.filter(|previous| *previous != view_id))
    }

    /// Promotes a preview view into a normal persistent tab.
    pub fn promote_preview(
        &mut self,
        pane_id: PaneId,
        view_id: DocumentViewId,
    ) -> Result<(), PaneError> {
        let leaf = find_leaf_mut(&mut self.root, pane_id).ok_or(PaneError::UnknownPane(pane_id))?;
        if !leaf.views.contains(&view_id) {
            return Err(PaneError::UnknownView(view_id));
        }
        if leaf.preview_view == Some(view_id) {
            leaf.preview_view = None;
        }
        Ok(())
    }

    /// Updates the regular-tab overflow offset for one pane.
    pub fn set_tab_scroll_offset(
        &mut self,
        pane_id: PaneId,
        offset: usize,
    ) -> Result<(), PaneError> {
        let leaf = find_leaf_mut(&mut self.root, pane_id).ok_or(PaneError::UnknownPane(pane_id))?;
        let regular_count = leaf.views.len().saturating_sub(leaf.pinned_views.len());
        leaf.tab_scroll_offset = offset.min(regular_count.saturating_sub(1));
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
            pinned_views: Vec::new(),
            preview_view: None,
            tab_scroll_offset: 0,
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
            leaf.pinned_views.retain(|candidate| *candidate != view_id);
            if leaf.preview_view == Some(view_id) {
                leaf.preview_view = None;
            }
            let next_index = view_index.min(leaf.views.len().saturating_sub(1));
            if leaf.active_view == view_id {
                leaf.active_view = leaf.views[next_index];
            }
            leaf.tab_scroll_offset = leaf
                .tab_scroll_offset
                .min(leaf.views.len().saturating_sub(1));
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

    fn move_active_tab_to_adjacent_pane(&mut self, delta: i32) -> Result<PaneId, PaneError> {
        let source = self.focused_pane;
        let view_id = self.focused_view();
        let leaf_ids = self
            .leaves()
            .into_iter()
            .map(PaneLeaf::id)
            .collect::<Vec<_>>();
        if leaf_ids.len() < 2 {
            return Ok(source);
        }
        let current = leaf_ids
            .iter()
            .position(|pane_id| *pane_id == source)
            .unwrap_or(0);
        let count = i32::try_from(leaf_ids.len()).unwrap_or(i32::MAX);
        let target_index = (i32::try_from(current).unwrap_or(0) + delta).rem_euclid(count);
        let target = leaf_ids[usize::try_from(target_index).unwrap_or(0)];
        let insertion = self.leaf(target).map_or(0, |leaf| leaf.views.len());
        self.move_view(source, target, view_id, insertion)
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

fn snapshot_node<F>(node: &PaneNode, view_key: &mut F) -> PaneNodeSnapshot
where
    F: FnMut(DocumentViewId) -> u64,
{
    match node {
        PaneNode::Leaf(leaf) => PaneNodeSnapshot::Leaf(PaneLeafSnapshot {
            pane_key: leaf.id.value(),
            view_keys: leaf.views.iter().copied().map(&mut *view_key).collect(),
            active_view_key: view_key(leaf.active_view),
            pinned_view_keys: leaf
                .pinned_views
                .iter()
                .copied()
                .map(&mut *view_key)
                .collect(),
            preview_view_key: leaf.preview_view.map(|view| view_key(view)),
            tab_scroll_offset: leaf.tab_scroll_offset,
        }),
        PaneNode::Split(split) => PaneNodeSnapshot::Split(PaneSplitSnapshot {
            split_key: split.id.value(),
            axis: split.axis,
            ratio_milli: split.ratio_milli,
            first: Box::new(snapshot_node(&split.first, view_key)),
            second: Box::new(snapshot_node(&split.second, view_key)),
        }),
    }
}

fn restore_node<F>(
    snapshot: &PaneNodeSnapshot,
    resolve_view: &mut F,
    pane_keys: &mut BTreeSet<u64>,
    view_keys: &mut BTreeSet<u64>,
) -> Result<PaneNode, PaneError>
where
    F: FnMut(u64) -> Option<DocumentViewId>,
{
    match snapshot {
        PaneNodeSnapshot::Leaf(leaf) => {
            validate_pane_key(leaf.pane_key, pane_keys)?;
            if leaf.view_keys.is_empty() {
                return Err(PaneError::InvalidSnapshot("leaf has no views"));
            }
            let mut views = Vec::with_capacity(leaf.view_keys.len());
            for key in &leaf.view_keys {
                if !view_keys.insert(*key) {
                    return Err(PaneError::InvalidSnapshot("view appears in multiple panes"));
                }
                views.push(resolve_view(*key).ok_or(PaneError::UnknownViewKey(*key))?);
            }
            let active_index = leaf
                .view_keys
                .iter()
                .position(|key| *key == leaf.active_view_key)
                .ok_or(PaneError::InvalidSnapshot("active view is not pane-local"))?;
            let mut pinned_views = Vec::with_capacity(leaf.pinned_view_keys.len());
            let mut pinned_keys = BTreeSet::new();
            for key in &leaf.pinned_view_keys {
                if !pinned_keys.insert(*key) {
                    return Err(PaneError::InvalidSnapshot("pinned view is duplicated"));
                }
                let index = leaf
                    .view_keys
                    .iter()
                    .position(|candidate| candidate == key)
                    .ok_or(PaneError::InvalidSnapshot("pinned view is not pane-local"))?;
                pinned_views.push(views[index]);
            }
            if leaf.view_keys.get(..pinned_views.len()) != Some(leaf.pinned_view_keys.as_slice()) {
                return Err(PaneError::InvalidSnapshot(
                    "pinned views are not the leading partition",
                ));
            }
            let preview_view = match leaf.preview_view_key {
                Some(key) => {
                    if pinned_keys.contains(&key) {
                        return Err(PaneError::InvalidSnapshot("preview view is pinned"));
                    }
                    let index = leaf
                        .view_keys
                        .iter()
                        .position(|candidate| *candidate == key)
                        .ok_or(PaneError::InvalidSnapshot("preview view is not pane-local"))?;
                    Some(views[index])
                }
                None => None,
            };
            let regular_count = views.len().saturating_sub(pinned_views.len());
            let maximum_offset = regular_count.saturating_sub(1);
            if leaf.tab_scroll_offset > maximum_offset {
                return Err(PaneError::InvalidSnapshot(
                    "tab overflow offset is out of range",
                ));
            }
            Ok(PaneNode::Leaf(PaneLeaf {
                id: PaneId(leaf.pane_key),
                active_view: views[active_index],
                views,
                pinned_views,
                preview_view,
                tab_scroll_offset: leaf.tab_scroll_offset,
            }))
        }
        PaneNodeSnapshot::Split(split) => {
            validate_pane_key(split.split_key, pane_keys)?;
            if !(100..=900).contains(&split.ratio_milli) {
                return Err(PaneError::InvalidSnapshot(
                    "split ratio is outside safe bounds",
                ));
            }
            Ok(PaneNode::Split(PaneSplit {
                id: PaneId(split.split_key),
                axis: split.axis,
                ratio_milli: split.ratio_milli,
                first: Box::new(restore_node(
                    &split.first,
                    resolve_view,
                    pane_keys,
                    view_keys,
                )?),
                second: Box::new(restore_node(
                    &split.second,
                    resolve_view,
                    pane_keys,
                    view_keys,
                )?),
            }))
        }
    }
}

fn validate_pane_key(key: u64, pane_keys: &mut BTreeSet<u64>) -> Result<(), PaneError> {
    if key == 0 {
        return Err(PaneError::InvalidSnapshot("pane identity is zero"));
    }
    if key == u64::MAX {
        return Err(PaneError::InvalidSnapshot(
            "pane identity leaves no room for future allocation",
        ));
    }
    if !pane_keys.insert(key) {
        return Err(PaneError::InvalidSnapshot("pane identity is duplicated"));
    }
    Ok(())
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
    use super::{
        PaneAxis, PaneError, PaneLayoutMetrics, PaneNodeSnapshot, PaneTree, PaneTreeSnapshot,
    };
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

    #[test]
    fn pinned_preview_and_reorder_invariants_are_preserved() -> TestResult {
        let (mut documents, mut registry, first_view) = views()?;
        let second_view = registry.create_view(document(&mut documents, "two")?);
        let third_view = registry.create_view(document(&mut documents, "three")?);
        let mut panes = PaneTree::new(first_view);
        let pane = panes.focused_pane();
        panes.add_view(pane, second_view)?;
        panes.add_view(pane, third_view)?;
        panes.pin_view(pane, third_view)?;
        assert_eq!(
            panes.leaf(pane).map(|leaf| leaf.views()),
            Some(&[third_view, first_view, second_view][..])
        );
        assert!(panes.set_preview_view(pane, second_view)?.is_none());
        assert_eq!(
            panes.leaf(pane).and_then(|leaf| leaf.preview_view()),
            Some(second_view)
        );
        panes.reorder_view(pane, second_view, 0)?;
        assert_eq!(
            panes.leaf(pane).map(|leaf| leaf.views()),
            Some(&[third_view, second_view, first_view][..])
        );
        assert_eq!(
            panes.set_preview_view(pane, third_view),
            Err(PaneError::CannotPreviewPinned)
        );
        panes.promote_preview(pane, second_view)?;
        assert_eq!(panes.leaf(pane).and_then(|leaf| leaf.preview_view()), None);
        Ok(())
    }

    #[test]
    fn reordering_regular_tabs_right_uses_display_insertion_coordinates() -> TestResult {
        let (mut documents, mut registry, first_view) = views()?;
        let second_view = registry.create_view(document(&mut documents, "two")?);
        let third_view = registry.create_view(document(&mut documents, "three")?);
        let mut panes = PaneTree::new(first_view);
        let pane = panes.focused_pane();
        panes.add_view(pane, second_view)?;
        panes.add_view(pane, third_view)?;

        panes.reorder_view(pane, first_view, 3)?;

        assert_eq!(
            panes.leaf(pane).map(|leaf| leaf.views()),
            Some(&[second_view, third_view, first_view][..])
        );
        Ok(())
    }

    #[test]
    fn moving_the_only_view_collapses_source_and_preserves_metadata() -> TestResult {
        let (mut documents, mut registry, first_view) = views()?;
        let second_view = registry.create_view(document(&mut documents, "two")?);
        let mut panes = PaneTree::new(first_view);
        let first_pane = panes.focused_pane();
        let second_pane = panes.split_focused(PaneAxis::Horizontal, second_view);
        panes.pin_view(first_pane, first_view)?;
        panes.move_view(first_pane, second_pane, first_view, 0)?;
        assert_eq!(panes.leaves().len(), 1);
        let leaf = panes
            .leaf(second_pane)
            .ok_or(PaneError::UnknownPane(second_pane))?;
        assert_eq!(leaf.views(), &[first_view, second_view]);
        assert!(leaf.is_pinned(first_view));
        Ok(())
    }

    #[test]
    fn pane_snapshot_round_trips_topology_tabs_focus_and_metadata() -> TestResult {
        let (mut documents, mut registry, first_view) = views()?;
        let second_view = registry.create_view(document(&mut documents, "two")?);
        let third_view = registry.create_view(document(&mut documents, "three")?);
        let mut panes = PaneTree::new(first_view);
        let first_pane = panes.focused_pane();
        panes.add_view(first_pane, second_view)?;
        panes.pin_view(first_pane, first_view)?;
        panes.set_preview_view(first_pane, second_view)?;
        let second_pane = panes.split_focused(PaneAxis::Vertical, third_view);
        panes.focus(first_pane)?;
        panes.set_tab_scroll_offset(first_pane, 0)?;

        let snapshot = panes.snapshot_with(luna_documents::DocumentViewId::value);
        let restored = PaneTree::restore_with(&snapshot, |key| {
            registry
                .views()
                .iter()
                .find(|view| view.id().value() == key)
                .map(|view| view.id())
        })?;

        assert_eq!(restored, panes);
        assert_eq!(restored.focused_pane(), first_pane);
        assert!(restored.leaf(second_pane).is_some());
        Ok(())
    }

    #[test]
    fn pane_snapshot_rejects_duplicate_view_ownership() -> TestResult {
        let (_documents, registry, first_view) = views()?;
        let snapshot = PaneTreeSnapshot {
            focused_pane_key: 1,
            root: PaneNodeSnapshot::Split(super::PaneSplitSnapshot {
                split_key: 2,
                axis: PaneAxis::Horizontal,
                ratio_milli: 500,
                first: Box::new(PaneNodeSnapshot::Leaf(super::PaneLeafSnapshot {
                    pane_key: 1,
                    view_keys: vec![first_view.value()],
                    active_view_key: first_view.value(),
                    pinned_view_keys: Vec::new(),
                    preview_view_key: None,
                    tab_scroll_offset: 0,
                })),
                second: Box::new(PaneNodeSnapshot::Leaf(super::PaneLeafSnapshot {
                    pane_key: 3,
                    view_keys: vec![first_view.value()],
                    active_view_key: first_view.value(),
                    pinned_view_keys: Vec::new(),
                    preview_view_key: None,
                    tab_scroll_offset: 0,
                })),
            }),
        };
        assert_eq!(
            PaneTree::restore_with(&snapshot, |key| {
                registry
                    .views()
                    .iter()
                    .find(|view| view.id().value() == key)
                    .map(|view| view.id())
            }),
            Err(PaneError::InvalidSnapshot("view appears in multiple panes"))
        );
        Ok(())
    }

    #[test]
    fn keyboard_tab_commands_preserve_partitions_and_move_between_panes() -> TestResult {
        let (mut documents, mut registry, first_view) = views()?;
        let second_view = registry.create_view(document(&mut documents, "two")?);
        let third_view = registry.create_view(document(&mut documents, "three")?);
        let mut panes = PaneTree::new(first_view);
        let first_pane = panes.focused_pane();
        panes.add_view(first_pane, second_view)?;
        panes.pin_view(first_pane, first_view)?;
        panes.move_active_tab_left()?;
        assert_eq!(
            panes.leaf(first_pane).map(|leaf| leaf.views()),
            Some(&[first_view, second_view][..])
        );
        let second_pane = panes.split_focused(PaneAxis::Horizontal, third_view);
        panes.focus(first_pane)?;
        panes.activate_view(first_pane, second_view)?;
        assert_eq!(panes.move_active_tab_to_next_pane()?, second_pane);
        assert_eq!(panes.pane_for_view(second_view), Some(second_pane));
        Ok(())
    }

    #[test]
    fn tab_scroll_offsets_clamp_to_regular_tabs() -> TestResult {
        let (mut documents, mut registry, first_view) = views()?;
        let second_view = registry.create_view(document(&mut documents, "two")?);
        let third_view = registry.create_view(document(&mut documents, "three")?);
        let mut panes = PaneTree::new(first_view);
        let pane = panes.focused_pane();
        panes.add_view(pane, second_view)?;
        panes.add_view(pane, third_view)?;
        panes.pin_view(pane, first_view)?;
        panes.set_tab_scroll_offset(pane, 99)?;
        assert_eq!(
            panes.leaf(pane).map(|leaf| leaf.tab_scroll_offset()),
            Some(1)
        );
        Ok(())
    }
}
