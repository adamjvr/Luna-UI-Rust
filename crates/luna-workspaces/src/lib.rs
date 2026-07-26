// SPDX-License-Identifier: MPL-2.0

//! Product-neutral workspace trees and filesystem scan adapters.
//!
//! This crate owns stable workspace-node identities, recursive immutable snapshots, expansion and
//! selection state, visible-row flattening, refresh preservation, synchronous scan and mutation
//! boundaries, and native-watcher delivery contracts. It intentionally does not own native dialogs,
//! editor tabs, command policy, or application-specific confirmation behavior.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

/// Stable identity for one filesystem path inside a workspace snapshot.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WorkspaceNodeId(String);

impl WorkspaceNodeId {
    /// Builds an exact deterministic ID from an absolute normalized path.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when `path` is relative or contains `.` or `..` components.
    pub fn from_absolute_path(path: &Path) -> Result<Self, WorkspaceError> {
        validate_absolute_path(path)?;
        let bytes = path_bytes(path.as_os_str());
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut value = String::with_capacity(15 + bytes.len().saturating_mul(2));
        value.push_str("workspace-node-");
        for byte in bytes {
            value.push(char::from(HEX[usize::from(byte >> 4)]));
            value.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        Ok(Self(value))
    }

    /// Returns the stable string used by UI and accessibility projections.
    #[must_use]
    pub fn stable_key(&self) -> &str {
        &self.0
    }
}

/// Kind of filesystem object represented by a workspace node.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WorkspaceNodeKind {
    /// Directory that may contain children.
    Directory,
    /// Regular file that may be opened as a document.
    File,
    /// Symbolic link shown as a non-followed leaf.
    Symlink,
}

impl WorkspaceNodeKind {
    const fn sort_rank(self) -> u8 {
        match self {
            Self::Directory => 0,
            Self::File => 1,
            Self::Symlink => 2,
        }
    }
}

/// Availability state for one workspace node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceNodeStatus {
    /// Node was observed successfully.
    Available,
    /// Directory or entry could not be read due to permissions.
    PermissionDenied,
    /// Recursive scanning stopped at the configured depth.
    DepthLimit,
    /// Another I/O failure occurred while observing the node.
    Unreadable(String),
}

impl WorkspaceNodeStatus {
    /// Returns whether the node is fully available.
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

/// One immutable node in a recursive workspace snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceNode {
    id: WorkspaceNodeId,
    path: PathBuf,
    relative_path: PathBuf,
    title: String,
    kind: WorkspaceNodeKind,
    status: WorkspaceNodeStatus,
    children: Vec<WorkspaceNodeId>,
}

impl WorkspaceNode {
    fn new(
        path: PathBuf,
        relative_path: PathBuf,
        title: String,
        kind: WorkspaceNodeKind,
        status: WorkspaceNodeStatus,
    ) -> Result<Self, WorkspaceError> {
        let id = WorkspaceNodeId::from_absolute_path(&path)?;
        Ok(Self {
            id,
            path,
            relative_path,
            title,
            kind,
            status,
            children: Vec::new(),
        })
    }

    /// Returns the stable node identity.
    #[must_use]
    pub const fn id(&self) -> &WorkspaceNodeId {
        &self.id
    }

    /// Returns the absolute normalized path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the path relative to the workspace root.
    #[must_use]
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    /// Returns the display title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the filesystem object kind.
    #[must_use]
    pub const fn kind(&self) -> WorkspaceNodeKind {
        self.kind
    }

    /// Returns the availability state.
    #[must_use]
    pub const fn status(&self) -> &WorkspaceNodeStatus {
        &self.status
    }

    /// Returns ordered child identities.
    #[must_use]
    pub fn children(&self) -> &[WorkspaceNodeId] {
        &self.children
    }
}

/// Policy controlling dot-prefixed entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HiddenFilePolicy {
    /// Exclude dot-prefixed entries.
    Exclude,
    /// Include dot-prefixed entries.
    Include,
}

/// Policy controlling symbolic links.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymlinkPolicy {
    /// Exclude symbolic links completely.
    Exclude,
    /// Show symbolic links as leaves without following their targets.
    ShowAsLeaf,
}

/// Recursive workspace-scan configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceScanOptions {
    /// Dot-file visibility policy.
    pub hidden_files: HiddenFilePolicy,
    /// Symbolic-link visibility policy.
    pub symlinks: SymlinkPolicy,
    /// Maximum recursive depth below the root.
    pub maximum_depth: u16,
}

impl Default for WorkspaceScanOptions {
    fn default() -> Self {
        Self {
            hidden_files: HiddenFilePolicy::Exclude,
            symlinks: SymlinkPolicy::ShowAsLeaf,
            maximum_depth: 64,
        }
    }
}

/// Immutable recursive filesystem snapshot for one workspace root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSnapshot {
    root: PathBuf,
    root_id: WorkspaceNodeId,
    nodes: BTreeMap<WorkspaceNodeId, WorkspaceNode>,
    path_index: BTreeMap<PathBuf, WorkspaceNodeId>,
    fingerprint: u64,
}

impl WorkspaceSnapshot {
    fn from_nodes(
        root: PathBuf,
        root_id: WorkspaceNodeId,
        nodes: BTreeMap<WorkspaceNodeId, WorkspaceNode>,
    ) -> Result<Self, WorkspaceError> {
        if !nodes.contains_key(&root_id) {
            return Err(WorkspaceError::invalid_snapshot(
                &root,
                "workspace snapshot does not contain its root node",
            ));
        }
        let path_index = nodes
            .values()
            .map(|node| (node.path.clone(), node.id.clone()))
            .collect();
        let fingerprint = snapshot_fingerprint(&nodes);
        Ok(Self {
            root,
            root_id,
            nodes,
            path_index,
            fingerprint,
        })
    }

    /// Returns the canonical workspace root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the root node identity.
    #[must_use]
    pub const fn root_id(&self) -> &WorkspaceNodeId {
        &self.root_id
    }

    /// Returns one node by stable identity.
    #[must_use]
    pub fn node(&self, id: &WorkspaceNodeId) -> Option<&WorkspaceNode> {
        self.nodes.get(id)
    }

    /// Returns one node by its stable UI key.
    #[must_use]
    pub fn node_for_stable_key(&self, stable_key: &str) -> Option<&WorkspaceNode> {
        self.nodes
            .values()
            .find(|node| node.id.stable_key() == stable_key)
    }

    /// Returns one node by absolute normalized path.
    #[must_use]
    pub fn node_for_path(&self, path: &Path) -> Option<&WorkspaceNode> {
        self.path_index.get(path).and_then(|id| self.nodes.get(id))
    }

    /// Returns whether the snapshot contains an identity.
    #[must_use]
    pub fn contains(&self, id: &WorkspaceNodeId) -> bool {
        self.nodes.contains_key(id)
    }

    /// Returns all nodes in stable identity order.
    #[must_use]
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = &WorkspaceNode> {
        self.nodes.values()
    }

    /// Returns a deterministic content fingerprint for refresh suppression.
    #[must_use]
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Reconciles one rescanned directory subtree into this immutable snapshot.
    ///
    /// Pass `None` when the subtree was removed. Stable path-derived identities outside the target
    /// remain byte-for-byte unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when the target escapes the workspace root or a replacement
    /// snapshot does not describe the requested subtree.
    pub fn reconcile_subtree(
        &self,
        target: &Path,
        replacement: Option<&WorkspaceSnapshot>,
    ) -> Result<Self, WorkspaceError> {
        validate_absolute_path(target)?;
        if !target.starts_with(&self.root) {
            return Err(WorkspaceError::invalid_path(
                target,
                "incremental refresh target escaped the workspace root",
            ));
        }
        if target == self.root {
            return replacement.cloned().ok_or_else(|| {
                WorkspaceError::invalid_snapshot(
                    target,
                    "the workspace root cannot be removed by subtree reconciliation",
                )
            });
        }
        if let Some(snapshot) = replacement
            && snapshot.root != target
        {
            return Err(WorkspaceError::invalid_snapshot(
                target,
                "replacement snapshot root does not match the refresh target",
            ));
        }
        let mut nodes = self
            .nodes
            .values()
            .filter(|node| !node.path.starts_with(target))
            .cloned()
            .map(|node| (node.id.clone(), node))
            .collect::<BTreeMap<_, _>>();
        if let Some(snapshot) = replacement {
            for replacement_node in snapshot.nodes.values() {
                let mut node = replacement_node.clone();
                node.relative_path = node
                    .path
                    .strip_prefix(&self.root)
                    .unwrap_or(node.path.as_path())
                    .to_path_buf();
                node.children.clear();
                let _ = nodes.insert(node.id.clone(), node);
            }
        }
        rebuild_children(&mut nodes);
        WorkspaceSnapshot::from_nodes(self.root.clone(), self.root_id.clone(), nodes)
    }
}

/// One flattened visible row produced from expansion state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceVisibleRow {
    /// Stable node identity.
    pub id: WorkspaceNodeId,
    /// Absolute filesystem path.
    pub path: PathBuf,
    /// Display title.
    pub title: String,
    /// Hierarchy depth, with the root at zero.
    pub depth: u16,
    /// Filesystem object kind.
    pub kind: WorkspaceNodeKind,
    /// Whether this directory is expanded.
    pub is_expanded: bool,
    /// Availability state.
    pub status: WorkspaceNodeStatus,
}

/// Mutable expansion and selection state over an immutable workspace snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceModel {
    snapshot: WorkspaceSnapshot,
    expanded: BTreeSet<WorkspaceNodeId>,
    selected: Option<WorkspaceNodeId>,
    generation: u64,
}

impl WorkspaceModel {
    /// Creates a model with the root directory expanded.
    #[must_use]
    pub fn new(snapshot: WorkspaceSnapshot) -> Self {
        let mut expanded = BTreeSet::new();
        let _ = expanded.insert(snapshot.root_id.clone());
        Self {
            snapshot,
            expanded,
            selected: None,
            generation: 1,
        }
    }

