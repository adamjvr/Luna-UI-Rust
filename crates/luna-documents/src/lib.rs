// SPDX-License-Identifier: MPL-2.0

//! Product-neutral document identity and lifecycle contracts.
//!
//! This crate deliberately does not read files, show dialogs, watch directories, or own editor
//! view state. Platform and product adapters provide canonical file identities and storage
//! revisions, while applications keep caret, selection, scroll, and rendering state above this
//! layer. The resulting model is deterministic and can be tested without touching the filesystem.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Component, Path, PathBuf};

/// Stable application-local identity for one open document.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DocumentId(u64);

impl DocumentId {
    /// Returns the underlying monotonically assigned value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Returns a stable string suitable for widget and cache keys.
    #[must_use]
    pub fn stable_key(self) -> String {
        format!("document-{}", self.0)
    }
}

impl Display for DocumentId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "document-{}", self.0)
    }
}

/// Canonical identity supplied by a filesystem adapter.
///
/// Luna requires an absolute path with no `.` or `..` components. The adapter remains responsible
/// for symlink resolution, filesystem case behavior, sandbox bookmarks, and any platform-specific
/// equivalence rules before constructing this value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FileIdentity {
    canonical_path: PathBuf,
}

impl FileIdentity {
    /// Creates an identity from an adapter-canonicalized absolute path.
    ///
    /// # Errors
    ///
    /// Returns [`DocumentError::NonCanonicalPath`] when the path is relative or contains current-
    /// directory or parent-directory components.
    pub fn from_canonical_path(path: impl Into<PathBuf>) -> Result<Self, DocumentError> {
        let canonical_path = path.into();
        if !canonical_path.is_absolute()
            || canonical_path
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(DocumentError::NonCanonicalPath(canonical_path));
        }
        Ok(Self { canonical_path })
    }

    /// Returns the canonical path used for duplicate-open prevention.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.canonical_path
    }
}

/// Opaque revision supplied by a storage adapter after reading, writing, or observing a file.
///
/// The adapter may derive this from a content digest, file identifier plus metadata, or another
/// deterministic token. Luna only compares values for equality and never interprets them.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StorageRevision(u64);

impl StorageRevision {
    /// Creates an opaque storage revision.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the adapter-provided value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Durable source associated with an open document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentSource {
    /// A new document that has not yet been assigned a file.
    Untitled {
        /// Monotonic user-visible sequence number.
        sequence: u32,
    },
    /// A document associated with a canonical file identity.
    File(FileIdentity),
    /// Read-only or generated application content that has no filesystem identity.
    Virtual {
        /// Stable application-owned key used to prevent duplicate virtual documents.
        key: String,
    },
}

/// State observed outside the editor after the last successful load or save.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ExternalState {
    /// Storage still matches the editor's known baseline.
    #[default]
    InSync,
    /// Storage changed to a different observed revision.
    Modified {
        /// Revision observed by the storage adapter.
        observed_revision: StorageRevision,
    },
    /// The file associated with the document no longer exists.
    Missing,
}

/// Result of asking whether a document may close without user intervention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseRequirement {
    /// The current document revision matches its saved baseline.
    Safe,
    /// The document has unsaved edits and requires a save/discard/cancel decision.
    SaveOrDiscard,
}

/// Work required when the user invokes Save.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SaveRequirement {
    /// The document already matches its saved revision.
    None,
    /// The document needs a destination selected before it can be written.
    SaveAs,
    /// The document can write to its existing file identity.
    WriteFile {
        /// Canonical destination identity.
        identity: FileIdentity,
        /// Storage revision expected by an optimistic-conflict check.
        expected_storage_revision: Option<StorageRevision>,
        /// External state that may require overwrite/reload/cancel policy.
        external_state: ExternalState,
    },
    /// Virtual documents are not writable through the ordinary file lifecycle.
    Unsupported,
}

