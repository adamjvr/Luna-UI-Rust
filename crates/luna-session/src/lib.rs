// SPDX-License-Identifier: MPL-2.0

//! Persistent product-neutral editor-session state.
//!
//! This crate stores recent-file paths and workspace restoration state without depending on UI,
//! document, or workspace-model crates. The standard adapter uses an atomic versioned text file;
//! the memory adapter provides deterministic tests and product harnesses.

use luna_core::{CodedError, ErrorCode};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
#[cfg(unix)]
use std::ffi::OsString;
use std::fmt::{Display, Formatter};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};

const FORMAT_HEADER: &str = "LUNA_EDITOR_SESSION_V2";
const LEGACY_FORMAT_HEADER: &str = "LUNA_EDITOR_SESSION_V1";

/// One persisted recent-file entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRecentFile {
    /// Canonical path recorded by the application.
    pub path: PathBuf,
    /// User-visible title.
    pub title: String,
}

/// Persisted workspace restoration data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionWorkspace {
    /// Canonical workspace root.
    pub root: PathBuf,
    /// Expanded directory paths.
    pub expanded_paths: Vec<PathBuf>,
    /// Selected workspace path, when available.
    pub selected_path: Option<PathBuf>,
}

/// Persisted storage baseline for one file-backed document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionStorageSnapshot {
    /// Adapter-provided content revision value.
    pub revision: u64,
    /// Adapter-provided concrete storage-instance value.
    pub instance: u128,
}

/// Persisted document source sufficient to reconstruct an editor session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionDocumentSource {
    /// File-backed document.
    File(PathBuf),
    /// Unsaved document with its monotonic title sequence.
    Untitled(u32),
    /// Application-owned virtual document.
    Virtual(String),
}

/// One persisted shared document buffer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionDocument {
    /// Application-defined document key.
    pub document_key: u64,
    /// Durable source description.
    pub source: SessionDocumentSource,
    /// User-visible title.
    pub title: String,
    /// Complete UTF-8 buffer text.
    pub text: String,
    /// Whether the buffer differed from its saved baseline when persisted.
    pub is_dirty: bool,
    /// Last known storage baseline for a file-backed document.
    pub storage_snapshot: Option<SessionStorageSnapshot>,
}

/// One persisted independent presentation of a shared document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionDocumentView {
    /// Application-defined view key used by the pane snapshot.
    pub view_key: u64,
    /// Application-defined shared document key.
    pub document_key: u64,
    /// Caret position as a UTF-8 byte offset in the shared document.
    pub caret_byte: usize,
    /// Selection anchor as a UTF-8 byte offset, when a selection is active.
    pub selection_anchor_byte: Option<usize>,
    /// Selection focus as a UTF-8 byte offset, when a selection is active.
    pub selection_focus_byte: Option<usize>,
    /// Pane-local horizontal scroll offset.
    pub scroll_x: i32,
    /// Pane-local vertical scroll offset.
    pub scroll_y: i32,
}

/// Persisted split direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionPaneAxis {
    /// Side-by-side children.
    Horizontal,
    /// Stacked children.
    Vertical,
}

/// One persisted tab entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionPaneTab {
    /// Application-defined view key.
    pub view_key: u64,
    /// Whether the tab belongs to the leading pinned partition.
    pub is_pinned: bool,
    /// Whether the tab is the pane-local replaceable preview.
    pub is_preview: bool,
}

/// Recursive persisted pane node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionPaneNode {
    /// Terminal editor pane.
    Leaf {
        /// Stable pane numeric key.
        pane_key: u64,
        /// Tabs in pane-local order.
        tabs: Vec<SessionPaneTab>,
        /// Active application-defined view key.
        active_view_key: u64,
        /// First regular tab shown by overflow projection.
        tab_scroll_offset: usize,
    },
    /// Recursive split.
    Split {
        /// Stable split numeric key.
        split_key: u64,
        /// Split direction.
        axis: SessionPaneAxis,
        /// First-child ratio in thousandths.
        ratio_milli: u16,
        /// First child.
        first: Box<SessionPaneNode>,
        /// Second child.
        second: Box<SessionPaneNode>,
    },
}

/// Complete persisted pane topology and focus state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionPaneTree {
    /// Focused leaf-pane numeric key.
    pub focused_pane_key: u64,
    /// Recursive root.
    pub root: SessionPaneNode,
}

/// Complete persisted editor-session state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionState {
    /// Recent files in most-recent-first order.
    pub recent_files: Vec<SessionRecentFile>,
    /// Last open workspace and tree state.
    pub workspace: Option<SessionWorkspace>,
    /// Shared document buffers needed by restored views.
    pub documents: Vec<SessionDocument>,
    /// Independent document presentations.
    pub views: Vec<SessionDocumentView>,
    /// Recursive pane topology and pane-local tab state.
    pub pane_tree: Option<SessionPaneTree>,
}

/// Synchronous persistence boundary for editor-session state.
pub trait SessionStore {
    /// Loads the current state, returning an empty state when no session exists.
    fn load(&self) -> Result<SessionState, SessionError>;

    /// Atomically saves the complete state.
    fn save(&self, state: &SessionState) -> Result<(), SessionError>;
}

/// Standard versioned session-file adapter.
#[derive(Clone, Debug)]
pub struct StdSessionStore {
    path: PathBuf,
}