    /// Returns the immutable snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &WorkspaceSnapshot {
        &self.snapshot
    }

    /// Returns a monotonic generation incremented after meaningful refreshes.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the selected node identity.
    #[must_use]
    pub const fn selected(&self) -> Option<&WorkspaceNodeId> {
        self.selected.as_ref()
    }

    /// Selects a node that exists in the current snapshot.
    pub fn select(&mut self, id: Option<WorkspaceNodeId>) -> bool {
        let next = id.filter(|candidate| self.snapshot.contains(candidate));
        if self.selected == next {
            return false;
        }
        self.selected = next;
        true
    }

    /// Selects a node and expands all surviving directory ancestors so its row is visible.
    pub fn reveal(&mut self, id: &WorkspaceNodeId) -> bool {
        let Some(node) = self.snapshot.node(id) else {
            return false;
        };
        let mut ancestors = Vec::new();
        let mut parent = node.path.parent().map(Path::to_path_buf);
        while let Some(path) = parent {
            let Some(ancestor) = self.snapshot.node_for_path(&path) else {
                break;
            };
            if ancestor.kind == WorkspaceNodeKind::Directory && ancestor.status.is_available() {
                ancestors.push(ancestor.id.clone());
            }
            if ancestor.id() == self.snapshot.root_id() {
                break;
            }
            parent = path.parent().map(Path::to_path_buf);
        }
        let mut changed = self.selected.as_ref() != Some(id);
        self.selected = Some(id.clone());
        for ancestor in ancestors {
            changed |= self.expanded.insert(ancestor);
        }
        changed
    }

    /// Toggles one available directory, returning whether state changed.
    pub fn toggle_expanded(&mut self, id: &WorkspaceNodeId) -> bool {
        let Some(node) = self.snapshot.node(id) else {
            return false;
        };
        if node.kind != WorkspaceNodeKind::Directory || !node.status.is_available() {
            return false;
        }
        if self.expanded.remove(id) {
            true
        } else {
            self.expanded.insert(id.clone())
        }
    }

    /// Returns whether a directory is expanded.
    #[must_use]
    pub fn is_expanded(&self, id: &WorkspaceNodeId) -> bool {
        self.expanded.contains(id)
    }

    /// Returns expanded directory paths in deterministic order.
    #[must_use]
    pub fn expanded_paths(&self) -> Vec<PathBuf> {
        self.expanded
            .iter()
            .filter_map(|id| self.snapshot.node(id))
            .map(|node| node.path.clone())
            .collect()
    }

    /// Restores expansion and selection state from persisted paths.
    pub fn restore_tree_state(
        &mut self,
        expanded_paths: &[PathBuf],
        selected_path: Option<&Path>,
    ) -> bool {
        let mut next = BTreeSet::new();
        let _ = next.insert(self.snapshot.root_id.clone());
        for path in expanded_paths {
            if let Some(node) = self.snapshot.node_for_path(path)
                && node.kind == WorkspaceNodeKind::Directory
                && node.status.is_available()
            {
                let _ = next.insert(node.id.clone());
            }
        }
        let selected = selected_path
            .and_then(|path| self.snapshot.node_for_path(path))
            .map(|node| node.id.clone());
        let changed = self.expanded != next || self.selected != selected;
        self.expanded = next;
        self.selected = selected;
        changed
    }

    /// Replaces the snapshot while preserving surviving expansion and selection state.
    pub fn refresh(&mut self, snapshot: WorkspaceSnapshot) -> bool {
        if self.snapshot == snapshot {
            return false;
        }
        self.expanded.retain(|id| {
            snapshot.node(id).is_some_and(|node| {
                node.kind == WorkspaceNodeKind::Directory && node.status.is_available()
            })
        });
        let _ = self.expanded.insert(snapshot.root_id.clone());
        self.selected = self.selected.take().filter(|id| snapshot.contains(id));
        self.snapshot = snapshot;
        self.generation = self.generation.saturating_add(1);
        true
    }

    /// Applies coalesced watcher events using a full or minimal subtree scan.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when event paths escape the workspace or the selected scan fails.
    pub fn refresh_from_events<S>(
        &mut self,
        service: &S,
        events: &[WorkspaceWatchEvent],
        options: WorkspaceScanOptions,
    ) -> Result<bool, WorkspaceError>
    where
        S: WorkspaceService + ?Sized,
    {
        let events = coalesce_watch_events(self.snapshot.root(), events)?;
        let Some(scope) = refresh_scope_for_events(self.snapshot.root(), &events)? else {
            return Ok(false);
        };
        let snapshot = match scope {
            WorkspaceRefreshScope::Full => service.scan(self.snapshot.root(), options)?,
            WorkspaceRefreshScope::Subtree(path) => match service.scan(&path, options) {
                Ok(replacement) => self.snapshot.reconcile_subtree(&path, Some(&replacement))?,
                Err(error) if error.is_not_found() => {
                    self.snapshot.reconcile_subtree(&path, None)?
                }
                Err(error) => return Err(error),
            },
        };
        Ok(self.refresh(snapshot))
    }

    /// Flattens the root and expanded descendants into visible rows.
    #[must_use]
    pub fn visible_rows(&self) -> Vec<WorkspaceVisibleRow> {
        let mut rows = Vec::new();
        self.append_visible(self.snapshot.root_id(), 0, &mut rows);
        rows
    }

    fn append_visible(
        &self,
        id: &WorkspaceNodeId,
        depth: u16,
        rows: &mut Vec<WorkspaceVisibleRow>,
    ) {
        let Some(node) = self.snapshot.node(id) else {
            return;
        };
        rows.push(WorkspaceVisibleRow {
            id: node.id.clone(),
            path: node.path.clone(),
            title: node.title.clone(),
            depth,
            kind: node.kind,
            is_expanded: self.expanded.contains(id),
            status: node.status.clone(),
        });
        if node.kind == WorkspaceNodeKind::Directory && self.expanded.contains(id) {
            for child in &node.children {
                self.append_visible(child, depth.saturating_add(1), rows);
            }
        }
    }
}

/// Collision policy for workspace mutation operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceCollisionPolicy {
    /// Fail when the destination already exists.
    FailIfExists,
    /// Replace an existing regular file, but never a directory or symbolic link.
    ReplaceFile,
}

/// Result of creating or renaming one workspace entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceMutationOutcome {
    path: PathBuf,
    kind: WorkspaceNodeKind,
}

impl WorkspaceMutationOutcome {
    /// Returns the absolute resulting path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the resulting entry kind.
    #[must_use]
    pub const fn kind(&self) -> WorkspaceNodeKind {
        self.kind
    }
}

/// Product-neutral synchronous filesystem mutation boundary.
pub trait WorkspaceMutationService {
    /// Creates an empty regular file below `parent`.
    fn create_file(
        &self,
        parent: &Path,
        name: &str,
        collision: WorkspaceCollisionPolicy,
    ) -> Result<WorkspaceMutationOutcome, WorkspaceError>;

    /// Creates one directory below `parent`.
    fn create_directory(
        &self,
        parent: &Path,
        name: &str,
    ) -> Result<WorkspaceMutationOutcome, WorkspaceError>;

    /// Renames one file or directory within its current parent directory.
    fn rename(
        &self,
        path: &Path,
        new_name: &str,
        collision: WorkspaceCollisionPolicy,
    ) -> Result<WorkspaceMutationOutcome, WorkspaceError>;

    /// Deletes one file, symlink, or directory tree.
    fn delete(&self, path: &Path) -> Result<(), WorkspaceError>;
}

/// Combined scan-and-mutation service used by editor workspace runtimes.
pub trait WorkspaceRuntimeService: WorkspaceService + WorkspaceMutationService {}

impl<T> WorkspaceRuntimeService for T where T: WorkspaceService + WorkspaceMutationService {}

/// Broad filesystem change classification delivered by native or fallback watchers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceWatchKind {
    /// One path was created.
    Created,
    /// Existing contents or metadata changed.
    Modified,
    /// One path was removed.
    Removed,
    /// One path moved or was renamed.
    Renamed,
    /// The backend could not classify the event more precisely.
    RescanRequired,
}

/// One path-level watcher event delivered to the UI thread.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceWatchEvent {
    /// Affected absolute path.
    pub path: PathBuf,
    /// Broad change classification.
    pub kind: WorkspaceWatchKind,
}

/// Native-watcher delivery boundary. Implementations must not mutate UI state directly.
pub trait WorkspaceWatchService {
    /// Starts observing one canonical workspace root.
    fn watch(&mut self, root: &Path) -> Result<(), WorkspaceError>;

    /// Drains pending events for delivery on the caller's UI thread.
    fn drain_events(&mut self) -> Result<Vec<WorkspaceWatchEvent>, WorkspaceError>;
}

/// Deterministic watcher queue for tests and host integration work.
#[derive(Clone, Debug, Default)]
pub struct MemoryWorkspaceWatchService {
    root: Option<PathBuf>,
    events: Vec<WorkspaceWatchEvent>,
}

impl MemoryWorkspaceWatchService {
    /// Appends one pending event.
    pub fn push(&mut self, event: WorkspaceWatchEvent) {
        self.events.push(event);
    }
}

impl WorkspaceWatchService for MemoryWorkspaceWatchService {
    fn watch(&mut self, root: &Path) -> Result<(), WorkspaceError> {
        validate_absolute_path(root)?;
        self.root = Some(root.to_path_buf());
        self.events.clear();
        Ok(())
    }

    fn drain_events(&mut self) -> Result<Vec<WorkspaceWatchEvent>, WorkspaceError> {
        let Some(root) = &self.root else {
            return Err(WorkspaceError::invalid_path(
                Path::new("/"),
                "workspace watcher has no active root",
            ));
        };
        if self
            .events
            .iter()
            .any(|event| !event.path.starts_with(root))
        {
            return Err(WorkspaceError::invalid_path(
                root,
                "watcher event escaped the active workspace root",
            ));
        }
        Ok(std::mem::take(&mut self.events))
    }
}

