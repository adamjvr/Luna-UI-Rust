// SPDX-License-Identifier: MPL-2.0

//! Testable text-file and document-dialog service boundaries.
//!
//! The traits in this crate are synchronous by design because the current native editor demo owns
//! a single winit event loop and invokes platform dialogs as modal operations. Products may adapt
//! the same contracts to asynchronous task systems later without moving filesystem or dialog policy
//! into Luna's document identity model.
//!
//! [`StdTextFileService`] provides UTF-8 reads, deterministic content revisions, optimistic write
//! preconditions, and same-directory atomic replacement. [`SystemDialogService`] uses AppleScript
//! dialogs on macOS or an installed Linux helper (`zenity` first, then `kdialog`) without moving
//! toolkit policy into Luna UI. [`MemoryTextFileService`] and [`ScriptedDialogService`] provide fully
//! deterministic adapters for unit tests. The dialog boundary also includes workspace-folder
//! selection so products can compose document and project lifecycles without toolkit coupling.

mod dialog_response;

use dialog_response::zenity_extra_button_selected;
use luna_core::{CodedError, ErrorCode};
use luna_documents::{FileIdentity, StorageInstance, StorageRevision, StorageSnapshot};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::ffi::OsString;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Output};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Successfully loaded UTF-8 text and its canonical storage identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedTextFile {
    identity: FileIdentity,
    text: String,
    snapshot: StorageSnapshot,
}

impl LoadedTextFile {
    /// Returns the canonical identity used for duplicate-open prevention.
    #[must_use]
    pub const fn identity(&self) -> &FileIdentity {
        &self.identity
    }

    /// Returns decoded UTF-8 text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Consumes the load result and returns its decoded text.
    #[must_use]
    pub fn into_text(self) -> String {
        self.text
    }

    /// Returns the deterministic content revision observed during the read.
    #[must_use]
    pub const fn revision(&self) -> StorageRevision {
        self.snapshot.revision()
    }

    /// Returns the complete storage snapshot observed during the read.
    #[must_use]
    pub const fn snapshot(&self) -> StorageSnapshot {
        self.snapshot
    }
}

/// Successfully written text and the resulting storage revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrittenTextFile {
    identity: FileIdentity,
    snapshot: StorageSnapshot,
}

impl WrittenTextFile {
    /// Returns the canonical destination identity.
    #[must_use]
    pub const fn identity(&self) -> &FileIdentity {
        &self.identity
    }

    /// Returns the deterministic content revision after the write.
    #[must_use]
    pub const fn revision(&self) -> StorageRevision {
        self.snapshot.revision()
    }

    /// Returns the complete storage snapshot after the write.
    #[must_use]
    pub const fn snapshot(&self) -> StorageSnapshot {
        self.snapshot
    }
}

/// Optimistic condition that must hold before a file is replaced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WritePrecondition {
    /// Replace or create the destination regardless of its current revision.
    Any,
    /// Write only when no destination currently exists.
    Missing,
    /// Write only when the current destination revision and storage instance match this snapshot.
    Matches(StorageSnapshot),
}

/// Current storage state observed for a canonical file path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileObservation {
    /// The path currently refers to a readable storage object.
    Present(StorageSnapshot),
    /// The path does not currently exist.
    Missing,
}

/// Product-neutral synchronous UTF-8 text-file operations.
pub trait TextFileService {
    /// Loads one file as UTF-8, returning canonical identity and content revision.
    fn load_utf8(&self, path: &Path) -> Result<LoadedTextFile, FileServiceError>;

    /// Produces the canonical identity that a Save As destination would receive.
    fn identity_for_save(&self, path: &Path) -> Result<FileIdentity, FileServiceError>;

    /// Observes whether a canonical path is present and, when present, returns its current
    /// content revision and concrete storage-instance identity.
    fn observe_file(&self, path: &Path) -> Result<FileObservation, FileServiceError>;

    /// Writes UTF-8 text through a same-directory temporary file and atomic replacement.
    fn write_utf8_atomic(
        &self,
        path: &Path,
        text: &str,
        precondition: WritePrecondition,
    ) -> Result<WrittenTextFile, FileServiceError>;
}

/// Broad file-service failure classification suitable for product policy and tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileServiceErrorKind {
    /// A requested file or parent directory does not exist.
    NotFound,
    /// The operating system denied the requested operation.
    PermissionDenied,
    /// File bytes are not valid UTF-8.
    InvalidUtf8,
    /// A write precondition did not match the current storage snapshot.
    Conflict,
    /// A path cannot be represented as a Luna file identity.
    InvalidPath,
    /// Another input/output failure occurred.
    Io,
}

/// Error returned by a [`TextFileService`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileServiceError {
    operation: &'static str,
    path: PathBuf,
    kind: FileServiceErrorKind,
    message: String,
    expected_revision: Option<StorageRevision>,
    observed_revision: Option<StorageRevision>,
}

impl FileServiceError {
    /// Returns the operation that failed.
    #[must_use]
    pub const fn operation(&self) -> &'static str {
        self.operation
    }

    /// Returns the affected path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the broad failure classification.
    #[must_use]
    pub const fn kind(&self) -> FileServiceErrorKind {
        self.kind
    }

    /// Returns the expected revision for a conflict, when available.
    #[must_use]
    pub const fn expected_revision(&self) -> Option<StorageRevision> {
        self.expected_revision
    }

    /// Returns the observed revision for a conflict, when available.
    #[must_use]
    pub const fn observed_revision(&self) -> Option<StorageRevision> {
        self.observed_revision
    }

    fn from_io(operation: &'static str, path: &Path, error: &io::Error) -> Self {
        let kind = match error.kind() {
            io::ErrorKind::NotFound => FileServiceErrorKind::NotFound,
            io::ErrorKind::PermissionDenied => FileServiceErrorKind::PermissionDenied,
            _ => FileServiceErrorKind::Io,
        };
        Self {
            operation,
            path: path.to_path_buf(),
            kind,
            message: error.to_string(),
            expected_revision: None,
            observed_revision: None,
        }
    }

    fn invalid_utf8(path: &Path, error: impl Display) -> Self {
        Self {
            operation: "decode UTF-8",
            path: path.to_path_buf(),
            kind: FileServiceErrorKind::InvalidUtf8,
            message: error.to_string(),
            expected_revision: None,
            observed_revision: None,
        }
    }

    fn invalid_path(path: &Path, error: impl Display) -> Self {
        Self {
            operation: "canonicalize path",
            path: path.to_path_buf(),
            kind: FileServiceErrorKind::InvalidPath,
            message: error.to_string(),
            expected_revision: None,
            observed_revision: None,
        }
    }

    fn conflict(
        path: &Path,
        expected_revision: Option<StorageRevision>,
        observed_revision: Option<StorageRevision>,
    ) -> Self {
        Self {
            operation: "check write precondition",
            path: path.to_path_buf(),
            kind: FileServiceErrorKind::Conflict,
            message: "destination storage changed since the editor baseline".to_owned(),
            expected_revision,
            observed_revision,
        }
    }
}

impl Display for FileServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "failed to {} {}: {}",
            self.operation,
            self.path.display(),
            self.message
        )
    }
}

impl Error for FileServiceError {}

impl CodedError for FileServiceError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self.kind {
            FileServiceErrorKind::NotFound => "file.not_found",
            FileServiceErrorKind::PermissionDenied => "file.permission_denied",
            FileServiceErrorKind::InvalidUtf8 => "file.invalid_utf8",
            FileServiceErrorKind::Conflict => "file.conflict",
            FileServiceErrorKind::InvalidPath => "file.invalid_path",
            FileServiceErrorKind::Io => "file.io",
        })
    }
}