impl StdSessionStore {
    /// Creates a store at an explicit path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Creates the conventional per-user state path for one application name.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] when the current platform has no usable per-user state directory.
    pub fn for_application(application: &str) -> Result<Self, SessionError> {
        let base = platform_state_directory()?;
        Ok(Self::new(
            base.join(application).join("editor-session-v2.txt"),
        ))
    }

    /// Returns the session-file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn platform_state_directory() -> Result<PathBuf, SessionError> {
    #[cfg(target_os = "macos")]
    {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library/Application Support"))
            .ok_or_else(|| {
                SessionError::invalid_path("HOME is required for macOS application state")
            });
    }

    #[cfg(target_os = "windows")]
    {
        return env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| {
                SessionError::invalid_path("LOCALAPPDATA is required for Windows state")
            });
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
            .ok_or_else(|| SessionError::invalid_path("no user state directory is available"))
    }
}

impl SessionStore for StdSessionStore {
    fn load(&self) -> Result<SessionState, SessionError> {
        let (content, source_path) = match fs::read_to_string(&self.path) {
            Ok(content) => (content, self.path.clone()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let legacy_path = self.path.with_file_name("editor-session-v1.txt");
                match fs::read_to_string(&legacy_path) {
                    Ok(content) => (content, legacy_path),
                    Err(legacy_error) if legacy_error.kind() == io::ErrorKind::NotFound => {
                        return Ok(SessionState::default());
                    }
                    Err(legacy_error) => {
                        return Err(SessionError::io(
                            "read legacy session",
                            &legacy_path,
                            legacy_error,
                        ));
                    }
                }
            }
            Err(error) => return Err(SessionError::io("read session", &self.path, error)),
        };
        decode_state(&content).map_err(|message| SessionError::decode(&source_path, message))
    }

    fn save(&self, state: &SessionState) -> Result<(), SessionError> {
        validate_state(state).map_err(SessionError::encode)?;
        let parent = self.path.parent().ok_or_else(|| {
            SessionError::invalid_path("session destination has no parent directory")
        })?;
        fs::create_dir_all(parent)
            .map_err(|error| SessionError::io("create session directory", parent, error))?;
        let temporary = self.path.with_extension("tmp");
        fs::write(&temporary, encode_state(state))
            .map_err(|error| SessionError::io("write temporary session", &temporary, error))?;
        if let Err(error) = fs::rename(&temporary, &self.path) {
            let _ = fs::remove_file(&temporary);
            return Err(SessionError::io("commit session", &self.path, error));
        }
        Ok(())
    }
}

/// In-memory session adapter for deterministic tests.
#[derive(Clone, Debug, Default)]
pub struct MemorySessionStore {
    state: Rc<RefCell<SessionState>>,
}

impl MemorySessionStore {
    /// Returns a clone of the currently stored state.
    #[must_use]
    pub fn state(&self) -> SessionState {
        self.state.borrow().clone()
    }

    /// Replaces the currently stored state.
    pub fn set_state(&self, state: SessionState) {
        *self.state.borrow_mut() = state;
    }
}

impl SessionStore for MemorySessionStore {
    fn load(&self) -> Result<SessionState, SessionError> {
        let state = self.state();
        validate_state(&state).map_err(SessionError::encode)?;
        Ok(state)
    }

    fn save(&self, state: &SessionState) -> Result<(), SessionError> {
        validate_state(state).map_err(SessionError::encode)?;
        self.set_state(state.clone());
        Ok(())
    }
}

/// Session persistence failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionError {
    operation: &'static str,
    path: Option<PathBuf>,
    message: String,
}

impl SessionError {
    /// Returns the stable operation label associated with this failure.
    #[must_use]
    pub const fn operation(&self) -> &'static str {
        self.operation
    }

    /// Returns the affected persistence path, when one exists.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn invalid_path(message: impl Into<String>) -> Self {
        Self {
            operation: "resolve session path",
            path: None,
            message: message.into(),
        }
    }

    fn io(operation: &'static str, path: &Path, error: io::Error) -> Self {
        Self {
            operation,
            path: Some(path.to_path_buf()),
            message: error.to_string(),
        }
    }

    fn decode(path: &Path, message: impl Into<String>) -> Self {
        Self {
            operation: "decode session",
            path: Some(path.to_path_buf()),
            message: message.into(),
        }
    }

    fn encode(message: impl Into<String>) -> Self {
        Self {
            operation: "encode session",
            path: None,
            message: message.into(),
        }
    }
}

impl Display for SessionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.path {
            Some(path) => write!(
                formatter,
                "{} {}: {}",
                self.operation,
                path.display(),
                self.message
            ),
            None => write!(formatter, "{}: {}", self.operation, self.message),
        }
    }
}

impl Error for SessionError {}

impl CodedError for SessionError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self.operation {
            "resolve session path" => "session.invalid_path",
            "decode session" => "session.decode",
            "encode session" => "session.encode",
            _ => "session.io",
        })
    }
}