/// Coalesces watcher bursts into deterministic path-level events.
///
/// # Errors
///
/// Returns [`WorkspaceError`] when an event escapes `root`.
pub fn coalesce_watch_events(
    root: &Path,
    events: &[WorkspaceWatchEvent],
) -> Result<Vec<WorkspaceWatchEvent>, WorkspaceError> {
    validate_absolute_path(root)?;
    if events.iter().any(|event| !event.path.starts_with(root)) {
        return Err(WorkspaceError::invalid_path(
            root,
            "watcher event escaped the active workspace root",
        ));
    }
    if events.len() > 256
        || events
            .iter()
            .any(|event| event.kind == WorkspaceWatchKind::RescanRequired)
    {
        return Ok(vec![WorkspaceWatchEvent {
            path: root.to_path_buf(),
            kind: WorkspaceWatchKind::RescanRequired,
        }]);
    }
    let mut by_path = BTreeMap::<PathBuf, WorkspaceWatchKind>::new();
    for event in events {
        let next = by_path
            .get(&event.path)
            .copied()
            .map_or(event.kind, |current| {
                stronger_watch_kind(current, event.kind)
            });
        let _ = by_path.insert(event.path.clone(), next);
    }
    let paths = by_path.keys().cloned().collect::<Vec<_>>();
    by_path.retain(|path, kind| {
        *kind != WorkspaceWatchKind::Modified
            || !paths
                .iter()
                .any(|candidate| candidate != path && candidate.starts_with(path))
    });
    Ok(by_path
        .into_iter()
        .map(|(path, kind)| WorkspaceWatchEvent { path, kind })
        .collect())
}

/// Chooses the smallest safe directory refresh scope for coalesced events.
///
/// Returns `None` for an empty event set.
///
/// # Errors
///
/// Returns [`WorkspaceError`] when an event escapes `root`.
pub fn refresh_scope_for_events(
    root: &Path,
    events: &[WorkspaceWatchEvent],
) -> Result<Option<WorkspaceRefreshScope>, WorkspaceError> {
    validate_absolute_path(root)?;
    if events.is_empty() {
        return Ok(None);
    }
    if events.iter().any(|event| !event.path.starts_with(root)) {
        return Err(WorkspaceError::invalid_path(
            root,
            "refresh event escaped the workspace root",
        ));
    }
    if events
        .iter()
        .any(|event| event.kind == WorkspaceWatchKind::RescanRequired || event.path == root)
    {
        return Ok(Some(WorkspaceRefreshScope::Full));
    }
    let mut directories = events.iter().filter_map(|event| event.path.parent());
    let Some(first) = directories.next() else {
        return Ok(Some(WorkspaceRefreshScope::Full));
    };
    let mut common = first.to_path_buf();
    for directory in directories {
        while !directory.starts_with(&common) && common != root {
            let Some(parent) = common.parent() else {
                return Ok(Some(WorkspaceRefreshScope::Full));
            };
            common = parent.to_path_buf();
        }
    }
    if common == root {
        Ok(Some(WorkspaceRefreshScope::Full))
    } else {
        Ok(Some(WorkspaceRefreshScope::Subtree(common)))
    }
}

const fn stronger_watch_kind(
    current: WorkspaceWatchKind,
    incoming: WorkspaceWatchKind,
) -> WorkspaceWatchKind {
    if watch_kind_rank(incoming) > watch_kind_rank(current) {
        incoming
    } else {
        current
    }
}

const fn watch_kind_rank(kind: WorkspaceWatchKind) -> u8 {
    match kind {
        WorkspaceWatchKind::Modified => 0,
        WorkspaceWatchKind::Created => 1,
        WorkspaceWatchKind::Removed => 2,
        WorkspaceWatchKind::Renamed => 3,
        WorkspaceWatchKind::RescanRequired => 4,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PollEntry {
    is_directory: bool,
    length: u64,
    modified: Option<SystemTime>,
}

/// Safe standard-library polling fallback for systems without native watcher delivery.
#[derive(Clone, Debug, Default)]
pub struct PollingWorkspaceWatchService {
    root: Option<PathBuf>,
    entries: BTreeMap<PathBuf, PollEntry>,
}

impl WorkspaceWatchService for PollingWorkspaceWatchService {
    fn watch(&mut self, root: &Path) -> Result<(), WorkspaceError> {
        let canonical = fs::canonicalize(root)
            .map_err(|error| WorkspaceError::io("canonicalize watcher root", root, error))?;
        validate_absolute_path(&canonical)?;
        self.entries = poll_entries(&canonical)?;
        self.root = Some(canonical);
        Ok(())
    }

    fn drain_events(&mut self) -> Result<Vec<WorkspaceWatchEvent>, WorkspaceError> {
        let root = self.root.as_ref().ok_or_else(|| {
            WorkspaceError::invalid_path(Path::new("/"), "workspace watcher has no active root")
        })?;
        let next = poll_entries(root)?;
        let mut events = Vec::new();
        for (path, entry) in &next {
            match self.entries.get(path) {
                None => events.push(WorkspaceWatchEvent {
                    path: path.clone(),
                    kind: WorkspaceWatchKind::Created,
                }),
                Some(previous) if previous != entry => events.push(WorkspaceWatchEvent {
                    path: path.clone(),
                    kind: WorkspaceWatchKind::Modified,
                }),
                Some(_) => {}
            }
        }
        for path in self.entries.keys() {
            if !next.contains_key(path) {
                events.push(WorkspaceWatchEvent {
                    path: path.clone(),
                    kind: WorkspaceWatchKind::Removed,
                });
            }
        }
        self.entries = next;
        coalesce_watch_events(root, &events)
    }
}

fn poll_entries(root: &Path) -> Result<BTreeMap<PathBuf, PollEntry>, WorkspaceError> {
    let mut entries = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && path != root => {
                continue;
            }
            Err(error) => {
                return Err(WorkspaceError::io("observe watched path", &path, error));
            }
        };
        let is_directory = metadata.is_dir() && !metadata.file_type().is_symlink();
        let _ = entries.insert(
            path.clone(),
            PollEntry {
                is_directory,
                length: metadata.len(),
                modified: metadata.modified().ok(),
            },
        );
        if is_directory {
            let directory = match fs::read_dir(&path) {
                Ok(directory) => directory,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound && path != root => {
                    let _ = entries.remove(&path);
                    continue;
                }
                Err(error) => {
                    return Err(WorkspaceError::io("read watched directory", &path, error));
                }
            };
            for entry in directory {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => {
                        return Err(WorkspaceError::io("read watched entry", &path, error));
                    }
                };
                pending.push(entry.path());
            }
        }
    }
    Ok(entries)
}

#[derive(Debug, Default)]
struct InotifyWorkspaceWatchService {
    root: Option<PathBuf>,
    child: Option<Child>,
    receiver: Option<Receiver<WorkspaceWatchEvent>>,
    reader: Option<JoinHandle<()>>,
}

impl InotifyWorkspaceWatchService {
    fn start(&mut self, root: &Path) -> Result<(), WorkspaceError> {
        self.stop();
        let canonical = fs::canonicalize(root)
            .map_err(|error| WorkspaceError::io("canonicalize watcher root", root, error))?;
        validate_absolute_path(&canonical)?;
        let mut child = Command::new("inotifywait")
            .args([
                "--monitor",
                "--recursive",
                "--quiet",
                "--format",
                "%w%f\t%e",
                "--event",
                "create,modify,attrib,close_write,delete,moved_from,moved_to",
            ])
            .arg(&canonical)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| WorkspaceError::io("start inotifywait", &canonical, error))?;
        let stdout = child.stdout.take().ok_or_else(|| {
            WorkspaceError::invalid_path(&canonical, "inotifywait did not expose event output")
        })?;
        let (sender, receiver) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let Some((path, flags)) = line.rsplit_once('\t') else {
                    continue;
                };
                let kind = if flags.contains("MOVED_") {
                    WorkspaceWatchKind::Renamed
                } else if flags.contains("DELETE") {
                    WorkspaceWatchKind::Removed
                } else if flags.contains("CREATE") {
                    WorkspaceWatchKind::Created
                } else {
                    WorkspaceWatchKind::Modified
                };
                if sender
                    .send(WorkspaceWatchEvent {
                        path: PathBuf::from(path),
                        kind,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        self.root = Some(canonical);
        self.child = Some(child);
        self.receiver = Some(receiver);
        self.reader = Some(reader);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.child = None;
        self.receiver = None;
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        self.root = None;
    }

    fn drain(&mut self) -> Result<Vec<WorkspaceWatchEvent>, WorkspaceError> {
        let root = self.root.clone().ok_or_else(|| {
            WorkspaceError::invalid_path(Path::new("/"), "native watcher has no active root")
        })?;
        if let Some(child) = &mut self.child
            && let Some(status) = child
                .try_wait()
                .map_err(|error| WorkspaceError::io("inspect inotifywait", &root, error))?
        {
            return Err(WorkspaceError::invalid_path(
                &root,
                format!("inotifywait exited unexpectedly with {status}"),
            ));
        }
        let receiver = self.receiver.as_ref().ok_or_else(|| {
            WorkspaceError::invalid_path(&root, "native watcher response channel is unavailable")
        })?;
        let events = receiver.try_iter().collect::<Vec<_>>();
        coalesce_watch_events(&root, &events)
    }
}

impl Drop for InotifyWorkspaceWatchService {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug)]
enum LinuxWatchBackend {
    Native(InotifyWorkspaceWatchService),
    Polling(PollingWorkspaceWatchService),
}

/// Linux watcher that prefers recursive inotify delivery and falls back to safe polling.
#[derive(Debug)]
pub struct LinuxWorkspaceWatchService {
    backend: LinuxWatchBackend,
}

impl Default for LinuxWorkspaceWatchService {
    fn default() -> Self {
        Self {
            backend: LinuxWatchBackend::Polling(PollingWorkspaceWatchService::default()),
        }
    }
}

impl WorkspaceWatchService for LinuxWorkspaceWatchService {
    fn watch(&mut self, root: &Path) -> Result<(), WorkspaceError> {
        let mut native = InotifyWorkspaceWatchService::default();
        if native.start(root).is_ok() {
            self.backend = LinuxWatchBackend::Native(native);
            return Ok(());
        }
        let mut polling = PollingWorkspaceWatchService::default();
        polling.watch(root)?;
        self.backend = LinuxWatchBackend::Polling(polling);
        Ok(())
    }

