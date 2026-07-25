// SPDX-License-Identifier: MPL-2.0

//! Product-neutral workspace trees and filesystem scan adapters.
//!
//! This crate owns stable workspace-node identities, recursive immutable snapshots, expansion and
//! selection state, visible-row flattening, refresh preservation, and synchronous scan boundaries.
//! It intentionally does not own native dialogs, editor tabs, command policy, or file mutations.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

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

/// Error category for workspace scanning and model construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceErrorKind {
    /// Path was relative, non-normalized, or outside an adapter root.
    InvalidPath,
    /// Requested path does not exist.
    NotFound,
    /// Requested root is not a directory.
    NotDirectory,
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

    fn io(operation: &'static str, path: &Path, error: std::io::Error) -> Self {
        let kind = match error.kind() {
            std::io::ErrorKind::NotFound => WorkspaceErrorKind::NotFound,
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
        HiddenFilePolicy, MemoryWorkspaceService, StdWorkspaceService, SymlinkPolicy,
        WorkspaceModel, WorkspaceNodeId, WorkspaceNodeKind, WorkspaceNodeStatus,
        WorkspaceScanOptions, WorkspaceService,
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
}