fn encode_state(state: &SessionState) -> String {
    let mut output = String::from(FORMAT_HEADER);
    output.push('\n');
    for recent in &state.recent_files {
        push_fields(
            &mut output,
            &[
                "recent",
                &hex_encode(&path_bytes(&recent.path)),
                &hex_encode(recent.title.as_bytes()),
            ],
        );
    }
    if let Some(workspace) = &state.workspace {
        push_fields(
            &mut output,
            &["workspace", &hex_encode(&path_bytes(&workspace.root))],
        );
        if let Some(selected) = &workspace.selected_path {
            push_fields(
                &mut output,
                &["selected", &hex_encode(&path_bytes(selected))],
            );
        }
        for expanded in &workspace.expanded_paths {
            push_fields(
                &mut output,
                &["expanded", &hex_encode(&path_bytes(expanded))],
            );
        }
    }
    for document in &state.documents {
        let key = document.document_key.to_string();
        let title = hex_encode(document.title.as_bytes());
        let text = hex_encode(document.text.as_bytes());
        let dirty = if document.is_dirty { "1" } else { "0" };
        match &document.source {
            SessionDocumentSource::File(path) => {
                let (storage_revision, storage_instance) = document.storage_snapshot.map_or_else(
                    || ("-".to_owned(), "-".to_owned()),
                    |snapshot| (snapshot.revision.to_string(), snapshot.instance.to_string()),
                );
                push_fields(
                    &mut output,
                    &[
                        "document-file",
                        &key,
                        &title,
                        &hex_encode(&path_bytes(path)),
                        dirty,
                        &text,
                        &storage_revision,
                        &storage_instance,
                    ],
                );
            }
            SessionDocumentSource::Untitled(sequence) => {
                push_fields(
                    &mut output,
                    &[
                        "document-untitled",
                        &key,
                        &title,
                        &sequence.to_string(),
                        dirty,
                        &text,
                    ],
                );
            }
            SessionDocumentSource::Virtual(virtual_key) => push_fields(
                &mut output,
                &[
                    "document-virtual",
                    &key,
                    &title,
                    &hex_encode(virtual_key.as_bytes()),
                    dirty,
                    &text,
                ],
            ),
        }
    }
    for view in &state.views {
        let selection_anchor = view
            .selection_anchor_byte
            .map_or_else(|| "-".to_owned(), |offset| offset.to_string());
        let selection_focus = view
            .selection_focus_byte
            .map_or_else(|| "-".to_owned(), |offset| offset.to_string());
        push_fields(
            &mut output,
            &[
                "view",
                &view.view_key.to_string(),
                &view.document_key.to_string(),
                &view.caret_byte.to_string(),
                &selection_anchor,
                &selection_focus,
                &view.scroll_x.to_string(),
                &view.scroll_y.to_string(),
            ],
        );
    }
    if let Some(tree) = &state.pane_tree {
        push_fields(
            &mut output,
            &["pane-focus", &tree.focused_pane_key.to_string()],
        );
        encode_pane_node(&mut output, "r", &tree.root);
    }
    output
}

fn push_fields(output: &mut String, fields: &[&str]) {
    output.push_str(&fields.join("\t"));
    output.push('\n');
}

fn encode_pane_node(output: &mut String, path: &str, node: &SessionPaneNode) {
    match node {
        SessionPaneNode::Leaf {
            pane_key,
            tabs,
            active_view_key,
            tab_scroll_offset,
        } => {
            push_fields(
                output,
                &[
                    "pane-leaf",
                    path,
                    &pane_key.to_string(),
                    &active_view_key.to_string(),
                    &tab_scroll_offset.to_string(),
                ],
            );
            for tab in tabs {
                push_fields(
                    output,
                    &[
                        "pane-tab",
                        path,
                        &tab.view_key.to_string(),
                        bool_field(tab.is_pinned),
                        bool_field(tab.is_preview),
                    ],
                );
            }
        }
        SessionPaneNode::Split {
            split_key,
            axis,
            ratio_milli,
            first,
            second,
        } => {
            let axis = match axis {
                SessionPaneAxis::Horizontal => "h",
                SessionPaneAxis::Vertical => "v",
            };
            push_fields(
                output,
                &[
                    "pane-split",
                    path,
                    &split_key.to_string(),
                    axis,
                    &ratio_milli.to_string(),
                ],
            );
            encode_pane_node(output, &format!("{path}0"), first);
            encode_pane_node(output, &format!("{path}1"), second);
        }
    }
}