    fn drain_events(&mut self) -> Result<Vec<WorkspaceWatchEvent>, WorkspaceError> {
        let fallback_root = match &mut self.backend {
            LinuxWatchBackend::Native(native) => match native.drain() {
                Ok(events) => return Ok(events),
                Err(_) => native.root.clone(),
            },
            LinuxWatchBackend::Polling(polling) => return polling.drain_events(),
        };
        let root = fallback_root.ok_or_else(|| {
            WorkspaceError::invalid_path(Path::new("/"), "native watcher lost its root")
        })?;
        let mut polling = PollingWorkspaceWatchService::default();
        polling.watch(&root)?;
        self.backend = LinuxWatchBackend::Polling(polling);
        Ok(vec![WorkspaceWatchEvent {
            path: root,
            kind: WorkspaceWatchKind::RescanRequired,
        }])
    }
}

/// Scope requested for an incremental workspace rescan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceRefreshScope {
    /// Rebuild the complete workspace snapshot.
    Full,
    /// Rebuild one directory subtree and reconcile it into the current snapshot.
    Subtree(PathBuf),
}

/// Synchronous adapter used to obtain recursive workspace snapshots.
pub trait WorkspaceService {
    /// Scans one directory into an immutable recursive snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when the root cannot be normalized or scanned.
    fn scan(
        &self,
        root: &Path,
        options: WorkspaceScanOptions,
    ) -> Result<WorkspaceSnapshot, WorkspaceError>;
}

/// Standard-library filesystem workspace scanner.
#[derive(Clone, Copy, Debug, Default)]
pub struct StdWorkspaceService;

impl WorkspaceService for StdWorkspaceService {
    fn scan(
        &self,
        root: &Path,
        options: WorkspaceScanOptions,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let canonical = fs::canonicalize(root)
            .map_err(|error| WorkspaceError::io("canonicalize workspace", root, error))?;
        validate_absolute_path(&canonical)?;
        let metadata = fs::metadata(&canonical)
            .map_err(|error| WorkspaceError::io("read workspace metadata", &canonical, error))?;
        if !metadata.is_dir() {
            return Err(WorkspaceError::not_directory(&canonical));
        }
        build_std_snapshot(canonical, options)
    }
}

impl WorkspaceMutationService for StdWorkspaceService {
    fn create_file(
        &self,
        parent: &Path,
        name: &str,
        collision: WorkspaceCollisionPolicy,
    ) -> Result<WorkspaceMutationOutcome, WorkspaceError> {
        let parent = canonical_directory(parent)?;
        validate_entry_name(name, &parent)?;
        let path = parent.join(name);
        match collision {
            WorkspaceCollisionPolicy::FailIfExists => {
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|error| WorkspaceError::io("create workspace file", &path, error))?;
            }
            WorkspaceCollisionPolicy::ReplaceFile => {
                if let Ok(metadata) = fs::symlink_metadata(&path)
                    && (!metadata.file_type().is_file() || metadata.file_type().is_symlink())
                {
                    return Err(WorkspaceError::already_exists(
                        "replace workspace file",
                        &path,
                        "only an existing regular file may be replaced",
                    ));
                }
                fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&path)
                    .map_err(|error| WorkspaceError::io("replace workspace file", &path, error))?;
            }
        }
        Ok(WorkspaceMutationOutcome {
            path,
            kind: WorkspaceNodeKind::File,
        })
    }

    fn create_directory(
        &self,
        parent: &Path,
        name: &str,
    ) -> Result<WorkspaceMutationOutcome, WorkspaceError> {
        let parent = canonical_directory(parent)?;
        validate_entry_name(name, &parent)?;
        let path = parent.join(name);
        fs::create_dir(&path)
            .map_err(|error| WorkspaceError::io("create workspace directory", &path, error))?;
        Ok(WorkspaceMutationOutcome {
            path,
            kind: WorkspaceNodeKind::Directory,
        })
    }

    fn rename(
        &self,
        path: &Path,
        new_name: &str,
        collision: WorkspaceCollisionPolicy,
    ) -> Result<WorkspaceMutationOutcome, WorkspaceError> {
        let canonical = path.to_path_buf();
        validate_absolute_path(&canonical)?;
        let parent = canonical.parent().ok_or_else(|| {
            WorkspaceError::invalid_path(&canonical, "workspace root cannot be renamed")
        })?;
        validate_entry_name(new_name, parent)?;
        let destination = parent.join(new_name);
        let source_metadata = fs::symlink_metadata(&canonical)
            .map_err(|error| WorkspaceError::io("read rename source", &canonical, error))?;
        let kind = if source_metadata.file_type().is_symlink() {
            WorkspaceNodeKind::Symlink
        } else if source_metadata.is_dir() {
            WorkspaceNodeKind::Directory
        } else {
            WorkspaceNodeKind::File
        };
        if destination == canonical {
            return Ok(WorkspaceMutationOutcome {
                path: canonical,
                kind,
            });
        }
        let destination_metadata = match fs::symlink_metadata(&destination) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(WorkspaceError::io(
                    "read rename destination",
                    &destination,
                    error,
                ));
            }
        };
        if let Some(destination_metadata) = destination_metadata {
            match collision {
                WorkspaceCollisionPolicy::FailIfExists => {
                    return Err(WorkspaceError::already_exists(
                        "rename workspace entry",
                        &destination,
                        "destination already exists",
                    ));
                }
                WorkspaceCollisionPolicy::ReplaceFile => {
                    if !source_metadata.file_type().is_file()
                        || !destination_metadata.file_type().is_file()
                        || destination_metadata.file_type().is_symlink()
                    {
                        return Err(WorkspaceError::already_exists(
                            "replace rename destination",
                            &destination,
                            "replacement is limited to regular files",
                        ));
                    }
                    #[cfg(not(unix))]
                    fs::remove_file(&destination).map_err(|error| {
                        WorkspaceError::io("remove rename destination", &destination, error)
                    })?;
                }
            }
        }
        fs::rename(&canonical, &destination)
            .map_err(|error| WorkspaceError::io("rename workspace entry", &canonical, error))?;
        Ok(WorkspaceMutationOutcome {
            path: destination,
            kind,
        })
    }

    fn delete(&self, path: &Path) -> Result<(), WorkspaceError> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| WorkspaceError::io("read delete target", path, error))?;
        if metadata.file_type().is_symlink() || metadata.is_file() {
            fs::remove_file(path)
                .map_err(|error| WorkspaceError::io("delete workspace file", path, error))
        } else if metadata.is_dir() {
            fs::remove_dir_all(path)
                .map_err(|error| WorkspaceError::io("delete workspace directory", path, error))
        } else {
            Err(WorkspaceError::invalid_path(
                path,
                "unsupported workspace entry type",
            ))
        }
    }
}

/// In-memory workspace scanner for deterministic tests and product harnesses.
#[derive(Clone, Debug)]
pub struct MemoryWorkspaceService {
    root: PathBuf,
    entries: Rc<RefCell<BTreeMap<PathBuf, MemoryWorkspaceEntry>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MemoryWorkspaceEntry {
    kind: WorkspaceNodeKind,
    status: WorkspaceNodeStatus,
}

impl MemoryWorkspaceService {
    /// Creates an empty in-memory workspace with one available root directory.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when `root` is not absolute and normalized.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, WorkspaceError> {
        let root = root.into();
        validate_absolute_path(&root)?;
        let mut entries = BTreeMap::new();
        let _ = entries.insert(
            root.clone(),
            MemoryWorkspaceEntry {
                kind: WorkspaceNodeKind::Directory,
                status: WorkspaceNodeStatus::Available,
            },
        );
        Ok(Self {
            root,
            entries: Rc::new(RefCell::new(entries)),
        })
    }

    /// Inserts an available directory, creating missing parent directories below the root.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when the path escapes the configured root.
    pub fn insert_directory(&self, path: &Path) -> Result<(), WorkspaceError> {
        self.insert_entry(path, WorkspaceNodeKind::Directory)
    }

    /// Inserts an available regular file, creating missing parent directories below the root.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when the path escapes the configured root.
    pub fn insert_file(&self, path: &Path) -> Result<(), WorkspaceError> {
        self.insert_entry(path, WorkspaceNodeKind::File)
    }

    /// Inserts a symbolic-link leaf, creating missing parent directories below the root.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when the path escapes the configured root.
    pub fn insert_symlink(&self, path: &Path) -> Result<(), WorkspaceError> {
        self.insert_entry(path, WorkspaceNodeKind::Symlink)
    }

    /// Assigns an availability state to an existing entry.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when the path is outside the root or absent.
    pub fn set_status(
        &self,
        path: &Path,
        status: WorkspaceNodeStatus,
    ) -> Result<(), WorkspaceError> {
        let path = self.resolve(path)?;
        let mut entries = self.entries.borrow_mut();
        let Some(entry) = entries.get_mut(&path) else {
            return Err(WorkspaceError::not_found("set workspace status", &path));
        };
        entry.status = status;
        Ok(())
    }

    /// Removes one entry and all descendants, returning whether anything was removed.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when the path escapes the root or names the root itself.
    pub fn remove(&self, path: &Path) -> Result<bool, WorkspaceError> {
        let path = self.resolve(path)?;
        if path == self.root {
            return Err(WorkspaceError::invalid_path(
                &path,
                "the memory workspace root cannot be removed",
            ));
        }
        let mut entries = self.entries.borrow_mut();
        let before = entries.len();
        entries.retain(|candidate, _| !candidate.starts_with(&path));
        Ok(entries.len() != before)
    }

    fn insert_entry(&self, path: &Path, kind: WorkspaceNodeKind) -> Result<(), WorkspaceError> {
        let path = self.resolve(path)?;
        if path == self.root && kind != WorkspaceNodeKind::Directory {
            return Err(WorkspaceError::invalid_path(
                &path,
                "the memory workspace root must remain a directory",
            ));
        }
        let mut entries = self.entries.borrow_mut();
        let mut parent = path.parent();
        while let Some(candidate) = parent {
            if !candidate.starts_with(&self.root) {
                break;
            }
            if entries
                .get(candidate)
                .is_some_and(|entry| entry.kind != WorkspaceNodeKind::Directory)
            {
                return Err(WorkspaceError::invalid_path(
                    &path,
                    format!(
                        "workspace ancestor is not a directory: {}",
                        candidate.display()
                    ),
                ));
            }
            let _ = entries
                .entry(candidate.to_path_buf())
                .or_insert(MemoryWorkspaceEntry {
                    kind: WorkspaceNodeKind::Directory,
                    status: WorkspaceNodeStatus::Available,
                });
            if candidate == self.root {
                break;
            }
            parent = candidate.parent();
        }
        if kind != WorkspaceNodeKind::Directory {
            entries.retain(|candidate, _| candidate == &path || !candidate.starts_with(&path));
        }
        let _ = entries.insert(
            path,
            MemoryWorkspaceEntry {
                kind,
                status: WorkspaceNodeStatus::Available,
            },
        );
        Ok(())
    }