/// One product-neutral open-document lifecycle record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentRecord {
    id: DocumentId,
    source: DocumentSource,
    title: String,
    saved_edit_revision: u64,
    storage_revision: Option<StorageRevision>,
    external_state: ExternalState,
}

impl DocumentRecord {
    /// Returns this document's stable identity.
    #[must_use]
    pub const fn id(&self) -> DocumentId {
        self.id
    }

    /// Returns the document source.
    #[must_use]
    pub const fn source(&self) -> &DocumentSource {
        &self.source
    }

    /// Returns the user-visible title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the edit revision recorded by the last successful load or save.
    #[must_use]
    pub const fn saved_edit_revision(&self) -> u64 {
        self.saved_edit_revision
    }

    /// Returns the adapter-provided storage revision known at the last load or save.
    #[must_use]
    pub const fn storage_revision(&self) -> Option<StorageRevision> {
        self.storage_revision
    }

    /// Returns the currently observed external state.
    #[must_use]
    pub const fn external_state(&self) -> ExternalState {
        self.external_state
    }

    /// Returns whether `current_edit_revision` differs from the saved baseline.
    #[must_use]
    pub const fn is_dirty(&self, current_edit_revision: u64) -> bool {
        current_edit_revision != self.saved_edit_revision
    }

    /// Returns the close decision required for the current editor revision.
    #[must_use]
    pub const fn close_requirement(&self, current_edit_revision: u64) -> CloseRequirement {
        if self.is_dirty(current_edit_revision) {
            CloseRequirement::SaveOrDiscard
        } else {
            CloseRequirement::Safe
        }
    }

    /// Returns the storage operation required for Save.
    #[must_use]
    pub fn save_requirement(&self, current_edit_revision: u64) -> SaveRequirement {
        match &self.source {
            DocumentSource::Untitled { .. } => SaveRequirement::SaveAs,
            DocumentSource::Virtual { .. } => SaveRequirement::Unsupported,
            DocumentSource::File(identity) if self.is_dirty(current_edit_revision) => {
                SaveRequirement::WriteFile {
                    identity: identity.clone(),
                    expected_storage_revision: self.storage_revision,
                    external_state: self.external_state,
                }
            }
            DocumentSource::File(_) => SaveRequirement::None,
        }
    }

    /// Records a successful save to the current source.
    pub fn mark_saved(
        &mut self,
        current_edit_revision: u64,
        storage_revision: Option<StorageRevision>,
    ) {
        self.saved_edit_revision = current_edit_revision;
        self.storage_revision = storage_revision;
        self.external_state = ExternalState::InSync;
    }

    /// Records a storage revision observed after the last load or save.
    pub fn observe_storage_revision(&mut self, observed_revision: StorageRevision) {
        self.external_state = if self.storage_revision == Some(observed_revision) {
            ExternalState::InSync
        } else {
            ExternalState::Modified { observed_revision }
        };
    }

    /// Records that the associated file is missing.
    pub fn observe_missing_file(&mut self) {
        self.external_state = ExternalState::Missing;
    }
}

/// Outcome of registering a file that may already be open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenFileOutcome {
    /// A new document record was inserted.
    Opened(DocumentId),
    /// The canonical file identity was already present.
    AlreadyOpen(DocumentId),
}

/// Deterministic registry for open document identities and lifecycle records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentRegistry {
    next_id: u64,
    next_untitled_sequence: u32,
    records: Vec<DocumentRecord>,
    file_index: BTreeMap<FileIdentity, DocumentId>,
    virtual_index: BTreeMap<String, DocumentId>,
}