const fn bool_field(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

#[derive(Clone, Debug)]
enum PaneRecord {
    Leaf {
        pane_key: u64,
        active_view_key: u64,
        tab_scroll_offset: usize,
        tabs: Vec<SessionPaneTab>,
    },
    Split {
        split_key: u64,
        axis: SessionPaneAxis,
        ratio_milli: u16,
    },
}

fn decode_state(content: &str) -> Result<SessionState, String> {
    let mut lines = content.lines();
    let header = lines.next();
    if header != Some(FORMAT_HEADER) && header != Some(LEGACY_FORMAT_HEADER) {
        return Err("unsupported session format".to_owned());
    }
    let mut state = SessionState::default();
    let mut workspace_root = None;
    let mut selected_path = None;
    let mut expanded_paths = Vec::new();
    let mut focused_pane_key = None;
    let mut pane_records = BTreeMap::<String, PaneRecord>::new();
    for (index, line) in lines.enumerate() {
        if line.is_empty() {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        match fields.as_slice() {
            ["recent", path, title] => state.recent_files.push(SessionRecentFile {
                path: decode_path(path, index)?,
                title: decode_string(title, index)?,
            }),
            ["workspace", path] => workspace_root = Some(decode_path(path, index)?),
            ["selected", path] => selected_path = Some(decode_path(path, index)?),
            ["expanded", path] => expanded_paths.push(decode_path(path, index)?),
            ["document-file", key, title, path, dirty, text] => {
                state.documents.push(SessionDocument {
                    document_key: parse_field(key, index, "document key")?,
                    source: SessionDocumentSource::File(decode_path(path, index)?),
                    title: decode_string(title, index)?,
                    text: decode_string(text, index)?,
                    is_dirty: parse_bool(dirty, index)?,
                    storage_snapshot: None,
                });
            }
            [
                "document-file",
                key,
                title,
                path,
                dirty,
                text,
                revision,
                instance,
            ] => {
                let storage_snapshot = match (*revision, *instance) {
                    ("-", "-") => None,
                    ("-", _) | (_, "-") => {
                        return Err(line_error(index, "incomplete file storage snapshot"));
                    }
                    _ => Some(SessionStorageSnapshot {
                        revision: parse_field(revision, index, "storage revision")?,
                        instance: parse_field(instance, index, "storage instance")?,
                    }),
                };
                state.documents.push(SessionDocument {
                    document_key: parse_field(key, index, "document key")?,
                    source: SessionDocumentSource::File(decode_path(path, index)?),
                    title: decode_string(title, index)?,
                    text: decode_string(text, index)?,
                    is_dirty: parse_bool(dirty, index)?,
                    storage_snapshot,
                });
            }
            ["document-untitled", key, title, sequence, dirty, text] => {
                state.documents.push(SessionDocument {
                    document_key: parse_field(key, index, "document key")?,
                    source: SessionDocumentSource::Untitled(parse_field(
                        sequence,
                        index,
                        "untitled sequence",
                    )?),
                    title: decode_string(title, index)?,
                    text: decode_string(text, index)?,
                    is_dirty: parse_bool(dirty, index)?,
                    storage_snapshot: None,
                });
            }
            ["document-virtual", key, title, virtual_key, dirty, text] => {
                state.documents.push(SessionDocument {
                    document_key: parse_field(key, index, "document key")?,
                    source: SessionDocumentSource::Virtual(decode_string(virtual_key, index)?),
                    title: decode_string(title, index)?,
                    text: decode_string(text, index)?,
                    is_dirty: parse_bool(dirty, index)?,
                    storage_snapshot: None,
                });
            }
            ["view", view_key, document_key] => state.views.push(SessionDocumentView {
                view_key: parse_field(view_key, index, "view key")?,
                document_key: parse_field(document_key, index, "document key")?,
                caret_byte: 0,
                selection_anchor_byte: None,
                selection_focus_byte: None,
                scroll_x: 0,
                scroll_y: 0,
            }),
            [
                "view",
                view_key,
                document_key,
                caret_byte,
                selection_anchor,
                selection_focus,
                scroll_x,
                scroll_y,
            ] => {
                let selection = match (*selection_anchor, *selection_focus) {
                    ("-", "-") => (None, None),
                    ("-", _) | (_, "-") => {
                        return Err(line_error(index, "incomplete view selection"));
                    }
                    _ => (
                        Some(parse_field(selection_anchor, index, "selection anchor")?),
                        Some(parse_field(selection_focus, index, "selection focus")?),
                    ),
                };
                state.views.push(SessionDocumentView {
                    view_key: parse_field(view_key, index, "view key")?,
                    document_key: parse_field(document_key, index, "document key")?,
                    caret_byte: parse_field(caret_byte, index, "caret offset")?,
                    selection_anchor_byte: selection.0,
                    selection_focus_byte: selection.1,
                    scroll_x: parse_field(scroll_x, index, "horizontal scroll")?,
                    scroll_y: parse_field(scroll_y, index, "vertical scroll")?,
                });
            }
            ["pane-focus", key] => {
                focused_pane_key = Some(parse_field(key, index, "focused pane key")?);
            }
            ["pane-leaf", path, pane_key, active_view_key, offset] => {
                if pane_records
                    .insert(
                        (*path).to_owned(),
                        PaneRecord::Leaf {
                            pane_key: parse_field(pane_key, index, "pane key")?,
                            active_view_key: parse_field(
                                active_view_key,
                                index,
                                "active view key",
                            )?,
                            tab_scroll_offset: parse_field(offset, index, "tab scroll offset")?,
                            tabs: Vec::new(),
                        },
                    )
                    .is_some()
                {
                    return Err(line_error(index, "duplicate pane node path"));
                }
            }
            ["pane-tab", path, view_key, pinned, preview] => {
                let record = pane_records
                    .get_mut(*path)
                    .ok_or_else(|| line_error(index, "pane tab precedes its leaf"))?;
                let PaneRecord::Leaf { tabs, .. } = record else {
                    return Err(line_error(index, "pane tab targets a split"));
                };
                tabs.push(SessionPaneTab {
                    view_key: parse_field(view_key, index, "tab view key")?,
                    is_pinned: parse_bool(pinned, index)?,
                    is_preview: parse_bool(preview, index)?,
                });
            }
            ["pane-split", path, split_key, axis, ratio] => {
                let axis = match *axis {
                    "h" => SessionPaneAxis::Horizontal,
                    "v" => SessionPaneAxis::Vertical,
                    _ => return Err(line_error(index, "invalid pane axis")),
                };
                if pane_records
                    .insert(
                        (*path).to_owned(),
                        PaneRecord::Split {
                            split_key: parse_field(split_key, index, "split key")?,
                            axis,
                            ratio_milli: parse_field(ratio, index, "split ratio")?,
                        },
                    )
                    .is_some()
                {
                    return Err(line_error(index, "duplicate pane node path"));
                }
            }
            _ => return Err(line_error(index, "invalid session record")),
        }
    }
    if let Some(root) = workspace_root {
        state.workspace = Some(SessionWorkspace {
            root,
            expanded_paths,
            selected_path,
        });
    }
    if !pane_records.is_empty() {
        let focused_pane_key =
            focused_pane_key.ok_or_else(|| "pane records are missing focused pane".to_owned())?;
        let mut reachable_paths = BTreeSet::new();
        collect_reachable_pane_paths("r", &pane_records, &mut reachable_paths)?;
        if reachable_paths.len() != pane_records.len() {
            return Err("pane records contain unreachable nodes".to_owned());
        }
        state.pane_tree = Some(SessionPaneTree {
            focused_pane_key,
            root: build_pane_node("r", &pane_records)?,
        });
    } else if focused_pane_key.is_some() {
        return Err("focused pane record has no pane tree".to_owned());
    }
    validate_state(&state)?;
    Ok(state)
}

fn collect_reachable_pane_paths(
    path: &str,
    records: &BTreeMap<String, PaneRecord>,
    reachable: &mut BTreeSet<String>,
) -> Result<(), String> {
    if !reachable.insert(path.to_owned()) {
        return Err(format!("pane tree revisits path {path}"));
    }
    match records.get(path) {
        Some(PaneRecord::Leaf { .. }) => Ok(()),
        Some(PaneRecord::Split { .. }) => {
            collect_reachable_pane_paths(&format!("{path}0"), records, reachable)?;
            collect_reachable_pane_paths(&format!("{path}1"), records, reachable)
        }
        None => Err(format!("missing pane node at path {path}")),
    }
}

fn validate_state(state: &SessionState) -> Result<(), String> {
    let mut document_keys = BTreeSet::new();
    let mut file_sources = BTreeSet::new();
    let mut untitled_sequences = BTreeSet::new();
    let mut virtual_sources = BTreeSet::new();
    for document in &state.documents {
        if document.document_key == 0 || document.document_key == u64::MAX {
            return Err(format!(
                "invalid session document key {}",
                document.document_key
            ));
        }
        if !document_keys.insert(document.document_key) {
            return Err(format!(
                "duplicate session document key {}",
                document.document_key
            ));
        }
        match &document.source {
            SessionDocumentSource::File(path) => {
                if !file_sources.insert(path.clone()) {
                    return Err(format!(
                        "duplicate file-backed document source {}",
                        path.display()
                    ));
                }
            }
            SessionDocumentSource::Untitled(sequence) => {
                if *sequence == 0 || *sequence == u32::MAX || !untitled_sequences.insert(*sequence)
                {
                    return Err(format!("invalid or duplicate untitled sequence {sequence}"));
                }
            }
            SessionDocumentSource::Virtual(key) if key.is_empty() => {
                return Err(format!(
                    "virtual document {} has an empty key",
                    document.document_key
                ));
            }
            SessionDocumentSource::Virtual(key) => {
                if !virtual_sources.insert(key.clone()) {
                    return Err(format!("duplicate virtual document source {key}"));
                }
            }
        }
        if !matches!(&document.source, SessionDocumentSource::File(_))
            && document.storage_snapshot.is_some()
        {
            return Err(format!(
                "non-file document {} has a storage snapshot",
                document.document_key
            ));
        }
    }

    let mut view_keys = BTreeSet::new();
    let mut viewed_document_keys = BTreeSet::new();
    for view in &state.views {
        if view.view_key == 0 || view.view_key == u64::MAX {
            return Err(format!("invalid session view key {}", view.view_key));
        }
        if !view_keys.insert(view.view_key) {
            return Err(format!("duplicate session view key {}", view.view_key));
        }
        if !document_keys.contains(&view.document_key) {
            return Err(format!(
                "session view {} references missing document {}",
                view.view_key, view.document_key
            ));
        }
        let _ = viewed_document_keys.insert(view.document_key);
        if view.selection_anchor_byte.is_some() != view.selection_focus_byte.is_some() {
            return Err(format!(
                "session view {} has an incomplete selection",
                view.view_key
            ));
        }
        if view.scroll_x < 0 || view.scroll_y < 0 {
            return Err(format!(
                "session view {} has a negative scroll offset",
                view.view_key
            ));
        }
    }

    if viewed_document_keys != document_keys {
        return Err("session documents and view-referenced documents differ".to_owned());
    }

    if let Some(tree) = &state.pane_tree {
        let mut node_keys = BTreeSet::new();
        let mut pane_keys = BTreeSet::new();
        let mut owned_views = BTreeSet::new();
        validate_pane_node(
            &tree.root,
            &view_keys,
            &mut node_keys,
            &mut pane_keys,
            &mut owned_views,
        )?;
        if tree.focused_pane_key == 0 || tree.focused_pane_key == u64::MAX {
            return Err(format!("invalid focused pane {}", tree.focused_pane_key));
        }
        if !pane_keys.contains(&tree.focused_pane_key) {
            return Err(format!(
                "focused pane {} is not a leaf",
                tree.focused_pane_key
            ));
        }
        if owned_views != view_keys {
            return Err("session views and pane-owned views differ".to_owned());
        }
    } else if !state.views.is_empty() {
        return Err("session views exist without a pane tree".to_owned());
    }
    Ok(())
}

fn validate_pane_node(
    node: &SessionPaneNode,
    view_keys: &BTreeSet<u64>,
    node_keys: &mut BTreeSet<u64>,
    pane_keys: &mut BTreeSet<u64>,
    owned_views: &mut BTreeSet<u64>,
) -> Result<(), String> {
    match node {
        SessionPaneNode::Leaf {
            pane_key,
            tabs,
            active_view_key,
            tab_scroll_offset,
        } => {
            if *pane_key == 0 || *pane_key == u64::MAX {
                return Err(format!("invalid pane-node key {pane_key}"));
            }
            if !node_keys.insert(*pane_key) {
                return Err(format!("duplicate pane-node key {pane_key}"));
            }
            let _ = pane_keys.insert(*pane_key);
            if tabs.is_empty() {
                return Err(format!("pane {pane_key} has no tabs"));
            }
            let mut local_views = BTreeSet::new();
            let mut regular_seen = false;
            let mut preview_count = 0_usize;
            for tab in tabs {
                if !view_keys.contains(&tab.view_key) {
                    return Err(format!(
                        "pane {pane_key} references missing view {}",
                        tab.view_key
                    ));
                }
                if !local_views.insert(tab.view_key) || !owned_views.insert(tab.view_key) {
                    return Err(format!(
                        "view {} has duplicate pane ownership",
                        tab.view_key
                    ));
                }
                if tab.is_pinned {
                    if regular_seen {
                        return Err(format!(
                            "pane {pane_key} has a pinned tab after a regular tab"
                        ));
                    }
                    if tab.is_preview {
                        return Err(format!("pane {pane_key} has a pinned preview tab"));
                    }
                } else {
                    regular_seen = true;
                }
                if tab.is_preview {
                    preview_count = preview_count.saturating_add(1);
                }
            }
            if preview_count > 1 {
                return Err(format!("pane {pane_key} has multiple preview tabs"));
            }
            if !local_views.contains(active_view_key) {
                return Err(format!(
                    "pane {pane_key} active view {active_view_key} is not pane-local"
                ));
            }
            let regular_count = tabs.iter().filter(|tab| !tab.is_pinned).count();
            if *tab_scroll_offset > regular_count.saturating_sub(1) {
                return Err(format!("pane {pane_key} tab scroll offset is out of range"));
            }
            Ok(())
        }
        SessionPaneNode::Split {
            split_key,
            ratio_milli,
            first,
            second,
            ..
        } => {
            if *split_key == 0 || *split_key == u64::MAX {
                return Err(format!("invalid pane-node key {split_key}"));
            }
            if !node_keys.insert(*split_key) {
                return Err(format!("duplicate pane-node key {split_key}"));
            }
            if !(100..=900).contains(ratio_milli) {
                return Err(format!("split {split_key} ratio is out of range"));
            }
            validate_pane_node(first, view_keys, node_keys, pane_keys, owned_views)?;
            validate_pane_node(second, view_keys, node_keys, pane_keys, owned_views)
        }
    }
}

fn build_pane_node(
    path: &str,
    records: &BTreeMap<String, PaneRecord>,
) -> Result<SessionPaneNode, String> {
    match records.get(path) {
        Some(PaneRecord::Leaf {
            pane_key,
            active_view_key,
            tab_scroll_offset,
            tabs,
        }) => Ok(SessionPaneNode::Leaf {
            pane_key: *pane_key,
            tabs: tabs.clone(),
            active_view_key: *active_view_key,
            tab_scroll_offset: *tab_scroll_offset,
        }),
        Some(PaneRecord::Split {
            split_key,
            axis,
            ratio_milli,
        }) => Ok(SessionPaneNode::Split {
            split_key: *split_key,
            axis: *axis,
            ratio_milli: *ratio_milli,
            first: Box::new(build_pane_node(&format!("{path}0"), records)?),
            second: Box::new(build_pane_node(&format!("{path}1"), records)?),
        }),
        None => Err(format!("missing pane node at path {path}")),
    }
}

fn decode_path(value: &str, index: usize) -> Result<PathBuf, String> {
    Ok(path_from_bytes(
        hex_decode(value).map_err(|error| line_error(index, error))?,
    ))
}

fn decode_string(value: &str, index: usize) -> Result<String, String> {
    String::from_utf8(hex_decode(value).map_err(|error| line_error(index, error))?)
        .map_err(|error| line_error(index, error.to_string()))
}

fn parse_field<T>(value: &str, index: usize, name: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: Display,
{
    value
        .parse::<T>()
        .map_err(|error| line_error(index, format!("invalid {name}: {error}")))
}

fn parse_bool(value: &str, index: usize) -> Result<bool, String> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(line_error(index, "invalid boolean field")),
    }
}

fn line_error(index: usize, message: impl Display) -> String {
    format!("line {}: {message}", index.saturating_add(2))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn hex_decode(value: &str) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2) {
        return Err("hex field has odd length".to_owned());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0])?;
            let low = hex_nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn hex_nibble(value: u8) -> Result<u8, String> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err("hex field contains a non-hexadecimal character".to_owned()),
    }
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str().to_string_lossy().as_bytes().to_vec()
}