/// Standard-library UTF-8 file adapter.
#[derive(Clone, Copy, Debug, Default)]
pub struct StdTextFileService;

impl StdTextFileService {
    fn identity_for_existing(path: &Path) -> Result<FileIdentity, FileServiceError> {
        let canonical = fs::canonicalize(path)
            .map_err(|error| FileServiceError::from_io("canonicalize", path, &error))?;
        FileIdentity::from_canonical_path(canonical)
            .map_err(|error| FileServiceError::invalid_path(path, error))
    }

    fn destination_path(path: &Path) -> Result<PathBuf, FileServiceError> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|error| FileServiceError::from_io("read current directory", path, &error))?
                .join(path)
        };
        if absolute.exists() {
            return fs::canonicalize(&absolute)
                .map_err(|error| FileServiceError::from_io("canonicalize", &absolute, &error));
        }
        let file_name = absolute.file_name().ok_or_else(|| {
            FileServiceError::invalid_path(path, "save destination has no file name")
        })?;
        let parent = absolute.parent().ok_or_else(|| {
            FileServiceError::invalid_path(path, "save destination has no parent directory")
        })?;
        let canonical_parent = fs::canonicalize(parent)
            .map_err(|error| FileServiceError::from_io("canonicalize parent", parent, &error))?;
        Ok(canonical_parent.join(file_name))
    }

    fn read_existing_snapshot(
        path: &Path,
    ) -> Result<Option<(Vec<u8>, StorageSnapshot)>, FileServiceError> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(FileServiceError::from_io(
                    "open storage snapshot",
                    path,
                    &error,
                ));
            }
        };
        let metadata = file
            .metadata()
            .map_err(|error| FileServiceError::from_io("read metadata", path, &error))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| FileServiceError::from_io("read storage snapshot", path, &error))?;
        let snapshot = Self::snapshot_for_bytes_and_metadata(path, &bytes, &metadata);
        Ok(Some((bytes, snapshot)))
    }

    #[cfg(unix)]
    fn storage_instance(_path: &Path, metadata: &Metadata) -> StorageInstance {
        let value = (u128::from(metadata.dev()) << 64) | u128::from(metadata.ino());
        StorageInstance::new(value)
    }

    #[cfg(not(unix))]
    fn storage_instance(path: &Path, metadata: &Metadata) -> StorageInstance {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in path.as_os_str().to_string_lossy().as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= metadata.len();
        StorageInstance::new(u128::from(hash))
    }

    fn snapshot_for_bytes_and_metadata(
        path: &Path,
        bytes: &[u8],
        metadata: &Metadata,
    ) -> StorageSnapshot {
        StorageSnapshot::new(
            content_revision(bytes),
            Self::storage_instance(path, metadata),
        )
    }

    fn snapshot_for_bytes(path: &Path, bytes: &[u8]) -> Result<StorageSnapshot, FileServiceError> {
        let metadata = fs::metadata(path)
            .map_err(|error| FileServiceError::from_io("read metadata", path, &error))?;
        Ok(Self::snapshot_for_bytes_and_metadata(
            path, bytes, &metadata,
        ))
    }

    fn check_precondition(
        path: &Path,
        precondition: WritePrecondition,
    ) -> Result<(), FileServiceError> {
        let observed = Self::read_existing_snapshot(path)?.map(|(_, snapshot)| snapshot);
        let matches = match precondition {
            WritePrecondition::Any => true,
            WritePrecondition::Missing => observed.is_none(),
            WritePrecondition::Matches(expected) => observed == Some(expected),
        };
        if matches {
            Ok(())
        } else {
            let expected_revision = match precondition {
                WritePrecondition::Matches(snapshot) => Some(snapshot.revision()),
                WritePrecondition::Any | WritePrecondition::Missing => None,
            };
            let observed_revision = observed.map(StorageSnapshot::revision);
            Err(FileServiceError::conflict(
                path,
                expected_revision,
                observed_revision,
            ))
        }
    }

    fn temporary_path(destination: &Path) -> Result<PathBuf, FileServiceError> {
        let parent = destination.parent().ok_or_else(|| {
            FileServiceError::invalid_path(destination, "destination has no parent directory")
        })?;
        let file_name = destination.file_name().ok_or_else(|| {
            FileServiceError::invalid_path(destination, "destination has no file name")
        })?;
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut temporary_name = OsString::from(".");
        temporary_name.push(file_name);
        temporary_name.push(format!(".luna-{}-{sequence}.tmp", std::process::id()));
        Ok(parent.join(temporary_name))
    }

    fn replace_destination(temporary: &Path, destination: &Path) -> io::Result<()> {
        #[cfg(windows)]
        if destination.exists() {
            fs::remove_file(destination)?;
        }
        fs::rename(temporary, destination)
    }

    fn write_replacement(
        temporary: &Path,
        destination: &Path,
        text: &str,
        precondition: WritePrecondition,
    ) -> Result<(), FileServiceError> {
        let existing_permissions = fs::metadata(destination)
            .ok()
            .map(|metadata| metadata.permissions());
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(temporary)
            .map_err(|error| {
                FileServiceError::from_io("create temporary file", temporary, &error)
            })?;
        if let Some(permissions) = existing_permissions {
            file.set_permissions(permissions).map_err(|error| {
                FileServiceError::from_io("copy destination permissions", temporary, &error)
            })?;
        }
        file.write_all(text.as_bytes()).map_err(|error| {
            FileServiceError::from_io("write temporary file", temporary, &error)
        })?;
        file.sync_all()
            .map_err(|error| FileServiceError::from_io("sync temporary file", temporary, &error))?;
        Self::check_precondition(destination, precondition)?;
        Self::replace_destination(temporary, destination).map_err(|error| {
            FileServiceError::from_io("replace destination", destination, &error)
        })?;
        #[cfg(unix)]
        if let Some(parent) = destination.parent() {
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| {
                    FileServiceError::from_io("sync parent directory", parent, &error)
                })?;
        }
        Ok(())
    }
}

impl TextFileService for StdTextFileService {
    fn load_utf8(&self, path: &Path) -> Result<LoadedTextFile, FileServiceError> {
        let identity = Self::identity_for_existing(path)?;
        let Some((bytes, snapshot)) = Self::read_existing_snapshot(identity.path())? else {
            return Err(FileServiceError::from_io(
                "read",
                identity.path(),
                &io::Error::from(io::ErrorKind::NotFound),
            ));
        };
        let text = String::from_utf8(bytes)
            .map_err(|error| FileServiceError::invalid_utf8(identity.path(), error))?;
        Ok(LoadedTextFile {
            identity,
            text,
            snapshot,
        })
    }

    fn identity_for_save(&self, path: &Path) -> Result<FileIdentity, FileServiceError> {
        let destination = Self::destination_path(path)?;
        FileIdentity::from_canonical_path(destination)
            .map_err(|error| FileServiceError::invalid_path(path, error))
    }

    fn observe_file(&self, path: &Path) -> Result<FileObservation, FileServiceError> {
        match Self::read_existing_snapshot(path)? {
            Some((_, snapshot)) => Ok(FileObservation::Present(snapshot)),
            None => Ok(FileObservation::Missing),
        }
    }