    fn resolve(&self, path: &Path) -> Result<PathBuf, WorkspaceError> {
        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        validate_absolute_path(&candidate)?;
        if !candidate.starts_with(&self.root) {
            return Err(WorkspaceError::invalid_path(
                &candidate,
                "memory workspace paths must stay below the configured root",
            ));
        }
        Ok(candidate)
    }
}

impl WorkspaceService for MemoryWorkspaceService {
    fn scan(
        &self,
        root: &Path,
        options: WorkspaceScanOptions,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let root = self.resolve(root)?;
        let entries = self.entries.borrow();
        let Some(root_entry) = entries.get(&root) else {
            return Err(WorkspaceError::not_found("scan memory workspace", &root));
        };
        if root_entry.kind != WorkspaceNodeKind::Directory {
            return Err(WorkspaceError::not_directory(&root));
        }
        build_memory_snapshot(&root, &entries, options)
    }
}

impl WorkspaceMutationService for MemoryWorkspaceService {
    fn create_file(
        &self,
        parent: &Path,
        name: &str,
        collision: WorkspaceCollisionPolicy,
    ) -> Result<WorkspaceMutationOutcome, WorkspaceError> {
        let parent = self.resolve(parent)?;
        validate_entry_name(name, &parent)?;
        let path = parent.join(name);
        let mut entries = self.entries.borrow_mut();
        let Some(parent_entry) = entries.get(&parent) else {
            return Err(WorkspaceError::not_found("create memory file", &parent));
        };
        if parent_entry.kind != WorkspaceNodeKind::Directory {
            return Err(WorkspaceError::not_directory(&parent));
        }
        if let Some(existing) = entries.get(&path)
            && (collision == WorkspaceCollisionPolicy::FailIfExists
                || existing.kind != WorkspaceNodeKind::File)
        {
            return Err(WorkspaceError::already_exists(
                "create memory file",
                &path,
                "destination already exists",
            ));
        }
        let _ = entries.insert(
            path.clone(),
            MemoryWorkspaceEntry {
                kind: WorkspaceNodeKind::File,
                status: WorkspaceNodeStatus::Available,
            },
        );
        Ok(WorkspaceMutationOutcome {
            path,
            kind: WorkspaceNodeKind::File,
        })
    }

    fn create_directory(
        &self,
        parent: &Path,
        name: &str,
    ) -> Result<WorkspaceMutationOutcome, WorkspaceError> {
        let parent = self.resolve(parent)?;
        validate_entry_name(name, &parent)?;
        let path = parent.join(name);
        let mut entries = self.entries.borrow_mut();
        let Some(parent_entry) = entries.get(&parent) else {
            return Err(WorkspaceError::not_found(
                "create memory directory",
                &parent,
            ));
        };
        if parent_entry.kind != WorkspaceNodeKind::Directory {
            return Err(WorkspaceError::not_directory(&parent));
        }
        if entries.contains_key(&path) {
            return Err(WorkspaceError::already_exists(
                "create memory directory",
                &path,
                "destination already exists",
            ));
        }
        let _ = entries.insert(
            path.clone(),
            MemoryWorkspaceEntry {
                kind: WorkspaceNodeKind::Directory,
                status: WorkspaceNodeStatus::Available,
            },
        );
        Ok(WorkspaceMutationOutcome {
            path,
            kind: WorkspaceNodeKind::Directory,
        })
    }

    fn rename(
        &self,
        path: &Path,
        new_name: &str,
        collision: WorkspaceCollisionPolicy,
    ) -> Result<WorkspaceMutationOutcome, WorkspaceError> {
        let path = self.resolve(path)?;
        if path == self.root {
            return Err(WorkspaceError::invalid_path(
                &path,
                "the memory workspace root cannot be renamed",
            ));
        }
        let parent = path
            .parent()
            .ok_or_else(|| WorkspaceError::invalid_path(&path, "rename source has no parent"))?;
        validate_entry_name(new_name, parent)?;
        let destination = parent.join(new_name);
        let mut entries = self.entries.borrow_mut();
        let source = entries
            .get(&path)
            .cloned()
            .ok_or_else(|| WorkspaceError::not_found("rename memory entry", &path))?;
        if destination == path {
            return Ok(WorkspaceMutationOutcome {
                path,
                kind: source.kind,
            });
        }
        if let Some(existing) = entries.get(&destination) {
            if collision == WorkspaceCollisionPolicy::FailIfExists
                || source.kind != WorkspaceNodeKind::File
                || existing.kind != WorkspaceNodeKind::File
            {
                return Err(WorkspaceError::already_exists(
                    "rename memory entry",
                    &destination,
                    "destination already exists",
                ));
            }
            let _ = entries.remove(&destination);
        }
        let moved = entries
            .iter()
            .filter(|(candidate, _)| candidate.starts_with(&path))
            .map(|(candidate, entry)| {
                let relative = candidate
                    .strip_prefix(&path)
                    .unwrap_or_else(|_| Path::new(""));
                (destination.join(relative), entry.clone())
            })
            .collect::<Vec<_>>();
        entries.retain(|candidate, _| !candidate.starts_with(&path));
        for (candidate, entry) in moved {
            let _ = entries.insert(candidate, entry);
        }
        Ok(WorkspaceMutationOutcome {
            path: destination,
            kind: source.kind,
        })
    }

    fn delete(&self, path: &Path) -> Result<(), WorkspaceError> {
        let path = self.resolve(path)?;
        if path == self.root {
            return Err(WorkspaceError::invalid_path(
                &path,
                "the memory workspace root cannot be deleted",
            ));
        }
        let mut entries = self.entries.borrow_mut();
        if !entries.contains_key(&path) {
            return Err(WorkspaceError::not_found("delete memory entry", &path));
        }
        entries.retain(|candidate, _| !candidate.starts_with(&path));
        Ok(())
    }
}

/// Error category for workspace scanning and model construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceErrorKind {
    /// Path was relative, non-normalized, or outside an adapter root.
    InvalidPath,
    /// Requested path does not exist.
    NotFound,
    /// Requested root is not a directory.
    NotDirectory,
    /// A create or rename destination already exists.
    AlreadyExists,
    /// A leaf name was empty, reserved, or contained a path separator.
    InvalidName,
    /// Permission denied.
    PermissionDenied,
    /// Snapshot invariants were violated.
    InvalidSnapshot,
    /// Other I/O failure.
    Io,
}

/// Typed workspace error with operation and path context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceError {
    operation: &'static str,
    path: PathBuf,
    kind: WorkspaceErrorKind,
    message: String,
}

impl WorkspaceError {
    fn invalid_path(path: &Path, message: impl Into<String>) -> Self {
        Self {
            operation: "validate workspace path",
            path: path.to_path_buf(),
            kind: WorkspaceErrorKind::InvalidPath,
            message: message.into(),
        }
    }

    fn invalid_snapshot(path: &Path, message: impl Into<String>) -> Self {
        Self {
            operation: "build workspace snapshot",
            path: path.to_path_buf(),
            kind: WorkspaceErrorKind::InvalidSnapshot,
            message: message.into(),
        }
    }

    fn not_found(operation: &'static str, path: &Path) -> Self {
        Self {
            operation,
            path: path.to_path_buf(),
            kind: WorkspaceErrorKind::NotFound,
            message: "workspace path does not exist".to_owned(),
        }
    }

    fn not_directory(path: &Path) -> Self {
        Self {
            operation: "open workspace",
            path: path.to_path_buf(),
            kind: WorkspaceErrorKind::NotDirectory,
            message: "workspace root is not a directory".to_owned(),
        }
    }

    fn already_exists(operation: &'static str, path: &Path, message: impl Into<String>) -> Self {
        Self {
            operation,
            path: path.to_path_buf(),
            kind: WorkspaceErrorKind::AlreadyExists,
            message: message.into(),
        }
    }

    fn invalid_name(path: &Path, message: impl Into<String>) -> Self {
        Self {
            operation: "validate workspace entry name",
            path: path.to_path_buf(),
            kind: WorkspaceErrorKind::InvalidName,
            message: message.into(),
        }
    }

    fn io(operation: &'static str, path: &Path, error: std::io::Error) -> Self {
        let kind = match error.kind() {
            std::io::ErrorKind::NotFound => WorkspaceErrorKind::NotFound,
            std::io::ErrorKind::AlreadyExists => WorkspaceErrorKind::AlreadyExists,
            std::io::ErrorKind::PermissionDenied => WorkspaceErrorKind::PermissionDenied,
            _ => WorkspaceErrorKind::Io,
        };
        Self {
            operation,
            path: path.to_path_buf(),
            kind,
            message: error.to_string(),
        }
    }

    /// Returns whether the failure means the requested path no longer exists.
    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        matches!(self.kind, WorkspaceErrorKind::NotFound)
    }

    /// Returns the operation that failed.
    #[must_use]
    pub const fn operation(&self) -> &'static str {
        self.operation
    }

    /// Returns the related path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the typed error category.
    #[must_use]
    pub const fn kind(&self) -> WorkspaceErrorKind {
        self.kind
    }
}

impl Display for WorkspaceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} {}: {}",
            self.operation,
            self.path.display(),
            self.message
        )
    }
}

impl Error for WorkspaceError {}

fn build_std_snapshot(
    root: PathBuf,
    options: WorkspaceScanOptions,
) -> Result<WorkspaceSnapshot, WorkspaceError> {
    let root_title = path_title(&root);
    let root_node = WorkspaceNode::new(
        root.clone(),
        PathBuf::new(),
        root_title,
        WorkspaceNodeKind::Directory,
        WorkspaceNodeStatus::Available,
    )?;
    let root_id = root_node.id.clone();
    let mut nodes = BTreeMap::new();
    let _ = nodes.insert(root_id.clone(), root_node);
    scan_std_directory(&root, &root, &root_id, 0, options, &mut nodes)?;
    sort_all_children(&mut nodes);
    WorkspaceSnapshot::from_nodes(root, root_id, nodes)
}