impl Default for DocumentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentRegistry {
    /// Creates an empty registry with IDs and untitled names starting at one.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: 1,
            next_untitled_sequence: 1,
            records: Vec::new(),
            file_index: BTreeMap::new(),
            virtual_index: BTreeMap::new(),
        }
    }

    /// Returns all records in insertion order.
    #[must_use]
    pub fn records(&self) -> &[DocumentRecord] {
        &self.records
    }

    /// Returns a record by identity.
    #[must_use]
    pub fn get(&self, id: DocumentId) -> Option<&DocumentRecord> {
        self.records.iter().find(|record| record.id == id)
    }

    /// Returns a mutable record by identity.
    #[must_use]
    pub fn get_mut(&mut self, id: DocumentId) -> Option<&mut DocumentRecord> {
        self.records.iter_mut().find(|record| record.id == id)
    }

    /// Creates a clean untitled document and reserves a monotonic title sequence.
    pub fn create_untitled(&mut self, initial_edit_revision: u64) -> DocumentId {
        let id = self.allocate_id();
        let sequence = self.next_untitled_sequence;
        self.next_untitled_sequence = self.next_untitled_sequence.saturating_add(1);
        self.records.push(DocumentRecord {
            id,
            source: DocumentSource::Untitled { sequence },
            title: format!("Untitled-{sequence}"),
            saved_edit_revision: initial_edit_revision,
            storage_revision: None,
            external_state: ExternalState::InSync,
        });
        id
    }

    /// Registers application-owned virtual content.
    ///
    /// # Errors
    ///
    /// Returns [`DocumentError::EmptyVirtualKey`] for an empty key.
    pub fn register_virtual(
        &mut self,
        key: impl Into<String>,
        title: impl Into<String>,
        initial_edit_revision: u64,
    ) -> Result<DocumentId, DocumentError> {
        let key = key.into();
        if key.is_empty() {
            return Err(DocumentError::EmptyVirtualKey);
        }
        if let Some(existing) = self.virtual_index.get(&key) {
            return Ok(*existing);
        }
        let id = self.allocate_id();
        self.virtual_index.insert(key.clone(), id);
        self.records.push(DocumentRecord {
            id,
            source: DocumentSource::Virtual { key },
            title: title.into(),
            saved_edit_revision: initial_edit_revision,
            storage_revision: None,
            external_state: ExternalState::InSync,
        });
        Ok(id)
    }

    /// Registers an opened file or returns the existing document with the same canonical identity.
    pub fn register_file(
        &mut self,
        identity: FileIdentity,
        title: impl Into<String>,
        initial_edit_revision: u64,
        storage_revision: Option<StorageRevision>,
    ) -> OpenFileOutcome {
        if let Some(existing) = self.file_index.get(&identity) {
            return OpenFileOutcome::AlreadyOpen(*existing);
        }
        let id = self.allocate_id();
        self.file_index.insert(identity.clone(), id);
        self.records.push(DocumentRecord {
            id,
            source: DocumentSource::File(identity),
            title: title.into(),
            saved_edit_revision: initial_edit_revision,
            storage_revision,
            external_state: ExternalState::InSync,
        });
        OpenFileOutcome::Opened(id)
    }

    /// Assigns a canonical file identity after Save As.
    ///
    /// # Errors
    ///
    /// Returns [`DocumentError::UnknownDocument`] when `id` is absent, or
    /// [`DocumentError::FileAlreadyOpen`] when another document owns the requested identity.
    pub fn assign_file(
        &mut self,
        id: DocumentId,
        identity: FileIdentity,
        title: impl Into<String>,
        current_edit_revision: u64,
        storage_revision: Option<StorageRevision>,
    ) -> Result<(), DocumentError> {
        let index = self
            .record_index(id)
            .ok_or(DocumentError::UnknownDocument(id))?;
        if let Some(existing) = self.file_index.get(&identity)
            && *existing != id
        {
            return Err(DocumentError::FileAlreadyOpen {
                identity,
                existing: *existing,
            });
        }
        self.remove_source_index(index);
        self.file_index.insert(identity.clone(), id);
        let record = &mut self.records[index];
        record.source = DocumentSource::File(identity);
        record.title = title.into();
        record.mark_saved(current_edit_revision, storage_revision);
        Ok(())
    }

    /// Removes a document and releases its file or virtual identity.
    pub fn remove(&mut self, id: DocumentId) -> Option<DocumentRecord> {
        let index = self.record_index(id)?;
        self.remove_source_index(index);
        Some(self.records.remove(index))
    }

    /// Returns the document already associated with a canonical file identity.
    #[must_use]
    pub fn document_for_file(&self, identity: &FileIdentity) -> Option<DocumentId> {
        self.file_index.get(identity).copied()
    }

    fn allocate_id(&mut self) -> DocumentId {
        let id = DocumentId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    fn record_index(&self, id: DocumentId) -> Option<usize> {
        self.records.iter().position(|record| record.id == id)
    }

    fn remove_source_index(&mut self, index: usize) {
        match &self.records[index].source {
            DocumentSource::File(identity) => {
                self.file_index.remove(identity);
            }
            DocumentSource::Virtual { key } => {
                self.virtual_index.remove(key);
            }
            DocumentSource::Untitled { .. } => {}
        }
    }
}