    fn write_utf8_atomic(
        &self,
        path: &Path,
        text: &str,
        precondition: WritePrecondition,
    ) -> Result<WrittenTextFile, FileServiceError> {
        let identity = self.identity_for_save(path)?;
        let destination = identity.path();
        Self::check_precondition(destination, precondition)?;
        let temporary = Self::temporary_path(destination)?;
        let write_result = Self::write_replacement(&temporary, destination, text, precondition);
        if write_result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        write_result?;
        let bytes = text.as_bytes();
        let snapshot = Self::snapshot_for_bytes(identity.path(), bytes)?;
        Ok(WrittenTextFile { identity, snapshot })
    }
}

/// User decision when closing a dirty document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirtyCloseChoice {
    /// Save the document, then close only after a successful write.
    Save,
    /// Discard current edits and close immediately.
    Discard,
    /// Leave the document open and unchanged.
    Cancel,
}

/// User decision after optimistic save conflict detection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveConflictChoice {
    /// Replace the destination regardless of its observed revision.
    Overwrite,
    /// Reload current storage content and discard editor changes.
    Reload,
    /// Leave storage and editor content unchanged.
    Cancel,
}

/// User decision when a workspace mutation collides with an existing regular file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceCollisionChoice {
    /// Replace the existing regular file.
    Replace,
    /// Leave the workspace unchanged.
    Cancel,
}

/// User decision before deleting one workspace entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceDeleteChoice {
    /// Delete the selected entry or directory tree.
    Delete,
    /// Leave the workspace unchanged.
    Cancel,
}

/// User decision for an open dirty document affected by workspace deletion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceDirtyDeleteChoice {
    /// Detach the editor buffer into a new untitled document before deletion.
    KeepOpen,
    /// Discard the editor buffer and close its tab after deletion.
    DiscardAndClose,
    /// Cancel the complete workspace deletion.
    Cancel,
}

/// Product-neutral synchronous dialog operations used by the document lifecycle.
pub trait DocumentDialogService {
    /// Chooses one file to open, or returns `None` when canceled.
    fn choose_open_file(&mut self) -> Result<Option<PathBuf>, DialogError>;

    /// Chooses one directory to open as a workspace, or returns `None` when canceled.
    fn choose_open_folder(&mut self) -> Result<Option<PathBuf>, DialogError>;

    /// Chooses one Save As destination, or returns `None` when canceled.
    fn choose_save_file(
        &mut self,
        suggested_name: &str,
        current_path: Option<&Path>,
    ) -> Result<Option<PathBuf>, DialogError>;

    /// Resolves a dirty-document close request.
    fn confirm_dirty_close(&mut self, title: &str) -> Result<DirtyCloseChoice, DialogError>;

    /// Resolves a storage conflict detected before writing.
    fn resolve_save_conflict(
        &mut self,
        title: &str,
        path: &Path,
    ) -> Result<SaveConflictChoice, DialogError>;

    /// Prompts for one workspace leaf name, or returns `None` when canceled.
    fn prompt_workspace_name(
        &mut self,
        title: &str,
        prompt: &str,
        initial_name: &str,
    ) -> Result<Option<String>, DialogError>;

    /// Resolves replacement of an existing regular file.
    fn confirm_workspace_replace(
        &mut self,
        path: &Path,
    ) -> Result<WorkspaceCollisionChoice, DialogError>;

    /// Confirms deletion of one file or recursive directory tree.
    fn confirm_workspace_delete(
        &mut self,
        path: &Path,
        is_directory: bool,
    ) -> Result<WorkspaceDeleteChoice, DialogError>;

    /// Resolves a dirty open document affected by workspace deletion.
    fn resolve_dirty_workspace_delete(
        &mut self,
        title: &str,
        path: &Path,
    ) -> Result<WorkspaceDirtyDeleteChoice, DialogError>;
}

/// Broad native-dialog failure classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogErrorKind {
    /// No supported native dialog helper is installed.
    Unavailable,
    /// Launching or communicating with the helper failed.
    Io,
    /// The helper returned an unsupported result.
    InvalidResponse,
}

/// Error returned by a [`DocumentDialogService`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogError {
    kind: DialogErrorKind,
    message: String,
}

impl DialogError {
    /// Returns the broad failure classification.
    #[must_use]
    pub const fn kind(&self) -> DialogErrorKind {
        self.kind
    }

    fn unavailable() -> Self {
        Self {
            kind: DialogErrorKind::Unavailable,
            message: "no supported native document dialog backend is available".to_owned(),
        }
    }

    fn io(context: &str, error: impl Display) -> Self {
        Self {
            kind: DialogErrorKind::Io,
            message: format!("{context}: {error}"),
        }
    }

    fn invalid_response(message: impl Into<String>) -> Self {
        Self {
            kind: DialogErrorKind::InvalidResponse,
            message: message.into(),
        }
    }
}

impl Display for DialogError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for DialogError {}

impl CodedError for DialogError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self.kind {
            DialogErrorKind::Unavailable => "dialog.unavailable",
            DialogErrorKind::Io => "dialog.io",
            DialogErrorKind::InvalidResponse => "dialog.invalid_response",
        })
    }
}

/// Native helper selected by [`SystemDialogService`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemDialogBackend {
    /// GNOME-compatible Zenity helper.
    Zenity,
    /// KDE KDialog helper.
    KDialog,
    /// macOS Standard Additions dialogs through `/usr/bin/osascript`.
    AppleScript,
    /// No supported helper was detected.
    Unavailable,
}

/// Native desktop dialog adapter backed by AppleScript, Zenity, or KDialog.
#[derive(Clone, Copy, Debug)]
pub struct SystemDialogService {
    backend: SystemDialogBackend,
}

impl Default for SystemDialogService {
    fn default() -> Self {
        Self::detect()
    }
}

impl SystemDialogService {
    /// Detects the native macOS adapter or an installed Linux dialog helper.
    #[must_use]
    pub fn detect() -> Self {
        #[cfg(target_os = "macos")]
        let backend = if Path::new("/usr/bin/osascript").is_file() {
            SystemDialogBackend::AppleScript
        } else {
            SystemDialogBackend::Unavailable
        };

        #[cfg(not(target_os = "macos"))]
        let backend = if command_exists("zenity") {
            SystemDialogBackend::Zenity
        } else if command_exists("kdialog") {
            SystemDialogBackend::KDialog
        } else {
            SystemDialogBackend::Unavailable
        };

        Self { backend }
    }

    /// Returns the detected helper backend.
    #[must_use]
    pub const fn backend(self) -> SystemDialogBackend {
        self.backend
    }

    fn run(&self, program: &str, args: &[OsString]) -> Result<Output, DialogError> {
        Command::new(program)
            .args(args)
            .output()
            .map_err(|error| DialogError::io("failed to launch native dialog", error))
    }

    fn run_applescript(&self, script: &str, args: &[OsString]) -> Result<Output, DialogError> {
        let mut command = Command::new("/usr/bin/osascript");
        command.arg("-e").arg(script).arg("--").args(args);
        command
            .output()
            .map_err(|error| DialogError::io("failed to launch macOS dialog", error))
    }