fn scan_std_directory(
    root: &Path,
    directory: &Path,
    directory_id: &WorkspaceNodeId,
    depth: u16,
    options: WorkspaceScanOptions,
    nodes: &mut BTreeMap<WorkspaceNodeId, WorkspaceNode>,
) -> Result<(), WorkspaceError> {
    if depth >= options.maximum_depth {
        if let Some(node) = nodes.get_mut(directory_id) {
            node.status = WorkspaceNodeStatus::DepthLimit;
        }
        return Ok(());
    }
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            if directory == root {
                return Err(WorkspaceError::io(
                    "read workspace directory",
                    directory,
                    error,
                ));
            }
            if let Some(node) = nodes.get_mut(directory_id) {
                node.status = status_from_io_error(&error);
            }
            return Ok(());
        }
    };
    let mut children = Vec::new();
    for result in entries {
        let entry = match result {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        if should_hide(&path, options.hidden_files) {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_or_else(|_| path.clone(), Path::to_path_buf);
        let title = path_title(&path);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                let node = WorkspaceNode::new(
                    path,
                    relative,
                    title,
                    WorkspaceNodeKind::File,
                    status_from_io_error(&error),
                )?;
                children.push(node.id.clone());
                let _ = nodes.insert(node.id.clone(), node);
                continue;
            }
        };
        let file_type = metadata.file_type();
        let kind = if file_type.is_symlink() {
            if options.symlinks == SymlinkPolicy::Exclude {
                continue;
            }
            WorkspaceNodeKind::Symlink
        } else if file_type.is_dir() {
            WorkspaceNodeKind::Directory
        } else {
            WorkspaceNodeKind::File
        };
        let node = WorkspaceNode::new(
            path.clone(),
            relative,
            title,
            kind,
            WorkspaceNodeStatus::Available,
        )?;
        let node_id = node.id.clone();
        children.push(node_id.clone());
        let _ = nodes.insert(node_id.clone(), node);
        if kind == WorkspaceNodeKind::Directory {
            scan_std_directory(
                root,
                &path,
                &node_id,
                depth.saturating_add(1),
                options,
                nodes,
            )?;
        }
    }
    if let Some(node) = nodes.get_mut(directory_id) {
        node.children = children;
    }
    Ok(())
}

fn build_memory_snapshot(
    root: &Path,
    entries: &BTreeMap<PathBuf, MemoryWorkspaceEntry>,
    options: WorkspaceScanOptions,
) -> Result<WorkspaceSnapshot, WorkspaceError> {
    let mut nodes = BTreeMap::new();
    for (path, entry) in entries {
        if !path.starts_with(root)
            || (path != root && contains_hidden_component(path, root, options.hidden_files))
        {
            continue;
        }
        if entry.kind == WorkspaceNodeKind::Symlink && options.symlinks == SymlinkPolicy::Exclude {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_or_else(|_| path.clone(), Path::to_path_buf);
        let depth = component_count(&relative);
        if depth > usize::from(options.maximum_depth) {
            continue;
        }
        let status = if depth == usize::from(options.maximum_depth)
            && entry.kind == WorkspaceNodeKind::Directory
        {
            WorkspaceNodeStatus::DepthLimit
        } else {
            entry.status.clone()
        };
        let node =
            WorkspaceNode::new(path.clone(), relative, path_title(path), entry.kind, status)?;
        let _ = nodes.insert(node.id.clone(), node);
    }
    let root_id = WorkspaceNodeId::from_absolute_path(root)?;
    if !nodes.contains_key(&root_id) {
        return Err(WorkspaceError::not_found("scan memory workspace", root));
    }
    let ids: Vec<_> = nodes.keys().cloned().collect();
    for id in ids {
        let Some(parent_path) = nodes.get(&id).map(|node| node.path.clone()) else {
            continue;
        };
        let child_ids: Vec<_> = nodes
            .values()
            .filter(|candidate| candidate.path.parent() == Some(parent_path.as_path()))
            .map(|candidate| candidate.id.clone())
            .collect();
        if let Some(node) = nodes.get_mut(&id) {
            node.children = child_ids;
        }
    }
    sort_all_children(&mut nodes);
    WorkspaceSnapshot::from_nodes(root.to_path_buf(), root_id, nodes)
}

fn rebuild_children(nodes: &mut BTreeMap<WorkspaceNodeId, WorkspaceNode>) {
    for node in nodes.values_mut() {
        node.children.clear();
    }
    let parent_children = nodes
        .values()
        .filter_map(|node| {
            let parent_path = node.path.parent()?.to_path_buf();
            let parent_id = nodes
                .values()
                .find(|candidate| candidate.path == parent_path)?
                .id
                .clone();
            Some((parent_id, node.id.clone()))
        })
        .collect::<Vec<_>>();
    for (parent, child) in parent_children {
        if let Some(node) = nodes.get_mut(&parent) {
            node.children.push(child);
        }
    }
    sort_all_children(nodes);
}

fn sort_all_children(nodes: &mut BTreeMap<WorkspaceNodeId, WorkspaceNode>) {
    let metadata: BTreeMap<_, _> = nodes
        .iter()
        .map(|(id, node)| {
            (
                id.clone(),
                (
                    node.kind.sort_rank(),
                    node.title.to_lowercase(),
                    node.title.clone(),
                ),
            )
        })
        .collect();
    for node in nodes.values_mut() {
        node.children.sort_by(|left, right| {
            metadata
                .get(left)
                .cmp(&metadata.get(right))
                .then_with(|| left.cmp(right))
        });
    }
}

fn status_from_io_error(error: &std::io::Error) -> WorkspaceNodeStatus {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        WorkspaceNodeStatus::PermissionDenied
    } else {
        WorkspaceNodeStatus::Unreadable(error.to_string())
    }
}

fn should_hide(path: &Path, policy: HiddenFilePolicy) -> bool {
    policy == HiddenFilePolicy::Exclude
        && path
            .file_name()
            .is_some_and(|name| path_bytes(name).first() == Some(&b'.'))
}

fn contains_hidden_component(path: &Path, root: &Path, policy: HiddenFilePolicy) -> bool {
    if policy == HiddenFilePolicy::Include {
        return false;
    }
    path.strip_prefix(root).is_ok_and(|relative| {
        relative.components().any(|component| match component {
            Component::Normal(name) => path_bytes(name).first() == Some(&b'.'),
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => false,
        })
    })
}

fn path_title(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn canonical_directory(path: &Path) -> Result<PathBuf, WorkspaceError> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| WorkspaceError::io("canonicalize workspace directory", path, error))?;
    validate_absolute_path(&canonical)?;
    let metadata = fs::metadata(&canonical)
        .map_err(|error| WorkspaceError::io("read workspace directory", &canonical, error))?;
    if !metadata.is_dir() {
        return Err(WorkspaceError::not_directory(&canonical));
    }
    Ok(canonical)
}

fn validate_entry_name(name: &str, parent: &Path) -> Result<(), WorkspaceError> {
    if name.is_empty() || name == "." || name == ".." {
        return Err(WorkspaceError::invalid_name(
            parent,
            "workspace entry name cannot be empty, '.' or '..'",
        ));
    }
    if Path::new(name).components().count() != 1
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
    {
        return Err(WorkspaceError::invalid_name(
            parent,
            "workspace entry name must be one leaf name without separators",
        ));
    }
    Ok(())
}

fn validate_absolute_path(path: &Path) -> Result<(), WorkspaceError> {
    if !path.is_absolute() {
        return Err(WorkspaceError::invalid_path(
            path,
            "workspace paths must be absolute",
        ));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(WorkspaceError::invalid_path(
            path,
            "workspace paths must not contain . or .. components",
        ));
    }
    Ok(())
}

fn component_count(path: &Path) -> usize {
    path.components().count()
}

fn snapshot_fingerprint(nodes: &BTreeMap<WorkspaceNodeId, WorkspaceNode>) -> u64 {
    let mut state = 0xcbf29ce484222325_u64;
    for node in nodes.values() {
        fingerprint_bytes(&mut state, node.id.stable_key().as_bytes());
        fingerprint_bytes(&mut state, &[node.kind.sort_rank()]);
        fingerprint_bytes(&mut state, node.title.as_bytes());
        fingerprint_bytes(&mut state, status_bytes(&node.status));
        if let WorkspaceNodeStatus::Unreadable(message) = &node.status {
            fingerprint_bytes(&mut state, message.as_bytes());
        }
        for child in &node.children {
            fingerprint_bytes(&mut state, child.stable_key().as_bytes());
        }
    }
    state
}

fn status_bytes(status: &WorkspaceNodeStatus) -> &[u8] {
    match status {
        WorkspaceNodeStatus::Available => b"available",
        WorkspaceNodeStatus::PermissionDenied => b"permission-denied",
        WorkspaceNodeStatus::DepthLimit => b"depth-limit",
        WorkspaceNodeStatus::Unreadable(_) => b"unreadable",
    }
}

fn fingerprint_bytes(state: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *state ^= u64::from(*byte);
        *state = state.wrapping_mul(0x100000001b3);
    }
}

#[cfg(unix)]
fn path_bytes(value: &OsStr) -> Vec<u8> {
    value.as_bytes().to_vec()
}

#[cfg(windows)]
fn path_bytes(value: &OsStr) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;

    value.encode_wide().flat_map(u16::to_le_bytes).collect()
}