#[cfg(unix)]
fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        MemorySessionStore, SessionDocument, SessionDocumentSource, SessionDocumentView,
        SessionPaneAxis, SessionPaneNode, SessionPaneTab, SessionPaneTree, SessionRecentFile,
        SessionState, SessionStore, SessionWorkspace, StdSessionStore,
    };
    use std::error::Error;
    #[cfg(unix)]
    use std::ffi::OsString;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;

    #[test]
    fn memory_store_round_trips_complete_state() -> Result<(), Box<dyn Error>> {
        let store = MemorySessionStore::default();
        let state = SessionState {
            recent_files: vec![SessionRecentFile {
                path: PathBuf::from("/tmp/recent.txt"),
                title: "recent.txt".to_owned(),
            }],
            workspace: Some(SessionWorkspace {
                root: PathBuf::from("/tmp/workspace"),
                expanded_paths: vec![PathBuf::from("/tmp/workspace/src")],
                selected_path: Some(PathBuf::from("/tmp/workspace/src/main.rs")),
            }),
            ..SessionState::default()
        };
        store.save(&state)?;
        assert_eq!(store.load()?, state);
        Ok(())
    }

    #[test]
    fn encoded_state_preserves_spaces_in_paths() -> Result<(), Box<dyn Error>> {
        let state = SessionState {
            recent_files: vec![SessionRecentFile {
                path: PathBuf::from("/tmp/a file.txt"),
                title: "A File".to_owned(),
            }],
            workspace: None,
            ..SessionState::default()
        };
        let encoded = super::encode_state(&state);
        assert_eq!(super::decode_state(&encoded)?, state);
        Ok(())
    }

    #[test]
    fn version_two_round_trips_recursive_panes_and_shared_views() -> Result<(), Box<dyn Error>> {
        let state = SessionState {
            documents: vec![SessionDocument {
                document_key: 7,
                source: SessionDocumentSource::Virtual("editor".to_owned()),
                title: "Editor.rs".to_owned(),
                text: "shared text".to_owned(),
                is_dirty: true,
                storage_snapshot: None,
            }],
            views: vec![
                SessionDocumentView {
                    view_key: 11,
                    document_key: 7,
                    caret_byte: 8,
                    selection_anchor_byte: Some(2),
                    selection_focus_byte: Some(8),
                    scroll_x: 12,
                    scroll_y: 48,
                },
                SessionDocumentView {
                    view_key: 12,
                    document_key: 7,
                    caret_byte: 0,
                    selection_anchor_byte: None,
                    selection_focus_byte: None,
                    scroll_x: 0,
                    scroll_y: 0,
                },
            ],
            pane_tree: Some(SessionPaneTree {
                focused_pane_key: 3,
                root: SessionPaneNode::Split {
                    split_key: 2,
                    axis: SessionPaneAxis::Horizontal,
                    ratio_milli: 625,
                    first: Box::new(SessionPaneNode::Leaf {
                        pane_key: 1,
                        tabs: vec![SessionPaneTab {
                            view_key: 11,
                            is_pinned: true,
                            is_preview: false,
                        }],
                        active_view_key: 11,
                        tab_scroll_offset: 0,
                    }),
                    second: Box::new(SessionPaneNode::Leaf {
                        pane_key: 3,
                        tabs: vec![SessionPaneTab {
                            view_key: 12,
                            is_pinned: false,
                            is_preview: true,
                        }],
                        active_view_key: 12,
                        tab_scroll_offset: 0,
                    }),
                },
            }),
            ..SessionState::default()
        };
        let encoded = super::encode_state(&state);
        assert!(encoded.starts_with("LUNA_EDITOR_SESSION_V2\n"));
        assert_eq!(super::decode_state(&encoded)?, state);
        Ok(())
    }

    #[test]
    fn file_storage_baselines_round_trip_and_non_file_baselines_are_rejected()
    -> Result<(), Box<dyn Error>> {
        let state = SessionState {
            documents: vec![SessionDocument {
                document_key: 9,
                source: SessionDocumentSource::File(PathBuf::from("/tmp/baseline.txt")),
                title: "baseline.txt".to_owned(),
                text: "saved".to_owned(),
                is_dirty: false,
                storage_snapshot: Some(super::SessionStorageSnapshot {
                    revision: 44,
                    instance: 55,
                }),
            }],
            views: vec![SessionDocumentView {
                view_key: 10,
                document_key: 9,
                caret_byte: 0,
                selection_anchor_byte: None,
                selection_focus_byte: None,
                scroll_x: 0,
                scroll_y: 0,
            }],
            pane_tree: Some(SessionPaneTree {
                focused_pane_key: 1,
                root: SessionPaneNode::Leaf {
                    pane_key: 1,
                    tabs: vec![SessionPaneTab {
                        view_key: 10,
                        is_pinned: false,
                        is_preview: false,
                    }],
                    active_view_key: 10,
                    tab_scroll_offset: 0,
                },
            }),
            ..SessionState::default()
        };
        assert_eq!(super::decode_state(&super::encode_state(&state))?, state);

        let mut invalid = state;
        invalid.documents[0].source = SessionDocumentSource::Virtual("virtual".to_owned());
        assert!(super::decode_state(&super::encode_state(&invalid)).is_err());
        Ok(())
    }

    #[test]
    fn legacy_version_one_state_remains_readable() -> Result<(), Box<dyn Error>> {
        let state = super::decode_state(
            "LUNA_EDITOR_SESSION_V1\nrecent\t2f746d702f6f6c642e747874\t6f6c642e747874\n",
        )?;
        assert_eq!(state.recent_files.len(), 1);
        assert!(state.pane_tree.is_none());
        Ok(())
    }

    #[test]
    fn standard_store_falls_back_to_version_one_filename() -> Result<(), Box<dyn Error>> {
        let directory =
            std::env::temp_dir().join(format!("luna-session-v1-fallback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory)?;
        let current = directory.join("editor-session-v2.txt");
        let legacy = directory.join("editor-session-v1.txt");
        std::fs::write(
            &legacy,
            "LUNA_EDITOR_SESSION_V1\nrecent\t2f746d702f6f6c642e747874\t6f6c642e747874\n",
        )?;
        let state = StdSessionStore::new(current).load()?;
        assert_eq!(state.recent_files.len(), 1);
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn invalid_header_is_rejected() {
        assert!(super::decode_state("UNKNOWN\n").is_err());
    }

    #[test]
    fn damaged_pane_sessions_reject_orphans_and_invalid_tab_partitions() {
        let state = SessionState {
            documents: vec![SessionDocument {
                document_key: 1,
                source: SessionDocumentSource::Virtual("one".to_owned()),
                title: "One".to_owned(),
                text: String::new(),
                is_dirty: false,
                storage_snapshot: None,
            }],
            views: vec![SessionDocumentView {
                view_key: 2,
                document_key: 1,
                caret_byte: 0,
                selection_anchor_byte: None,
                selection_focus_byte: None,
                scroll_x: 0,
                scroll_y: 0,
            }],
            pane_tree: Some(SessionPaneTree {
                focused_pane_key: 3,
                root: SessionPaneNode::Leaf {
                    pane_key: 3,
                    tabs: vec![SessionPaneTab {
                        view_key: 2,
                        is_pinned: false,
                        is_preview: false,
                    }],
                    active_view_key: 2,
                    tab_scroll_offset: 0,
                },
            }),
            ..SessionState::default()
        };
        let mut orphan = super::encode_state(&state);
        orphan.push_str("pane-leaf\tx\t9\t2\t0\n");
        assert!(super::decode_state(&orphan).is_err());

        let invalid_partition = SessionState {
            documents: vec![
                state.documents[0].clone(),
                SessionDocument {
                    document_key: 4,
                    source: SessionDocumentSource::Virtual("two".to_owned()),
                    title: "Two".to_owned(),
                    text: String::new(),
                    is_dirty: false,
                    storage_snapshot: None,
                },
            ],
            views: vec![
                state.views[0].clone(),
                SessionDocumentView {
                    view_key: 5,
                    document_key: 4,
                    caret_byte: 0,
                    selection_anchor_byte: None,
                    selection_focus_byte: None,
                    scroll_x: 0,
                    scroll_y: 0,
                },
            ],
            pane_tree: Some(SessionPaneTree {
                focused_pane_key: 3,
                root: SessionPaneNode::Leaf {
                    pane_key: 3,
                    tabs: vec![
                        SessionPaneTab {
                            view_key: 2,
                            is_pinned: false,
                            is_preview: false,
                        },
                        SessionPaneTab {
                            view_key: 5,
                            is_pinned: true,
                            is_preview: false,
                        },
                    ],
                    active_view_key: 2,
                    tab_scroll_offset: 0,
                },
            }),
            ..SessionState::default()
        };
        assert!(super::decode_state(&super::encode_state(&invalid_partition)).is_err());
    }

    #[test]
    fn memory_store_rejects_duplicate_sources_and_exhausted_identities() {
        let store = MemorySessionStore::default();
        let duplicate_files = SessionState {
            documents: vec![
                SessionDocument {
                    document_key: 1,
                    source: SessionDocumentSource::File(PathBuf::from("/tmp/shared.txt")),
                    title: "One".to_owned(),
                    text: String::new(),
                    is_dirty: false,
                    storage_snapshot: None,
                },
                SessionDocument {
                    document_key: 2,
                    source: SessionDocumentSource::File(PathBuf::from("/tmp/shared.txt")),
                    title: "Two".to_owned(),
                    text: String::new(),
                    is_dirty: false,
                    storage_snapshot: None,
                },
            ],
            ..SessionState::default()
        };
        assert!(store.save(&duplicate_files).is_err());

        let exhausted = SessionState {
            documents: vec![SessionDocument {
                document_key: u64::MAX,
                source: SessionDocumentSource::Untitled(u32::MAX),
                title: "Exhausted".to_owned(),
                text: String::new(),
                is_dirty: true,
                storage_snapshot: None,
            }],
            ..SessionState::default()
        };
        assert!(store.save(&exhausted).is_err());
    }

    #[test]
    fn standard_store_round_trips_versioned_state() -> Result<(), Box<dyn Error>> {
        let path = std::env::temp_dir().join(format!(
            "luna-session-{}-round-trip.txt",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let store = StdSessionStore::new(&path);
        let state = SessionState {
            recent_files: vec![SessionRecentFile {
                path: PathBuf::from("/tmp/round-trip.txt"),
                title: "round-trip.txt".to_owned(),
            }],
            workspace: None,
            ..SessionState::default()
        };
        store.save(&state)?;
        assert_eq!(store.load()?, state);
        std::fs::remove_file(path)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn encoded_state_preserves_non_utf8_unix_path_bytes() -> Result<(), Box<dyn Error>> {
        let path = PathBuf::from(OsString::from_vec(vec![b'/', b't', b'm', b'p', b'/', 0xff]));
        let state = SessionState {
            recent_files: vec![SessionRecentFile {
                path,
                title: "raw-path".to_owned(),
            }],
            workspace: None,
            ..SessionState::default()
        };
        let encoded = super::encode_state(&state);
        assert_eq!(super::decode_state(&encoded)?, state);
        Ok(())
    }
}