    fn applescript_question(
        &self,
        title: &str,
        text: &str,
        ok_label: &str,
        extra_label: &str,
    ) -> Result<QuestionResult, DialogError> {
        const SCRIPT: &str = r#"on run argv
set dialogTitle to item 1 of argv
set dialogText to item 2 of argv
set primaryLabel to item 3 of argv
set secondaryLabel to item 4 of argv
try
    if secondaryLabel is "Cancel" then
        set answer to display dialog dialogText with title dialogTitle buttons {"Cancel", primaryLabel} default button primaryLabel cancel button "Cancel"
    else
        set answer to display dialog dialogText with title dialogTitle buttons {"Cancel", secondaryLabel, primaryLabel} default button primaryLabel cancel button "Cancel"
    end if
    return button returned of answer
on error number -128
    return "Cancel"
end try
end run"#;
        let output = self.run_applescript(
            SCRIPT,
            &[
                OsString::from(title),
                OsString::from(text),
                OsString::from(ok_label),
                OsString::from(extra_label),
            ],
        )?;
        if !output.status.success() {
            return Err(dialog_output_error(output));
        }
        let response = String::from_utf8(output.stdout)
            .map_err(|error| DialogError::invalid_response(error.to_string()))?;
        Ok(match response.trim() {
            value if value == ok_label => QuestionResult::Primary,
            "Cancel" => QuestionResult::Cancel,
            value if value == extra_label => QuestionResult::Secondary,
            value => {
                return Err(DialogError::invalid_response(format!(
                    "unsupported macOS dialog response: {value}"
                )));
            }
        })
    }

    fn selected_path(output: Output) -> Result<Option<PathBuf>, DialogError> {
        if output.status.success() {
            return path_from_dialog_stdout(output.stdout);
        }
        if is_cancel_status(output.status) {
            return Ok(None);
        }
        Err(dialog_output_error(output))
    }

    fn zenity_question(
        &self,
        title: &str,
        text: &str,
        ok_label: &str,
        extra_label: &str,
    ) -> Result<QuestionResult, DialogError> {
        let mut arguments = vec![
            OsString::from("--question"),
            OsString::from(format!("--title={title}")),
            OsString::from(format!("--text={text}")),
            OsString::from(format!("--ok-label={ok_label}")),
            OsString::from("--cancel-label=Cancel"),
        ];
        if extra_label != "Cancel" {
            arguments.push(OsString::from(format!("--extra-button={extra_label}")));
        }

        let output = self.run("zenity", &arguments)?;
        if zenity_extra_button_selected(&output.stdout, &output.stderr, extra_label) {
            return Ok(QuestionResult::Secondary);
        }
        if output.status.success() {
            return Ok(QuestionResult::Primary);
        }
        if is_cancel_status(output.status) {
            Ok(QuestionResult::Cancel)
        } else {
            Err(dialog_output_error(output))
        }
    }

    fn kdialog_question(&self, title: &str, text: &str) -> Result<QuestionResult, DialogError> {
        let output = self.run(
            "kdialog",
            &[
                OsString::from("--title"),
                OsString::from(title),
                OsString::from("--warningyesnocancel"),
                OsString::from(text),
            ],
        )?;
        match output.status.code() {
            Some(0) => Ok(QuestionResult::Primary),
            Some(1) => Ok(QuestionResult::Secondary),
            Some(2) => Ok(QuestionResult::Cancel),
            _ => Err(dialog_output_error(output)),
        }
    }
}

impl DocumentDialogService for SystemDialogService {
    fn choose_open_file(&mut self) -> Result<Option<PathBuf>, DialogError> {
        match self.backend {
            SystemDialogBackend::Zenity => Self::selected_path(self.run(
                "zenity",
                &[
                    OsString::from("--file-selection"),
                    OsString::from("--title=Open Text File"),
                ],
            )?),
            SystemDialogBackend::KDialog => Self::selected_path(self.run(
                "kdialog",
                &[
                    OsString::from("--title"),
                    OsString::from("Open Text File"),
                    OsString::from("--getopenfilename"),
                    OsString::from("."),
                    OsString::from("Text files (*.txt *.md *.rs *.toml *.json);;All files (*)"),
                ],
            )?),
            SystemDialogBackend::AppleScript => Self::selected_path(self.run_applescript(
                r#"try
set chosenFile to choose file with prompt "Open Text File"
return POSIX path of chosenFile
on error number -128
return ""
end try"#,
                &[],
            )?),
            SystemDialogBackend::Unavailable => Err(DialogError::unavailable()),
        }
    }

    fn choose_open_folder(&mut self) -> Result<Option<PathBuf>, DialogError> {
        match self.backend {
            SystemDialogBackend::Zenity => Self::selected_path(self.run(
                "zenity",
                &[
                    OsString::from("--file-selection"),
                    OsString::from("--directory"),
                    OsString::from("--title=Open Workspace Folder"),
                ],
            )?),
            SystemDialogBackend::KDialog => Self::selected_path(self.run(
                "kdialog",
                &[
                    OsString::from("--title"),
                    OsString::from("Open Workspace Folder"),
                    OsString::from("--getexistingdirectory"),
                    OsString::from("."),
                ],
            )?),
            SystemDialogBackend::AppleScript => Self::selected_path(self.run_applescript(
                r#"try
set chosenFolder to choose folder with prompt "Open Workspace Folder"
return POSIX path of chosenFolder
on error number -128
return ""
end try"#,
                &[],
            )?),
            SystemDialogBackend::Unavailable => Err(DialogError::unavailable()),
        }
    }