#[cfg(not(any(unix, windows)))]
fn path_bytes(value: &OsStr) -> Vec<u8> {
    value.to_string_lossy().as_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::{
        HiddenFilePolicy, MemoryWorkspaceService, MemoryWorkspaceWatchService,
        PollingWorkspaceWatchService, StdWorkspaceService, SymlinkPolicy, WorkspaceCollisionPolicy,
        WorkspaceModel, WorkspaceMutationService, WorkspaceNodeId, WorkspaceNodeKind,
        WorkspaceNodeStatus, WorkspaceRefreshScope, WorkspaceScanOptions, WorkspaceService,
        WorkspaceWatchEvent, WorkspaceWatchKind, WorkspaceWatchService, coalesce_watch_events,
        refresh_scope_for_events,
    };
    use std::error::Error;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    type TestResult = Result<(), Box<dyn Error>>;

    fn memory() -> Result<MemoryWorkspaceService, Box<dyn Error>> {
        Ok(MemoryWorkspaceService::new(PathBuf::from("/workspace"))?)
    }

    fn temp_root(label: &str) -> Result<PathBuf, Box<dyn Error>> {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "luna-workspaces-{label}-{}-{sequence}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::create_dir_all(&root)?;
        Ok(root)
    }

    #[test]
    fn stable_identity_uses_exact_absolute_path() -> TestResult {
        let first = WorkspaceNodeId::from_absolute_path(Path::new("/workspace/src/main.rs"))?;
        let second = WorkspaceNodeId::from_absolute_path(Path::new("/workspace/src/main.rs"))?;
        let other = WorkspaceNodeId::from_absolute_path(Path::new("/workspace/src/lib.rs"))?;
        assert_eq!(first, second);
        assert_ne!(first, other);
        assert!(first.stable_key().starts_with("workspace-node-"));
        Ok(())
    }

    #[test]
    fn relative_identity_is_rejected() {
        assert!(WorkspaceNodeId::from_absolute_path(Path::new("src/main.rs")).is_err());
    }

    #[test]
    fn memory_scan_sorts_directories_before_files() -> TestResult {
        let service = memory()?;
        service.insert_file(Path::new("zeta.txt"))?;
        service.insert_directory(Path::new("Alpha"))?;
        service.insert_file(Path::new("beta.txt"))?;
        let snapshot = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        let root = snapshot
            .node(snapshot.root_id())
            .ok_or_else(|| std::io::Error::other("root missing"))?;
        let titles: Vec<_> = root
            .children()
            .iter()
            .filter_map(|id| snapshot.node(id))
            .map(|node| node.title())
            .collect();
        assert_eq!(titles, vec!["Alpha", "beta.txt", "zeta.txt"]);
        Ok(())
    }

    #[test]
    fn memory_service_rejects_children_below_non_directories() -> TestResult {
        let service = memory()?;
        service.insert_file(Path::new("plain.txt"))?;
        assert!(
            service
                .insert_file(Path::new("plain.txt/child.txt"))
                .is_err()
        );
        service.insert_symlink(Path::new("linked"))?;
        assert!(service.insert_file(Path::new("linked/child.txt")).is_err());
        Ok(())
    }

    #[test]
    fn replacing_directory_with_file_removes_descendants() -> TestResult {
        let service = memory()?;
        service.insert_file(Path::new("folder/child.txt"))?;
        service.insert_file(Path::new("folder"))?;
        let snapshot = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        assert_eq!(
            snapshot
                .node_for_path(Path::new("/workspace/folder"))
                .map(|node| node.kind()),
            Some(WorkspaceNodeKind::File)
        );
        assert!(
            snapshot
                .node_for_path(Path::new("/workspace/folder/child.txt"))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn hidden_policy_excludes_dot_entries_by_default() -> TestResult {
        let service = memory()?;
        service.insert_file(Path::new(".secret"))?;
        service.insert_file(Path::new("visible"))?;
        let default_snapshot =
            service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        assert!(
            default_snapshot
                .node_for_path(Path::new("/workspace/.secret"))
                .is_none()
        );
        let visible = service.scan(
            Path::new("/workspace"),
            WorkspaceScanOptions {
                hidden_files: HiddenFilePolicy::Include,
                ..WorkspaceScanOptions::default()
            },
        )?;
        assert!(
            visible
                .node_for_path(Path::new("/workspace/.secret"))
                .is_some()
        );
        Ok(())
    }

    #[test]
    fn hidden_directory_descendants_are_excluded_together() -> TestResult {
        let service = memory()?;
        service.insert_file(Path::new(".cache/nested/state.txt"))?;
        let snapshot = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        assert!(
            snapshot
                .node_for_path(Path::new("/workspace/.cache"))
                .is_none()
        );
        assert!(
            snapshot
                .node_for_path(Path::new("/workspace/.cache/nested/state.txt"))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn symlink_policy_can_exclude_or_show_leaf() -> TestResult {
        let service = memory()?;
        service.insert_symlink(Path::new("linked"))?;
        let shown = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        assert_eq!(
            shown
                .node_for_path(Path::new("/workspace/linked"))
                .map(|node| node.kind()),
            Some(WorkspaceNodeKind::Symlink)
        );
        let excluded = service.scan(
            Path::new("/workspace"),
            WorkspaceScanOptions {
                symlinks: SymlinkPolicy::Exclude,
                ..WorkspaceScanOptions::default()
            },
        )?;
        assert!(
            excluded
                .node_for_path(Path::new("/workspace/linked"))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn visible_rows_follow_expansion_state() -> TestResult {
        let service = memory()?;
        service.insert_file(Path::new("src/main.rs"))?;
        let snapshot = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        let mut model = WorkspaceModel::new(snapshot);
        assert_eq!(model.visible_rows().len(), 2);
        let source_id = model
            .snapshot()
            .node_for_path(Path::new("/workspace/src"))
            .ok_or_else(|| std::io::Error::other("source folder missing"))?
            .id()
            .clone();
        assert!(model.toggle_expanded(&source_id));
        assert_eq!(model.visible_rows().len(), 3);
        Ok(())
    }

    #[test]
    fn reveal_expands_ancestors_and_selects_file() -> TestResult {
        let service = memory()?;
        service.insert_file(Path::new("src/nested/main.rs"))?;
        let snapshot = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        let mut model = WorkspaceModel::new(snapshot);
        let file_id = model
            .snapshot()
            .node_for_path(Path::new("/workspace/src/nested/main.rs"))
            .ok_or_else(|| std::io::Error::other("file missing"))?
            .id()
            .clone();
        assert!(model.reveal(&file_id));
        assert_eq!(model.selected(), Some(&file_id));
        let visible_paths: Vec<_> = model
            .visible_rows()
            .into_iter()
            .map(|row| row.path)
            .collect();
        assert!(visible_paths.contains(&PathBuf::from("/workspace/src/nested/main.rs")));
        Ok(())
    }

    #[test]
    fn refresh_preserves_surviving_expansion_and_selection() -> TestResult {
        let service = memory()?;
        service.insert_file(Path::new("src/main.rs"))?;
        let snapshot = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        let mut model = WorkspaceModel::new(snapshot);
        let source_id = model
            .snapshot()
            .node_for_path(Path::new("/workspace/src"))
            .ok_or_else(|| std::io::Error::other("source folder missing"))?
            .id()
            .clone();
        let file_id = model
            .snapshot()
            .node_for_path(Path::new("/workspace/src/main.rs"))
            .ok_or_else(|| std::io::Error::other("file missing"))?
            .id()
            .clone();
        assert!(model.toggle_expanded(&source_id));
        assert!(model.select(Some(file_id.clone())));
        service.insert_file(Path::new("src/lib.rs"))?;
        let refreshed = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        assert!(model.refresh(refreshed));
        assert!(model.is_expanded(&source_id));
        assert_eq!(model.selected(), Some(&file_id));
        Ok(())
    }

    #[test]
    fn refresh_clears_removed_selection() -> TestResult {
        let service = memory()?;
        service.insert_file(Path::new("main.rs"))?;
        let snapshot = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        let mut model = WorkspaceModel::new(snapshot);
        let file_id = model
            .snapshot()
            .node_for_path(Path::new("/workspace/main.rs"))
            .ok_or_else(|| std::io::Error::other("file missing"))?
            .id()
            .clone();
        assert!(model.select(Some(file_id)));
        assert!(service.remove(Path::new("main.rs"))?);
        let refreshed = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        assert!(model.refresh(refreshed));
        assert!(model.selected().is_none());
        Ok(())
    }

    #[test]
    fn unchanged_refresh_is_suppressed() -> TestResult {
        let service = memory()?;
        let snapshot = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        let mut model = WorkspaceModel::new(snapshot.clone());
        assert!(!model.refresh(snapshot));
        assert_eq!(model.generation(), 1);
        Ok(())
    }

    #[test]
    fn unreadable_directory_is_projected_as_leaf() -> TestResult {
        let service = memory()?;
        service.insert_file(Path::new("locked/inside.txt"))?;
        service.set_status(Path::new("locked"), WorkspaceNodeStatus::PermissionDenied)?;
        let snapshot = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        let locked = snapshot
            .node_for_path(Path::new("/workspace/locked"))
            .ok_or_else(|| std::io::Error::other("locked folder missing"))?;
        assert_eq!(locked.status(), &WorkspaceNodeStatus::PermissionDenied);
        let locked_id = locked.id().clone();
        let mut model = WorkspaceModel::new(snapshot);
        assert!(!model.toggle_expanded(&locked_id));
        Ok(())
    }

    #[test]
    fn maximum_depth_marks_truncated_directory() -> TestResult {
        let service = memory()?;
        service.insert_file(Path::new("one/two/three.txt"))?;
        let snapshot = service.scan(
            Path::new("/workspace"),
            WorkspaceScanOptions {
                maximum_depth: 1,
                ..WorkspaceScanOptions::default()
            },
        )?;
        let one = snapshot
            .node_for_path(Path::new("/workspace/one"))
            .ok_or_else(|| std::io::Error::other("one missing"))?;
        assert_eq!(one.status(), &WorkspaceNodeStatus::DepthLimit);
        assert!(
            snapshot
                .node_for_path(Path::new("/workspace/one/two"))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn standard_scanner_reads_real_directory_tree() -> TestResult {
        let root = temp_root("scan")?;
        fs::create_dir_all(root.join("src"))?;
        fs::write(root.join("src/main.rs"), "fn main() {}\n")?;
        fs::write(root.join("README.md"), "# Demo\n")?;
        let snapshot = StdWorkspaceService.scan(&root, WorkspaceScanOptions::default())?;
        assert!(snapshot.node_for_path(&root.join("src/main.rs")).is_some());
        assert!(snapshot.node_for_path(&root.join("README.md")).is_some());
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn standard_scanner_does_not_follow_symlink_directories() -> TestResult {
        use std::os::unix::fs::symlink;

        let root = temp_root("symlink")?;
        let outside = temp_root("outside")?;
        fs::write(outside.join("secret.txt"), "outside\n")?;
        symlink(&outside, root.join("linked"))?;
        let snapshot = StdWorkspaceService.scan(&root, WorkspaceScanOptions::default())?;
        let linked = snapshot
            .node_for_path(&root.join("linked"))
            .ok_or_else(|| std::io::Error::other("link missing"))?;
        assert_eq!(linked.kind(), WorkspaceNodeKind::Symlink);
        assert!(linked.children().is_empty());
        fs::remove_dir_all(root)?;
        fs::remove_dir_all(outside)?;
        Ok(())
    }

    #[test]
    fn mutation_names_reject_reserved_and_multi_component_values() -> TestResult {
        let service = memory()?;
        for name in ["", ".", "..", "nested/name", "nested\\name", "bad\0name"] {
            let error = service
                .create_file(
                    Path::new("/workspace"),
                    name,
                    WorkspaceCollisionPolicy::FailIfExists,
                )
                .err()
                .ok_or_else(|| std::io::Error::other("invalid name was accepted"))?;
            assert_eq!(error.kind(), super::WorkspaceErrorKind::InvalidName);
        }
        Ok(())
    }

    #[test]
    fn memory_mutations_create_rename_replace_and_delete_subtrees() -> TestResult {
        let service = memory()?;
        let created = service.create_directory(Path::new("/workspace"), "src")?;
        assert_eq!(created.path(), Path::new("/workspace/src"));
        let file = service.create_file(
            Path::new("/workspace/src"),
            "main.rs",
            WorkspaceCollisionPolicy::FailIfExists,
        )?;
        assert_eq!(file.kind(), WorkspaceNodeKind::File);
        assert!(
            service
                .create_file(
                    Path::new("/workspace/src"),
                    "main.rs",
                    WorkspaceCollisionPolicy::FailIfExists,
                )
                .is_err()
        );
        let renamed = service.rename(
            Path::new("/workspace/src"),
            "source",
            WorkspaceCollisionPolicy::FailIfExists,
        )?;
        assert_eq!(renamed.path(), Path::new("/workspace/source"));
        let snapshot = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        assert!(
            snapshot
                .node_for_path(Path::new("/workspace/source/main.rs"))
                .is_some()
        );
        service.delete(Path::new("/workspace/source"))?;
        let snapshot = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        assert!(
            snapshot
                .node_for_path(Path::new("/workspace/source"))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn same_name_rename_is_a_non_destructive_no_op() -> TestResult {
        let service = memory()?;
        service.insert_file(Path::new("same.txt"))?;
        let outcome = service.rename(
            Path::new("/workspace/same.txt"),
            "same.txt",
            WorkspaceCollisionPolicy::ReplaceFile,
        )?;
        assert_eq!(outcome.path(), Path::new("/workspace/same.txt"));
        assert!(
            service
                .scan(Path::new("/workspace"), WorkspaceScanOptions::default())?
                .node_for_path(Path::new("/workspace/same.txt"))
                .is_some()
        );
        Ok(())
    }

    #[test]
    fn model_restores_persisted_expansion_and_selection_paths() -> TestResult {
        let service = memory()?;
        service.insert_file(Path::new("src/nested/main.rs"))?;
        let snapshot = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        let mut model = WorkspaceModel::new(snapshot);
        assert!(model.restore_tree_state(
            &[
                PathBuf::from("/workspace/src"),
                PathBuf::from("/workspace/src/nested"),
            ],
            Some(Path::new("/workspace/src/nested/main.rs")),
        ));
        assert_eq!(model.expanded_paths().len(), 3);
        assert_eq!(
            model
                .selected()
                .and_then(|id| model.snapshot().node(id))
                .map(|node| node.path()),
            Some(Path::new("/workspace/src/nested/main.rs")),
        );
        Ok(())
    }

    #[test]
    fn standard_mutation_round_trip_uses_real_filesystem() -> TestResult {
        let root = temp_root("mutations")?;
        StdWorkspaceService.create_directory(&root, "src")?;
        StdWorkspaceService.create_file(
            &root.join("src"),
            "main.rs",
            WorkspaceCollisionPolicy::FailIfExists,
        )?;
        StdWorkspaceService.rename(
            &root.join("src/main.rs"),
            "lib.rs",
            WorkspaceCollisionPolicy::FailIfExists,
        )?;
        assert!(root.join("src/lib.rs").is_file());
        StdWorkspaceService.delete(&root.join("src"))?;
        assert!(!root.join("src").exists());
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn rename_never_replaces_a_broken_symlink_destination() -> TestResult {
        use std::os::unix::fs::symlink;

        let root = temp_root("broken-symlink-collision")?;
        fs::write(
            root.join("source.txt"),
            "source
",
        )?;
        symlink(root.join("missing-target"), root.join("destination.txt"))?;

        let error = StdWorkspaceService
            .rename(
                &root.join("source.txt"),
                "destination.txt",
                WorkspaceCollisionPolicy::ReplaceFile,
            )
            .err()
            .ok_or_else(|| std::io::Error::other("broken symlink was unexpectedly replaced"))?;

        assert_eq!(error.kind(), super::WorkspaceErrorKind::AlreadyExists);
        assert!(root.join("source.txt").is_file());
        assert!(
            fs::symlink_metadata(root.join("destination.txt"))?
                .file_type()
                .is_symlink()
        );
        fs::remove_file(root.join("destination.txt"))?;
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn watcher_bursts_coalesce_and_choose_the_smallest_safe_scope() -> TestResult {
        let root = Path::new("/workspace");
        let events = vec![
            WorkspaceWatchEvent {
                path: PathBuf::from("/workspace/src/main.rs"),
                kind: WorkspaceWatchKind::Modified,
            },
            WorkspaceWatchEvent {
                path: PathBuf::from("/workspace/src/main.rs"),
                kind: WorkspaceWatchKind::Renamed,
            },
            WorkspaceWatchEvent {
                path: PathBuf::from("/workspace/src/lib.rs"),
                kind: WorkspaceWatchKind::Created,
            },
        ];
        let coalesced = coalesce_watch_events(root, &events)?;
        assert_eq!(coalesced.len(), 2);
        assert_eq!(
            coalesced
                .iter()
                .find(|event| event.path.ends_with("main.rs"))
                .map(|event| event.kind),
            Some(WorkspaceWatchKind::Renamed)
        );
        assert_eq!(
            refresh_scope_for_events(root, &coalesced)?,
            Some(WorkspaceRefreshScope::Subtree(PathBuf::from(
                "/workspace/src"
            )))
        );
        let cross_tree = vec![
            WorkspaceWatchEvent {
                path: PathBuf::from("/workspace/src/main.rs"),
                kind: WorkspaceWatchKind::Modified,
            },
            WorkspaceWatchEvent {
                path: PathBuf::from("/workspace/docs/guide.md"),
                kind: WorkspaceWatchKind::Modified,
            },
        ];
        assert_eq!(
            refresh_scope_for_events(root, &cross_tree)?,
            Some(WorkspaceRefreshScope::Full)
        );
        Ok(())
    }

    #[test]
    fn incremental_refresh_preserves_unaffected_identity_expansion_and_selection() -> TestResult {
        let service = memory()?;
        service.insert_file(Path::new("src/main.rs"))?;
        service.insert_file(Path::new("docs/guide.md"))?;
        let snapshot = service.scan(Path::new("/workspace"), WorkspaceScanOptions::default())?;
        let mut model = WorkspaceModel::new(snapshot);
        let source_id = model
            .snapshot()
            .node_for_path(Path::new("/workspace/src"))
            .ok_or_else(|| std::io::Error::other("source directory missing"))?
            .id()
            .clone();
        let selected = model
            .snapshot()
            .node_for_path(Path::new("/workspace/src/main.rs"))
            .ok_or_else(|| std::io::Error::other("selected file missing"))?
            .id()
            .clone();
        let unaffected = model
            .snapshot()
            .node_for_path(Path::new("/workspace/docs/guide.md"))
            .ok_or_else(|| std::io::Error::other("unaffected file missing"))?
            .id()
            .clone();
        assert!(model.toggle_expanded(&source_id));
        assert!(model.select(Some(selected.clone())));

        service.insert_file(Path::new("src/lib.rs"))?;
        assert!(model.refresh_from_events(
            &service,
            &[WorkspaceWatchEvent {
                path: PathBuf::from("/workspace/src/lib.rs"),
                kind: WorkspaceWatchKind::Created,
            }],
            WorkspaceScanOptions::default(),
        )?);

        assert!(model.is_expanded(&source_id));
        assert_eq!(model.selected(), Some(&selected));
        assert_eq!(
            model
                .snapshot()
                .node_for_path(Path::new("/workspace/docs/guide.md"))
                .map(|node| node.id()),
            Some(&unaffected)
        );
        assert!(
            model
                .snapshot()
                .node_for_path(Path::new("/workspace/src/lib.rs"))
                .is_some()
        );
        Ok(())
    }

    #[test]
    fn polling_watcher_reports_real_create_and_remove_events() -> TestResult {
        let root = temp_root("poll-watcher")?;
        let mut watcher = PollingWorkspaceWatchService::default();
        watcher.watch(&root)?;
        let file = root.join("created.txt");
        fs::write(&file, "created")?;
        let created = watcher.drain_events()?;
        assert!(
            created
                .iter()
                .any(|event| { event.path == file && event.kind == WorkspaceWatchKind::Created })
        );
        fs::remove_file(&file)?;
        let removed = watcher.drain_events()?;
        assert!(
            removed
                .iter()
                .any(|event| { event.path == file && event.kind == WorkspaceWatchKind::Removed })
        );
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn watcher_events_are_drained_only_for_active_root() -> TestResult {
        let mut watcher = MemoryWorkspaceWatchService::default();
        watcher.watch(Path::new("/workspace"))?;
        watcher.push(WorkspaceWatchEvent {
            path: PathBuf::from("/workspace/src/main.rs"),
            kind: WorkspaceWatchKind::Modified,
        });
        assert_eq!(watcher.drain_events()?.len(), 1);
        assert!(watcher.drain_events()?.is_empty());
        watcher.push(WorkspaceWatchEvent {
            path: PathBuf::from("/outside/file.txt"),
            kind: WorkspaceWatchKind::Created,
        });
        assert!(watcher.drain_events().is_err());
        Ok(())
    }
}