/// Errors produced by document identity and lifecycle operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentError {
    /// A filesystem adapter supplied a path that is not canonical enough for identity comparison.
    NonCanonicalPath(PathBuf),
    /// A virtual document key was empty.
    EmptyVirtualKey,
    /// A requested document identity was not registered.
    UnknownDocument(DocumentId),
    /// Another open document already owns a requested file identity.
    FileAlreadyOpen {
        /// Conflicting canonical identity.
        identity: FileIdentity,
        /// Existing document that owns the identity.
        existing: DocumentId,
    },
}

impl Display for DocumentError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonCanonicalPath(path) => write!(
                formatter,
                "document file identity must be an absolute canonical path: {}",
                path.display()
            ),
            Self::EmptyVirtualKey => formatter.write_str("virtual document key cannot be empty"),
            Self::UnknownDocument(id) => write!(formatter, "unknown document: {id}"),
            Self::FileAlreadyOpen { identity, existing } => write!(
                formatter,
                "file is already open as {existing}: {}",
                identity.path().display()
            ),
        }
    }
}

impl Error for DocumentError {}

#[cfg(test)]
mod tests {
    use super::{
        CloseRequirement, DocumentError, DocumentRegistry, DocumentSource, ExternalState,
        FileIdentity, OpenFileOutcome, SaveRequirement, StorageRevision,
    };
    use std::error::Error;
    use std::path::PathBuf;

    fn canonical(name: &str) -> Result<FileIdentity, DocumentError> {
        let path = std::env::temp_dir().join("luna-document-tests").join(name);
        FileIdentity::from_canonical_path(path)
    }

    #[test]
    fn untitled_sequences_are_monotonic_and_not_reused() {
        let mut registry = DocumentRegistry::new();
        let first = registry.create_untitled(0);
        let second = registry.create_untitled(0);
        let _ = registry.remove(first);
        let third = registry.create_untitled(0);

        assert_eq!(
            registry.get(second).map(|record| record.title()),
            Some("Untitled-2")
        );
        assert_eq!(
            registry.get(third).map(|record| record.title()),
            Some("Untitled-3")
        );
    }

    #[test]
    fn canonical_file_identity_rejects_relative_and_parent_components() {
        assert!(FileIdentity::from_canonical_path("relative.txt").is_err());
        assert!(FileIdentity::from_canonical_path(PathBuf::from("/tmp/../file.txt")).is_err());
    }

    #[test]
    fn duplicate_file_open_returns_existing_document() -> Result<(), Box<dyn Error>> {
        let mut registry = DocumentRegistry::new();
        let identity = canonical("duplicate.txt")?;
        let opened = registry.register_file(identity.clone(), "duplicate.txt", 0, None);
        let duplicate = registry.register_file(identity, "duplicate.txt", 0, None);

        let OpenFileOutcome::Opened(opened_id) = opened else {
            return Err(std::io::Error::other("first file registration was not opened").into());
        };
        assert_eq!(duplicate, OpenFileOutcome::AlreadyOpen(opened_id));
        assert_eq!(registry.records().len(), 1);
        Ok(())
    }