    fn choose_save_file(
        &mut self,
        suggested_name: &str,
        current_path: Option<&Path>,
    ) -> Result<Option<PathBuf>, DialogError> {
        let suggested =
            current_path.map_or_else(|| PathBuf::from(suggested_name), Path::to_path_buf);
        match self.backend {
            SystemDialogBackend::Zenity => Self::selected_path(self.run(
                "zenity",
                &[
                    OsString::from("--file-selection"),
                    OsString::from("--save"),
                    OsString::from("--confirm-overwrite"),
                    OsString::from("--title=Save Text File"),
                    OsString::from(format!("--filename={}", suggested.display())),
                ],
            )?),
            SystemDialogBackend::KDialog => Self::selected_path(self.run(
                "kdialog",
                &[
                    OsString::from("--title"),
                    OsString::from("Save Text File"),
                    OsString::from("--getsavefilename"),
                    suggested.clone().into_os_string(),
                    OsString::from("Text files (*.txt *.md *.rs *.toml *.json);;All files (*)"),
                ],
            )?),
            SystemDialogBackend::AppleScript => {
                let directory = suggested.parent().unwrap_or_else(|| Path::new("."));
                let name = suggested
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new(suggested_name));
                Self::selected_path(self.run_applescript(
                    r#"on run argv
set defaultFolder to POSIX file (item 1 of argv)
set defaultName to item 2 of argv
try
    set chosenFile to choose file name with prompt "Save Text File" default location defaultFolder default name defaultName
    return POSIX path of chosenFile
on error number -128
    return ""
end try
end run"#,
                    &[directory.as_os_str().to_owned(), name.to_owned()],
                )?)
            }
            SystemDialogBackend::Unavailable => Err(DialogError::unavailable()),
        }
    }

    fn confirm_dirty_close(&mut self, title: &str) -> Result<DirtyCloseChoice, DialogError> {
        let text = format!("Save changes to {title} before closing?");
        let result = match self.backend {
            SystemDialogBackend::Zenity => {
                self.zenity_question("Unsaved Changes", &text, "Save", "Discard")?
            }
            SystemDialogBackend::KDialog => self.kdialog_question("Unsaved Changes", &text)?,
            SystemDialogBackend::AppleScript => {
                self.applescript_question("Unsaved Changes", &text, "Save", "Discard")?
            }
            SystemDialogBackend::Unavailable => return Err(DialogError::unavailable()),
        };
        Ok(match result {
            QuestionResult::Primary => DirtyCloseChoice::Save,
            QuestionResult::Secondary => DirtyCloseChoice::Discard,
            QuestionResult::Cancel => DirtyCloseChoice::Cancel,
        })
    }

    fn resolve_save_conflict(
        &mut self,
        title: &str,
        path: &Path,
    ) -> Result<SaveConflictChoice, DialogError> {
        let text = format!(
            "{} changed outside Luna. Overwrite it, reload storage content, or cancel?",
            path.display()
        );
        let result = match self.backend {
            SystemDialogBackend::Zenity => {
                self.zenity_question(title, &text, "Overwrite", "Reload")?
            }
            SystemDialogBackend::KDialog => self.kdialog_question(title, &text)?,
            SystemDialogBackend::AppleScript => {
                self.applescript_question(title, &text, "Overwrite", "Reload")?
            }
            SystemDialogBackend::Unavailable => return Err(DialogError::unavailable()),
        };
        Ok(match result {
            QuestionResult::Primary => SaveConflictChoice::Overwrite,
            QuestionResult::Secondary => SaveConflictChoice::Reload,
            QuestionResult::Cancel => SaveConflictChoice::Cancel,
        })
    }

    fn prompt_workspace_name(
        &mut self,
        title: &str,
        prompt: &str,
        initial_name: &str,
    ) -> Result<Option<String>, DialogError> {
        let output = match self.backend {
            SystemDialogBackend::Zenity => self.run(
                "zenity",
                &[
                    OsString::from("--entry"),
                    OsString::from(format!("--title={title}")),
                    OsString::from(format!("--text={prompt}")),
                    OsString::from(format!("--entry-text={initial_name}")),
                ],
            )?,
            SystemDialogBackend::KDialog => self.run(
                "kdialog",
                &[
                    OsString::from("--title"),
                    OsString::from(title),
                    OsString::from("--inputbox"),
                    OsString::from(prompt),
                    OsString::from(initial_name),
                ],
            )?,
            SystemDialogBackend::AppleScript => self.run_applescript(
                r#"on run argv
try
    set answer to display dialog (item 2 of argv) with title (item 1 of argv) default answer (item 3 of argv) buttons {"Cancel", "OK"} default button "OK" cancel button "Cancel"
    return text returned of answer
on error number -128
    return "__LUNA_CANCEL__"
end try
end run"#,
                &[
                    OsString::from(title),
                    OsString::from(prompt),
                    OsString::from(initial_name),
                ],
            )?,
            SystemDialogBackend::Unavailable => return Err(DialogError::unavailable()),
        };
        if output.status.success() {
            let value = String::from_utf8(output.stdout)
                .map_err(|error| DialogError::invalid_response(error.to_string()))?;
            let value = value
                .strip_suffix("\r\n")
                .or_else(|| value.strip_suffix('\n'))
                .unwrap_or(&value);
            if value == "__LUNA_CANCEL__" {
                Ok(None)
            } else {
                Ok(Some(value.to_owned()))
            }
        } else if is_cancel_status(output.status) {
            Ok(None)
        } else {
            Err(dialog_output_error(output))
        }
    }

    fn confirm_workspace_replace(
        &mut self,
        path: &Path,
    ) -> Result<WorkspaceCollisionChoice, DialogError> {
        let text = format!("Replace the existing file {}?", path.display());
        let result = match self.backend {
            SystemDialogBackend::Zenity => {
                self.zenity_question("Replace Existing File", &text, "Replace", "Cancel")?
            }
            SystemDialogBackend::KDialog => {
                self.kdialog_question("Replace Existing File", &text)?
            }
            SystemDialogBackend::AppleScript => {
                self.applescript_question("Replace Existing File", &text, "Replace", "Cancel")?
            }
            SystemDialogBackend::Unavailable => return Err(DialogError::unavailable()),
        };
        Ok(if result == QuestionResult::Primary {
            WorkspaceCollisionChoice::Replace
        } else {
            WorkspaceCollisionChoice::Cancel
        })
    }

    fn confirm_workspace_delete(
        &mut self,
        path: &Path,
        is_directory: bool,
    ) -> Result<WorkspaceDeleteChoice, DialogError> {
        let object = if is_directory {
            "directory tree"
        } else {
            "file"
        };
        let text = format!("Permanently delete this {object}?\n{}", path.display());
        let result = match self.backend {
            SystemDialogBackend::Zenity => {
                self.zenity_question("Delete Workspace Entry", &text, "Delete", "Cancel")?
            }
            SystemDialogBackend::KDialog => {
                self.kdialog_question("Delete Workspace Entry", &text)?
            }
            SystemDialogBackend::AppleScript => {
                self.applescript_question("Delete Workspace Entry", &text, "Delete", "Cancel")?
            }
            SystemDialogBackend::Unavailable => return Err(DialogError::unavailable()),
        };
        Ok(if result == QuestionResult::Primary {
            WorkspaceDeleteChoice::Delete
        } else {
            WorkspaceDeleteChoice::Cancel
        })
    }

    fn resolve_dirty_workspace_delete(
        &mut self,
        title: &str,
        path: &Path,
    ) -> Result<WorkspaceDirtyDeleteChoice, DialogError> {
        let text = format!(
            "{title} has unsaved changes and is being deleted from the workspace. Keep the buffer as an untitled document, discard and close it, or cancel?\n{}",
            path.display()
        );
        let result = match self.backend {
            SystemDialogBackend::Zenity => self.zenity_question(
                "Unsaved Workspace Document",
                &text,
                "Keep Open",
                "Discard & Close",
            )?,
            SystemDialogBackend::KDialog => {
                self.kdialog_question("Unsaved Workspace Document", &text)?
            }
            SystemDialogBackend::AppleScript => self.applescript_question(
                "Unsaved Workspace Document",
                &text,
                "Keep Open",
                "Discard & Close",
            )?,
            SystemDialogBackend::Unavailable => return Err(DialogError::unavailable()),
        };
        Ok(match result {
            QuestionResult::Primary => WorkspaceDirtyDeleteChoice::KeepOpen,
            QuestionResult::Secondary => WorkspaceDirtyDeleteChoice::DiscardAndClose,
            QuestionResult::Cancel => WorkspaceDirtyDeleteChoice::Cancel,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QuestionResult {
    Primary,
    Secondary,
    Cancel,
}

fn path_from_dialog_stdout(stdout: Vec<u8>) -> Result<Option<PathBuf>, DialogError> {
    let value = String::from_utf8(stdout)
        .map_err(|error| DialogError::invalid_response(error.to_string()))?;
    let selected = value
        .strip_suffix("\r\n")
        .or_else(|| value.strip_suffix('\n'))
        .unwrap_or(&value);
    if selected.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PathBuf::from(selected)))
    }
}

fn command_exists(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|directory| directory.join(program).is_file())
    })
}

fn is_cancel_status(status: ExitStatus) -> bool {
    matches!(status.code(), Some(1))
}

fn dialog_output_error(output: Output) -> DialogError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    DialogError::io(
        "native dialog returned an error",
        if stderr.trim().is_empty() {
            format!("exit status {}", output.status)
        } else {
            stderr.trim().to_owned()
        },
    )
}

/// In-memory text-file adapter for deterministic tests and product harnesses.
#[derive(Clone, Debug)]
pub struct MemoryTextFileService {
    root: PathBuf,
    files: Rc<RefCell<BTreeMap<PathBuf, MemoryFile>>>,
    next_instance: Rc<Cell<u128>>,
}

