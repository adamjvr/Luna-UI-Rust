// SPDX-License-Identifier: MPL-2.0

//! Persistent product-neutral editor-session state.
//!
//! This crate stores recent-file paths and workspace restoration state without depending on UI,
//! document, or workspace-model crates. The standard adapter uses an atomic versioned text file;
//! the memory adapter provides deterministic tests and product harnesses.

use std::cell::RefCell;
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

const FORMAT_HEADER: &str = "LUNA_EDITOR_SESSION_V1";

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

/// Complete persisted editor-session state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionState {
    /// Recent files in most-recent-first order.
    pub recent_files: Vec<SessionRecentFile>,
    /// Last open workspace and tree state.
    pub workspace: Option<SessionWorkspace>,
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
    /// Returns [`SessionError`] when neither `XDG_STATE_HOME` nor `HOME` is available.
    pub fn for_application(application: &str) -> Result<Self, SessionError> {
        let base = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
            .ok_or_else(|| SessionError::invalid_path("no user state directory is available"))?;
        Ok(Self::new(
            base.join(application).join("editor-session-v1.txt"),
        ))
    }

    /// Returns the session-file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl SessionStore for StdSessionStore {
    fn load(&self) -> Result<SessionState, SessionError> {
        let content = match fs::read_to_string(&self.path) {
            Ok(content) => content,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(SessionState::default());
            }
            Err(error) => return Err(SessionError::io("read session", &self.path, error)),
        };
        decode_state(&content).map_err(|message| SessionError::decode(&self.path, message))
    }

    fn save(&self, state: &SessionState) -> Result<(), SessionError> {
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
        Ok(self.state())
    }

    fn save(&self, state: &SessionState) -> Result<(), SessionError> {
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

fn encode_state(state: &SessionState) -> String {
    let mut output = String::from(FORMAT_HEADER);
    output.push('\n');
    for recent in &state.recent_files {
        output.push_str("recent\t");
        output.push_str(&hex_encode(&path_bytes(&recent.path)));
        output.push('\t');
        output.push_str(&hex_encode(recent.title.as_bytes()));
        output.push('\n');
    }
    if let Some(workspace) = &state.workspace {
        output.push_str("workspace\t");
        output.push_str(&hex_encode(&path_bytes(&workspace.root)));
        output.push('\n');
        if let Some(selected) = &workspace.selected_path {
            output.push_str("selected\t");
            output.push_str(&hex_encode(&path_bytes(selected)));
            output.push('\n');
        }
        for expanded in &workspace.expanded_paths {
            output.push_str("expanded\t");
            output.push_str(&hex_encode(&path_bytes(expanded)));
            output.push('\n');
        }
    }
    output
}

fn decode_state(content: &str) -> Result<SessionState, String> {
    let mut lines = content.lines();
    if lines.next() != Some(FORMAT_HEADER) {
        return Err("unsupported session format".to_owned());
    }
    let mut state = SessionState::default();
    let mut workspace_root = None;
    let mut selected_path = None;
    let mut expanded_paths = Vec::new();
    for (index, line) in lines.enumerate() {
        if line.is_empty() {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        match fields.as_slice() {
            ["recent", path, title] => state.recent_files.push(SessionRecentFile {
                path: path_from_bytes(hex_decode(path).map_err(|error| line_error(index, error))?),
                title: String::from_utf8(
                    hex_decode(title).map_err(|error| line_error(index, error))?,
                )
                .map_err(|error| line_error(index, error.to_string()))?,
            }),
            ["workspace", path] => {
                workspace_root = Some(path_from_bytes(
                    hex_decode(path).map_err(|error| line_error(index, error))?,
                ));
            }
            ["selected", path] => {
                selected_path = Some(path_from_bytes(
                    hex_decode(path).map_err(|error| line_error(index, error))?,
                ));
            }
            ["expanded", path] => expanded_paths.push(path_from_bytes(
                hex_decode(path).map_err(|error| line_error(index, error))?,
            )),
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
    Ok(state)
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
        MemorySessionStore, SessionRecentFile, SessionState, SessionStore, SessionWorkspace,
        StdSessionStore,
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
        };
        let encoded = super::encode_state(&state);
        assert_eq!(super::decode_state(&encoded)?, state);
        Ok(())
    }

    #[test]
    fn invalid_header_is_rejected() {
        assert!(super::decode_state("UNKNOWN\n").is_err());
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
        };
        let encoded = super::encode_state(&state);
        assert_eq!(super::decode_state(&encoded)?, state);
        Ok(())
    }
}