    #[test]
    fn dirty_close_requires_an_explicit_decision() {
        let mut registry = DocumentRegistry::new();
        let id = registry.create_untitled(4);
        let record = registry.get(id);

        assert_eq!(
            record.map(|value| value.close_requirement(4)),
            Some(CloseRequirement::Safe)
        );
        assert_eq!(
            record.map(|value| value.close_requirement(5)),
            Some(CloseRequirement::SaveOrDiscard)
        );
    }

    #[test]
    fn untitled_dirty_document_requires_save_as() {
        let mut registry = DocumentRegistry::new();
        let id = registry.create_untitled(0);
        assert_eq!(
            registry.get(id).map(|record| record.save_requirement(1)),
            Some(SaveRequirement::SaveAs)
        );
    }

    #[test]
    fn clean_untitled_document_still_requires_save_as() {
        let mut registry = DocumentRegistry::new();
        let id = registry.create_untitled(7);
        assert_eq!(
            registry.get(id).map(|record| record.save_requirement(7)),
            Some(SaveRequirement::SaveAs)
        );
    }

    #[test]
    fn clean_file_requires_no_save() -> Result<(), Box<dyn Error>> {
        let mut registry = DocumentRegistry::new();
        let outcome = registry.register_file(canonical("clean.txt")?, "clean.txt", 7, None);
        let OpenFileOutcome::Opened(id) = outcome else {
            return Err(std::io::Error::other("file registration did not open").into());
        };
        assert_eq!(
            registry.get(id).map(|record| record.save_requirement(7)),
            Some(SaveRequirement::None)
        );
        Ok(())
    }

    #[test]
    fn file_save_requirement_preserves_expected_revision() -> Result<(), Box<dyn Error>> {
        let mut registry = DocumentRegistry::new();
        let identity = canonical("save.txt")?;
        let outcome = registry.register_file(
            identity.clone(),
            "save.txt",
            2,
            Some(StorageRevision::new(19)),
        );
        let OpenFileOutcome::Opened(id) = outcome else {
            return Err(std::io::Error::other("file registration did not open").into());
        };

        assert_eq!(
            registry.get(id).map(|record| record.save_requirement(3)),
            Some(SaveRequirement::WriteFile {
                identity,
                expected_storage_revision: Some(StorageRevision::new(19)),
                external_state: ExternalState::InSync,
            })
        );
        Ok(())
    }

    #[test]
    fn successful_save_resets_dirty_and_external_state() -> Result<(), Box<dyn Error>> {
        let mut registry = DocumentRegistry::new();
        let identity = canonical("saved.txt")?;
        let outcome =
            registry.register_file(identity, "saved.txt", 1, Some(StorageRevision::new(2)));
        let OpenFileOutcome::Opened(id) = outcome else {
            return Err(std::io::Error::other("file registration did not open").into());
        };
        let record = registry
            .get_mut(id)
            .ok_or_else(|| std::io::Error::other("missing opened record"))?;
        record.observe_storage_revision(StorageRevision::new(3));
        record.mark_saved(4, Some(StorageRevision::new(4)));

        assert!(!record.is_dirty(4));
        assert_eq!(record.external_state(), ExternalState::InSync);
        assert_eq!(record.storage_revision(), Some(StorageRevision::new(4)));
        Ok(())
    }

    #[test]
    fn external_revision_change_is_exposed_as_save_conflict() -> Result<(), Box<dyn Error>> {
        let mut registry = DocumentRegistry::new();
        let identity = canonical("external.txt")?;
        let outcome = registry.register_file(
            identity.clone(),
            "external.txt",
            1,
            Some(StorageRevision::new(10)),
        );
        let OpenFileOutcome::Opened(id) = outcome else {
            return Err(std::io::Error::other("file registration did not open").into());
        };
        let record = registry
            .get_mut(id)
            .ok_or_else(|| std::io::Error::other("missing opened record"))?;
        record.observe_storage_revision(StorageRevision::new(11));

        assert_eq!(
            record.save_requirement(2),
            SaveRequirement::WriteFile {
                identity,
                expected_storage_revision: Some(StorageRevision::new(10)),
                external_state: ExternalState::Modified {
                    observed_revision: StorageRevision::new(11),
                },
            }
        );
        Ok(())
    }