#[derive(Clone, Debug)]
struct MemoryFile {
    bytes: Vec<u8>,
    instance: StorageInstance,
}

impl MemoryTextFileService {
    /// Creates an empty adapter rooted at an absolute canonical path.
    ///
    /// # Errors
    ///
    /// Returns [`FileServiceError`] when `root` is relative or contains `.` or `..` components.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, FileServiceError> {
        let root = root.into();
        validate_memory_path(&root).map_err(|message| FileServiceError {
            operation: "create memory filesystem",
            path: root.clone(),
            kind: FileServiceErrorKind::InvalidPath,
            message,
            expected_revision: None,
            observed_revision: None,
        })?;
        Ok(Self {
            root,
            files: Rc::new(RefCell::new(BTreeMap::new())),
            next_instance: Rc::new(Cell::new(1)),
        })
    }

    fn allocate_instance(&self) -> StorageInstance {
        let value = self.next_instance.get();
        self.next_instance.set(value.saturating_add(1));
        StorageInstance::new(value)
    }

    fn replace_bytes(&self, path: PathBuf, bytes: Vec<u8>) {
        let entry = MemoryFile {
            bytes,
            instance: self.allocate_instance(),
        };
        let _ = self.files.borrow_mut().insert(path, entry);
    }

    /// Inserts or replaces UTF-8 content without applying a write precondition.
    ///
    /// # Errors
    ///
    /// Returns [`FileServiceError`] when the path cannot be normalized below the configured root.
    pub fn insert_utf8(&self, path: &Path, text: &str) -> Result<(), FileServiceError> {
        let path = self.resolve(path)?;
        self.replace_bytes(path, text.as_bytes().to_vec());
        Ok(())
    }

    /// Inserts arbitrary bytes, allowing invalid-UTF-8 tests.
    ///
    /// # Errors
    ///
    /// Returns [`FileServiceError`] when the path cannot be normalized below the configured root.
    pub fn insert_bytes(&self, path: &Path, bytes: Vec<u8>) -> Result<(), FileServiceError> {
        let path = self.resolve(path)?;
        self.replace_bytes(path, bytes);
        Ok(())
    }

    /// Modifies UTF-8 bytes while preserving the current storage-instance identity.
    ///
    /// This models an in-place external write rather than atomic replacement.
    pub fn modify_utf8_in_place(&self, path: &Path, text: &str) -> Result<(), FileServiceError> {
        let path = self.resolve(path)?;
        let mut files = self.files.borrow_mut();
        let file = files.get_mut(&path).ok_or_else(|| FileServiceError {
            operation: "modify memory file",
            path: path.clone(),
            kind: FileServiceErrorKind::NotFound,
            message: "memory file does not exist".to_owned(),
            expected_revision: None,
            observed_revision: None,
        })?;
        file.bytes = text.as_bytes().to_vec();
        Ok(())
    }

    /// Removes one file, returning whether it existed.
    pub fn remove_file(&self, path: &Path) -> Result<bool, FileServiceError> {
        let path = self.resolve(path)?;
        Ok(self.files.borrow_mut().remove(&path).is_some())
    }

    /// Returns a copy of current bytes for assertions.
    ///
    /// # Errors
    ///
    /// Returns [`FileServiceError`] when the path cannot be normalized below the configured root.
    pub fn bytes(&self, path: &Path) -> Result<Option<Vec<u8>>, FileServiceError> {
        let path = self.resolve(path)?;
        Ok(self
            .files
            .borrow()
            .get(&path)
            .map(|file| file.bytes.clone()))
    }

    fn resolve(&self, path: &Path) -> Result<PathBuf, FileServiceError> {
        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        validate_memory_path(&candidate).map_err(|message| FileServiceError {
            operation: "normalize memory path",
            path: candidate.clone(),
            kind: FileServiceErrorKind::InvalidPath,
            message,
            expected_revision: None,
            observed_revision: None,
        })?;
        if !candidate.starts_with(&self.root) {
            return Err(FileServiceError {
                operation: "normalize memory path",
                path: candidate,
                kind: FileServiceErrorKind::InvalidPath,
                message: "memory filesystem paths must stay below the configured root".to_owned(),
                expected_revision: None,
                observed_revision: None,
            });
        }
        Ok(candidate)
    }
}

impl TextFileService for MemoryTextFileService {
    fn load_utf8(&self, path: &Path) -> Result<LoadedTextFile, FileServiceError> {
        let path = self.resolve(path)?;
        let file = self
            .files
            .borrow()
            .get(&path)
            .cloned()
            .ok_or_else(|| FileServiceError {
                operation: "read memory file",
                path: path.clone(),
                kind: FileServiceErrorKind::NotFound,
                message: "memory file does not exist".to_owned(),
                expected_revision: None,
                observed_revision: None,
            })?;
        let snapshot = StorageSnapshot::new(content_revision(&file.bytes), file.instance);
        let text = String::from_utf8(file.bytes)
            .map_err(|error| FileServiceError::invalid_utf8(&path, error))?;
        let identity = FileIdentity::from_canonical_path(path.clone())
            .map_err(|error| FileServiceError::invalid_path(&path, error))?;
        Ok(LoadedTextFile {
            identity,
            text,
            snapshot,
        })
    }

    fn identity_for_save(&self, path: &Path) -> Result<FileIdentity, FileServiceError> {
        let path = self.resolve(path)?;
        FileIdentity::from_canonical_path(path.clone())
            .map_err(|error| FileServiceError::invalid_path(&path, error))
    }

    fn observe_file(&self, path: &Path) -> Result<FileObservation, FileServiceError> {
        let path = self.resolve(path)?;
        let files = self.files.borrow();
        Ok(match files.get(&path) {
            Some(file) => FileObservation::Present(StorageSnapshot::new(
                content_revision(&file.bytes),
                file.instance,
            )),
            None => FileObservation::Missing,
        })
    }

    fn write_utf8_atomic(
        &self,
        path: &Path,
        text: &str,
        precondition: WritePrecondition,
    ) -> Result<WrittenTextFile, FileServiceError> {
        let identity = self.identity_for_save(path)?;
        let observed = self
            .files
            .borrow()
            .get(identity.path())
            .map(|file| StorageSnapshot::new(content_revision(&file.bytes), file.instance));
        let matches = match precondition {
            WritePrecondition::Any => true,
            WritePrecondition::Missing => observed.is_none(),
            WritePrecondition::Matches(expected) => observed == Some(expected),
        };
        if !matches {
            let expected_revision = match precondition {
                WritePrecondition::Matches(snapshot) => Some(snapshot.revision()),
                WritePrecondition::Any | WritePrecondition::Missing => None,
            };
            return Err(FileServiceError::conflict(
                identity.path(),
                expected_revision,
                observed.map(StorageSnapshot::revision),
            ));
        }
        let instance = self.allocate_instance();
        let snapshot = StorageSnapshot::new(content_revision(text.as_bytes()), instance);
        let _ = self.files.borrow_mut().insert(
            identity.path().to_path_buf(),
            MemoryFile {
                bytes: text.as_bytes().to_vec(),
                instance,
            },
        );
        Ok(WrittenTextFile { identity, snapshot })
    }
}

fn validate_memory_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("memory filesystem paths must be absolute".to_owned());
    }
    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err("memory filesystem paths cannot contain . or .. components".to_owned());
    }
    Ok(())
}

/// Deterministic queued dialog adapter for tests.
#[derive(Clone, Debug, Default)]
pub struct ScriptedDialogService {
    state: Rc<RefCell<ScriptedDialogState>>,
}

#[derive(Clone, Debug, Default)]
struct ScriptedDialogState {
    open_files: VecDeque<Option<PathBuf>>,
    open_folders: VecDeque<Option<PathBuf>>,
    save_files: VecDeque<Option<PathBuf>>,
    close_choices: VecDeque<DirtyCloseChoice>,
    conflict_choices: VecDeque<SaveConflictChoice>,
    workspace_names: VecDeque<Option<String>>,
    workspace_collisions: VecDeque<WorkspaceCollisionChoice>,
    workspace_deletes: VecDeque<WorkspaceDeleteChoice>,
    workspace_dirty_deletes: VecDeque<WorkspaceDirtyDeleteChoice>,
}

impl ScriptedDialogService {
    /// Appends one open-dialog result.
    pub fn push_open_file(&self, path: Option<PathBuf>) {
        self.state.borrow_mut().open_files.push_back(path);
    }

    /// Appends one workspace-folder dialog result.
    pub fn push_open_folder(&self, path: Option<PathBuf>) {
        self.state.borrow_mut().open_folders.push_back(path);
    }

    /// Appends one Save As dialog result.
    pub fn push_save_file(&self, path: Option<PathBuf>) {
        self.state.borrow_mut().save_files.push_back(path);
    }

    /// Appends one dirty-close decision.
    pub fn push_dirty_close(&self, choice: DirtyCloseChoice) {
        self.state.borrow_mut().close_choices.push_back(choice);
    }

    /// Appends one save-conflict decision.
    pub fn push_save_conflict(&self, choice: SaveConflictChoice) {
        self.state.borrow_mut().conflict_choices.push_back(choice);
    }

    /// Appends one workspace-name prompt result.
    pub fn push_workspace_name(&self, name: Option<String>) {
        self.state.borrow_mut().workspace_names.push_back(name);
    }

    /// Appends one workspace-collision decision.
    pub fn push_workspace_collision(&self, choice: WorkspaceCollisionChoice) {
        self.state
            .borrow_mut()
            .workspace_collisions
            .push_back(choice);
    }

    /// Appends one workspace-delete confirmation.
    pub fn push_workspace_delete(&self, choice: WorkspaceDeleteChoice) {
        self.state.borrow_mut().workspace_deletes.push_back(choice);
    }

    /// Appends one dirty-workspace-document deletion decision.
    pub fn push_workspace_dirty_delete(&self, choice: WorkspaceDirtyDeleteChoice) {
        self.state
            .borrow_mut()
            .workspace_dirty_deletes
            .push_back(choice);
    }
}

impl DocumentDialogService for ScriptedDialogService {
    fn choose_open_file(&mut self) -> Result<Option<PathBuf>, DialogError> {
        Ok(self.state.borrow_mut().open_files.pop_front().flatten())
    }

    fn choose_open_folder(&mut self) -> Result<Option<PathBuf>, DialogError> {
        Ok(self.state.borrow_mut().open_folders.pop_front().flatten())
    }

    fn choose_save_file(
        &mut self,
        _suggested_name: &str,
        _current_path: Option<&Path>,
    ) -> Result<Option<PathBuf>, DialogError> {
        Ok(self.state.borrow_mut().save_files.pop_front().flatten())
    }

    fn confirm_dirty_close(&mut self, _title: &str) -> Result<DirtyCloseChoice, DialogError> {
        Ok(self
            .state
            .borrow_mut()
            .close_choices
            .pop_front()
            .unwrap_or(DirtyCloseChoice::Cancel))
    }

    fn resolve_save_conflict(
        &mut self,
        _title: &str,
        _path: &Path,
    ) -> Result<SaveConflictChoice, DialogError> {
        Ok(self
            .state
            .borrow_mut()
            .conflict_choices
            .pop_front()
            .unwrap_or(SaveConflictChoice::Cancel))
    }

    fn prompt_workspace_name(
        &mut self,
        _title: &str,
        _prompt: &str,
        _initial_name: &str,
    ) -> Result<Option<String>, DialogError> {
        Ok(self
            .state
            .borrow_mut()
            .workspace_names
            .pop_front()
            .flatten())
    }

    fn confirm_workspace_replace(
        &mut self,
        _path: &Path,
    ) -> Result<WorkspaceCollisionChoice, DialogError> {
        Ok(self
            .state
            .borrow_mut()
            .workspace_collisions
            .pop_front()
            .unwrap_or(WorkspaceCollisionChoice::Cancel))
    }

    fn confirm_workspace_delete(
        &mut self,
        _path: &Path,
        _is_directory: bool,
    ) -> Result<WorkspaceDeleteChoice, DialogError> {
        Ok(self
            .state
            .borrow_mut()
            .workspace_deletes
            .pop_front()
            .unwrap_or(WorkspaceDeleteChoice::Cancel))
    }

    fn resolve_dirty_workspace_delete(
        &mut self,
        _title: &str,
        _path: &Path,
    ) -> Result<WorkspaceDirtyDeleteChoice, DialogError> {
        Ok(self
            .state
            .borrow_mut()
            .workspace_dirty_deletes
            .pop_front()
            .unwrap_or(WorkspaceDirtyDeleteChoice::Cancel))
    }
}

fn content_revision(bytes: &[u8]) -> StorageRevision {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    StorageRevision::new(hash)
}

#[cfg(test)]
mod tests {
    use super::{
        DirtyCloseChoice, DocumentDialogService, FileObservation, FileServiceErrorKind,
        MemoryTextFileService, SaveConflictChoice, ScriptedDialogService, StdTextFileService,
        TEMP_FILE_SEQUENCE, TextFileService, WorkspaceCollisionChoice, WorkspaceDeleteChoice,
        WorkspaceDirtyDeleteChoice, WritePrecondition, path_from_dialog_stdout,
    };
    use std::error::Error;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::Ordering;

    fn memory() -> Result<MemoryTextFileService, Box<dyn Error>> {
        Ok(MemoryTextFileService::new(PathBuf::from(
            "/luna-memory-tests",
        ))?)
    }