    #[test]
    fn missing_file_is_exposed_as_save_conflict() -> Result<(), Box<dyn Error>> {
        let mut registry = DocumentRegistry::new();
        let identity = canonical("missing.txt")?;
        let outcome = registry.register_file(identity, "missing.txt", 0, None);
        let OpenFileOutcome::Opened(id) = outcome else {
            return Err(std::io::Error::other("file registration did not open").into());
        };
        let record = registry
            .get_mut(id)
            .ok_or_else(|| std::io::Error::other("missing opened record"))?;
        record.observe_missing_file();
        assert_eq!(record.external_state(), ExternalState::Missing);
        Ok(())
    }

    #[test]
    fn save_as_assigns_file_identity_and_updates_indexes() -> Result<(), Box<dyn Error>> {
        let mut registry = DocumentRegistry::new();
        let id = registry.create_untitled(0);
        let identity = canonical("assigned.txt")?;
        registry.assign_file(
            id,
            identity.clone(),
            "assigned.txt",
            5,
            Some(StorageRevision::new(6)),
        )?;

        assert_eq!(registry.document_for_file(&identity), Some(id));
        let record = registry
            .get(id)
            .ok_or_else(|| std::io::Error::other("missing assigned record"))?;
        assert_eq!(record.source(), &DocumentSource::File(identity));
        assert_eq!(record.title(), "assigned.txt");
        assert!(!record.is_dirty(5));
        Ok(())
    }

    #[test]
    fn save_as_rejects_identity_owned_by_another_document() -> Result<(), Box<dyn Error>> {
        let mut registry = DocumentRegistry::new();
        let identity = canonical("owned.txt")?;
        let outcome = registry.register_file(identity.clone(), "owned.txt", 0, None);
        let OpenFileOutcome::Opened(existing) = outcome else {
            return Err(std::io::Error::other("file registration did not open").into());
        };
        let untitled = registry.create_untitled(0);
        let error = registry.assign_file(untitled, identity, "owned.txt", 1, None);

        assert!(matches!(
            error,
            Err(DocumentError::FileAlreadyOpen {
                existing: owner,
                ..
            }) if owner == existing
        ));
        assert!(
            registry
                .get(untitled)
                .is_some_and(|record| matches!(record.source(), DocumentSource::Untitled { .. }))
        );
        Ok(())
    }

    #[test]
    fn removing_file_releases_duplicate_open_identity() -> Result<(), Box<dyn Error>> {
        let mut registry = DocumentRegistry::new();
        let identity = canonical("reopen.txt")?;
        let first = registry.register_file(identity.clone(), "reopen.txt", 0, None);
        let OpenFileOutcome::Opened(first_id) = first else {
            return Err(std::io::Error::other("file registration did not open").into());
        };
        let _ = registry.remove(first_id);
        assert!(matches!(
            registry.register_file(identity, "reopen.txt", 0, None),
            OpenFileOutcome::Opened(_)
        ));
        Ok(())
    }

    #[test]
    fn virtual_documents_are_deduplicated_by_application_key() -> Result<(), Box<dyn Error>> {
        let mut registry = DocumentRegistry::new();
        let first = registry.register_virtual("welcome", "Welcome", 0)?;
        let second = registry.register_virtual("welcome", "Different title", 0)?;

        assert_eq!(first, second);
        assert_eq!(registry.records().len(), 1);
        assert_eq!(
            registry.get(first).map(|record| record.title()),
            Some("Welcome")
        );
        Ok(())
    }
}