    fn temporary_directory(label: &str) -> Result<PathBuf, Box<dyn Error>> {
        loop {
            let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "luna-document-services-{}-{sequence}-{label}",
                std::process::id()
            ));
            match fs::create_dir(&directory) {
                Ok(()) => return Ok(directory),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(Box::new(error)),
            }
        }
    }

    #[test]
    fn dialog_path_parsing_preserves_filename_spaces() -> Result<(), Box<dyn Error>> {
        let selected = path_from_dialog_stdout(b"/tmp/ spaced name \n".to_vec())?;

        assert_eq!(selected, Some(PathBuf::from("/tmp/ spaced name ")));
        Ok(())
    }

    #[test]
    fn memory_service_loads_utf8_with_stable_identity() -> Result<(), Box<dyn Error>> {
        let service = memory()?;
        service.insert_utf8(Path::new("notes.txt"), "hello")?;
        let loaded = service.load_utf8(Path::new("notes.txt"))?;

        assert_eq!(loaded.text(), "hello");
        assert_eq!(
            loaded.identity().path(),
            Path::new("/luna-memory-tests/notes.txt")
        );
        Ok(())
    }

    #[test]
    fn memory_service_rejects_absolute_paths_outside_its_root() -> Result<(), Box<dyn Error>> {
        let service = memory()?;
        let result = service.insert_utf8(Path::new("/another-root/notes.txt"), "hello");

        assert!(result.is_err_and(|error| error.kind() == FileServiceErrorKind::InvalidPath));
        Ok(())
    }

    #[test]
    fn invalid_utf8_is_reported_without_lossy_decoding() -> Result<(), Box<dyn Error>> {
        let service = memory()?;
        service.insert_bytes(Path::new("invalid.txt"), vec![0xff, 0xfe])?;
        let error = service.load_utf8(Path::new("invalid.txt"));

        assert!(error.is_err_and(|value| value.kind() == FileServiceErrorKind::InvalidUtf8));
        Ok(())
    }

    #[test]
    fn standard_service_round_trips_utf8_with_revision_checks() -> Result<(), Box<dyn Error>> {
        let directory = temporary_directory("round-trip")?;
        let path = directory.join("notes.txt");
        fs::write(&path, "before")?;
        let service = StdTextFileService;
        let loaded = service.load_utf8(&path)?;
        let written = service.write_utf8_atomic(
            &path,
            "after",
            WritePrecondition::Matches(loaded.snapshot()),
        )?;

        assert_eq!(fs::read_to_string(&path)?, "after");
        assert_ne!(written.revision(), loaded.revision());
        fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn standard_service_preserves_existing_unix_permissions() -> Result<(), Box<dyn Error>> {
        use std::os::unix::fs::PermissionsExt;

        let directory = temporary_directory("permissions")?;
        let path = directory.join("script.txt");
        fs::write(&path, "before")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o744))?;
        let service = StdTextFileService;
        let loaded = service.load_utf8(&path)?;
        let _ = service.write_utf8_atomic(
            &path,
            "after",
            WritePrecondition::Matches(loaded.snapshot()),
        )?;

        let mode = fs::metadata(&path)?.permissions().mode() & 0o777;
        assert_eq!(mode, 0o744);
        fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn matching_revision_allows_atomic_write() -> Result<(), Box<dyn Error>> {
        let service = memory()?;
        service.insert_utf8(Path::new("save.txt"), "before")?;
        let loaded = service.load_utf8(Path::new("save.txt"))?;
        let written = service.write_utf8_atomic(
            Path::new("save.txt"),
            "after",
            WritePrecondition::Matches(loaded.snapshot()),
        )?;

        assert_eq!(
            service.bytes(Path::new("save.txt"))?,
            Some(b"after".to_vec())
        );
        assert_ne!(written.revision(), loaded.revision());
        Ok(())
    }

    #[test]
    fn stale_revision_is_rejected_as_conflict() -> Result<(), Box<dyn Error>> {
        let service = memory()?;
        service.insert_utf8(Path::new("conflict.txt"), "baseline")?;
        let baseline = service.load_utf8(Path::new("conflict.txt"))?;
        service.insert_utf8(Path::new("conflict.txt"), "external")?;
        let error = service.write_utf8_atomic(
            Path::new("conflict.txt"),
            "editor",
            WritePrecondition::Matches(baseline.snapshot()),
        );

        assert!(error.is_err_and(|value| value.kind() == FileServiceErrorKind::Conflict));
        Ok(())
    }

    #[test]
    fn same_content_replacement_is_rejected_by_snapshot_precondition() -> Result<(), Box<dyn Error>>
    {
        let service = memory()?;
        let path = Path::new("same-content.txt");
        service.insert_utf8(path, "same")?;
        let baseline = service.load_utf8(path)?;
        service.insert_utf8(path, "same")?;

        let error = service.write_utf8_atomic(
            path,
            "editor",
            WritePrecondition::Matches(baseline.snapshot()),
        );

        assert!(error.is_err_and(|value| value.kind() == FileServiceErrorKind::Conflict));
        Ok(())
    }

    #[test]
    fn missing_precondition_prevents_accidental_overwrite() -> Result<(), Box<dyn Error>> {
        let service = memory()?;
        service.insert_utf8(Path::new("existing.txt"), "existing")?;
        let error = service.write_utf8_atomic(
            Path::new("existing.txt"),
            "replacement",
            WritePrecondition::Missing,
        );

        assert!(error.is_err_and(|value| value.kind() == FileServiceErrorKind::Conflict));
        Ok(())
    }

    #[test]
    fn observation_distinguishes_in_place_change_replacement_and_missing()
    -> Result<(), Box<dyn Error>> {
        let service = memory()?;
        let path = Path::new("observe.txt");
        service.insert_utf8(path, "baseline")?;
        let FileObservation::Present(initial) = service.observe_file(path)? else {
            return Err(std::io::Error::other("initial observation was missing").into());
        };

        service.modify_utf8_in_place(path, "modified")?;
        let FileObservation::Present(modified) = service.observe_file(path)? else {
            return Err(std::io::Error::other("modified observation was missing").into());
        };
        assert_eq!(initial.instance(), modified.instance());
        assert_ne!(initial.revision(), modified.revision());

        service.insert_utf8(path, "replacement")?;
        let FileObservation::Present(replaced) = service.observe_file(path)? else {
            return Err(std::io::Error::other("replacement observation was missing").into());
        };
        assert_ne!(modified.instance(), replaced.instance());

        assert!(service.remove_file(path)?);
        assert_eq!(service.observe_file(path)?, FileObservation::Missing);
        Ok(())
    }

    #[test]
    fn scripted_dialogs_return_queued_choices() -> Result<(), Box<dyn Error>> {
        let scripted = ScriptedDialogService::default();
        scripted.push_open_file(Some(PathBuf::from("/tmp/open.txt")));
        scripted.push_open_folder(Some(PathBuf::from("/tmp/workspace")));
        scripted.push_save_file(Some(PathBuf::from("/tmp/save.txt")));
        scripted.push_dirty_close(DirtyCloseChoice::Discard);
        scripted.push_save_conflict(SaveConflictChoice::Reload);
        scripted.push_workspace_name(Some("renamed.txt".to_owned()));
        scripted.push_workspace_collision(WorkspaceCollisionChoice::Replace);
        scripted.push_workspace_delete(WorkspaceDeleteChoice::Delete);
        scripted.push_workspace_dirty_delete(WorkspaceDirtyDeleteChoice::KeepOpen);
        let mut dialogs = scripted;

        assert_eq!(
            dialogs.choose_open_file()?,
            Some(PathBuf::from("/tmp/open.txt"))
        );
        assert_eq!(
            dialogs.choose_open_folder()?,
            Some(PathBuf::from("/tmp/workspace"))
        );
        assert_eq!(
            dialogs.choose_save_file("save.txt", None)?,
            Some(PathBuf::from("/tmp/save.txt"))
        );
        assert_eq!(
            dialogs.confirm_dirty_close("save.txt")?,
            DirtyCloseChoice::Discard
        );
        assert_eq!(
            dialogs.resolve_save_conflict("save.txt", Path::new("/tmp/save.txt"))?,
            SaveConflictChoice::Reload
        );
        assert_eq!(
            dialogs.prompt_workspace_name("Rename", "Name", "old.txt")?,
            Some("renamed.txt".to_owned())
        );
        assert_eq!(
            dialogs.confirm_workspace_replace(Path::new("/tmp/renamed.txt"))?,
            WorkspaceCollisionChoice::Replace
        );
        assert_eq!(
            dialogs.confirm_workspace_delete(Path::new("/tmp/renamed.txt"), false)?,
            WorkspaceDeleteChoice::Delete
        );
        assert_eq!(
            dialogs.resolve_dirty_workspace_delete("renamed.txt", Path::new("/tmp/renamed.txt"))?,
            WorkspaceDirtyDeleteChoice::KeepOpen
        );
        Ok(())
    }
}
