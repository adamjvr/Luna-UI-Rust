// SPDX-License-Identifier: MPL-2.0

//! Product-neutral composition helpers for downstream Luna applications.
//!
//! Luna's service traits intentionally remain independent. This crate demonstrates how a consumer
//! can gather file, dialog, workspace, watcher, session, syntax, and completion adapters into one
//! application-owned value without making Luna responsible for product workflow or policy.

mod profile;

pub use profile::{DownstreamApplicationProfile, IntegrationNamespace, IntegrationProfileError};

use luna_core::{CodedError, ErrorCode};
use luna_host_core::{
    NativePlatform, PlatformSupportTier, current_native_platform, platform_support_tier,
};
use std::env;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Stable downstream application metadata used by diagnostics and packaging examples.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrationDescriptor {
    /// Reverse-DNS or otherwise stable application identifier.
    pub application_id: String,
    /// Human-readable application name.
    pub display_name: String,
}

impl IntegrationDescriptor {
    /// Creates validated integration metadata.
    ///
    /// # Errors
    ///
    /// Returns an error when either identifier is empty or only whitespace.
    pub fn new(
        application_id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Result<Self, IntegrationError> {
        let application_id = application_id.into();
        let display_name = display_name.into();
        if application_id.trim().is_empty() {
            return Err(IntegrationError::EmptyApplicationId);
        }
        if display_name.trim().is_empty() {
            return Err(IntegrationError::EmptyDisplayName);
        }
        Ok(Self {
            application_id,
            display_name,
        })
    }
}

/// Invalid downstream-integration metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrationError {
    /// The stable application identifier was empty.
    EmptyApplicationId,
    /// The human-readable application name was empty.
    EmptyDisplayName,
}

impl std::fmt::Display for IntegrationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::EmptyApplicationId => "application identifier cannot be empty",
            Self::EmptyDisplayName => "application display name cannot be empty",
        })
    }
}

impl std::error::Error for IntegrationError {}

impl CodedError for IntegrationError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self {
            Self::EmptyApplicationId => "integration.empty_application_id",
            Self::EmptyDisplayName => "integration.empty_display_name",
        })
    }
}

/// Product-neutral search path for packaged application resources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceLocator {
    application_id: String,
    roots: Vec<PathBuf>,
}

impl ResourceLocator {
    /// Creates an empty locator for one stable application identifier.
    ///
    /// # Errors
    ///
    /// Returns [`IntegrationError::EmptyApplicationId`] when the identifier is blank.
    pub fn new(application_id: impl Into<String>) -> Result<Self, IntegrationError> {
        let application_id = application_id.into();
        if application_id.trim().is_empty() {
            return Err(IntegrationError::EmptyApplicationId);
        }
        Ok(Self {
            application_id,
            roots: Vec::new(),
        })
    }

    /// Builds the standard development and packaged-resource search order.
    ///
    /// The optional `LUNA_RESOURCE_ROOT` environment variable is checked first. macOS bundle
    /// resources, executable-relative Linux `share` directories, repository `resources`, and the
    /// XDG data roots and the conventional `/usr/local/share` and `/usr/share` defaults follow.
    ///
    /// # Errors
    ///
    /// Returns an integration error for an empty application identifier.
    pub fn discover(application_id: impl Into<String>) -> Result<Self, IntegrationError> {
        let mut locator = Self::new(application_id)?;
        if let Some(root) = env::var_os("LUNA_RESOURCE_ROOT") {
            locator.push_root(root);
        }
        if let Ok(executable) = env::current_exe()
            && let Some(directory) = executable.parent()
        {
            if let Some(parent) = directory.parent() {
                locator.push_root(parent.join("Resources"));
                locator.push_root(parent.join("share").join(&locator.application_id));
            }
            locator.push_root(directory.join("resources"));
        }
        if let Ok(directory) = env::current_dir() {
            locator.push_root(directory.join("resources"));
        }
        if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
            locator.push_root(PathBuf::from(data_home).join(&locator.application_id));
        } else if let Some(home) = env::var_os("HOME") {
            locator.push_root(
                PathBuf::from(home)
                    .join(".local/share")
                    .join(&locator.application_id),
            );
        }
        if let Some(data_directories) = env::var_os("XDG_DATA_DIRS") {
            for directory in env::split_paths(&data_directories) {
                locator.push_root(directory.join(&locator.application_id));
            }
        } else {
            locator.push_root(Path::new("/usr/local/share").join(&locator.application_id));
            locator.push_root(Path::new("/usr/share").join(&locator.application_id));
        }
        Ok(locator)
    }

    /// Appends one search root if it is not already present.
    pub fn push_root(&mut self, root: impl Into<PathBuf>) {
        let root = root.into();
        if !self.roots.contains(&root) {
            self.roots.push(root);
        }
    }

    /// Returns the stable application identifier.
    #[must_use]
    pub fn application_id(&self) -> &str {
        &self.application_id
    }

    /// Returns search roots in priority order.
    #[must_use]
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Resolves one normalized relative resource path.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceError::InvalidRelativePath`] for absolute paths, empty paths, or paths
    /// containing `.` or `..`. Returns [`ResourceError::NotFound`] when no root contains the file.
    pub fn resolve(&self, relative: &Path) -> Result<PathBuf, ResourceError> {
        validate_resource_path(relative)?;
        for root in &self.roots {
            let candidate = root.join(relative);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
        Err(ResourceError::NotFound(relative.to_path_buf()))
    }

    /// Reads one packaged resource as strict UTF-8.
    ///
    /// # Errors
    ///
    /// Returns path-validation, not-found, I/O, or UTF-8 errors with stable machine codes.
    pub fn read_utf8(&self, relative: &Path) -> Result<String, ResourceError> {
        let path = self.resolve(relative)?;
        let bytes = fs::read(&path).map_err(|error| ResourceError::Io {
            path: path.clone(),
            message: error.to_string(),
        })?;
        String::from_utf8(bytes).map_err(|error| ResourceError::InvalidUtf8 {
            path,
            message: error.to_string(),
        })
    }
}

/// Failure while resolving or loading a packaged resource.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ResourceError {
    /// The requested path was not a normalized relative path.
    InvalidRelativePath(PathBuf),
    /// No configured root contained the requested resource.
    NotFound(PathBuf),
    /// Reading a resolved resource failed.
    Io {
        /// Resolved path.
        path: PathBuf,
        /// Operating-system diagnostic.
        message: String,
    },
    /// A resource expected to be text was not UTF-8.
    InvalidUtf8 {
        /// Resolved path.
        path: PathBuf,
        /// Decoder diagnostic.
        message: String,
    },
}

impl Display for ResourceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRelativePath(path) => {
                write!(
                    formatter,
                    "resource path must be normalized and relative: {}",
                    path.display()
                )
            }
            Self::NotFound(path) => write!(formatter, "resource was not found: {}", path.display()),
            Self::Io { path, message } => {
                write!(formatter, "read resource {}: {message}", path.display())
            }
            Self::InvalidUtf8 { path, message } => {
                write!(
                    formatter,
                    "decode resource {} as UTF-8: {message}",
                    path.display()
                )
            }
        }
    }
}

impl Error for ResourceError {}

impl CodedError for ResourceError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self {
            Self::InvalidRelativePath(_) => "resource.invalid_relative_path",
            Self::NotFound(_) => "resource.not_found",
            Self::Io { .. } => "resource.io",
            Self::InvalidUtf8 { .. } => "resource.invalid_utf8",
        })
    }
}

fn validate_resource_path(path: &Path) -> Result<(), ResourceError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(ResourceError::InvalidRelativePath(path.to_path_buf()));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::CurDir | Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(ResourceError::InvalidRelativePath(path.to_path_buf()));
    }
    Ok(())
}

/// Platform commitment included with downstream diagnostic reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntegrationPlatformReport {
    /// Platform family compiled into the application.
    pub platform: NativePlatform,
    /// Luna's documented support commitment for that platform.
    pub support_tier: PlatformSupportTier,
}

impl IntegrationPlatformReport {
    /// Reports the currently compiled native platform and support tier.
    #[must_use]
    pub const fn current() -> Self {
        let platform = current_native_platform();
        Self {
            platform,
            support_tier: platform_support_tier(platform),
        }
    }
}

/// Application-owned bundle of independent Luna adapters.
///
/// The generic parameters are intentionally unconstrained at storage time. A downstream product can
/// use concrete adapters, wrappers, mocks, asynchronous bridges, or trait objects while preserving
/// ownership and borrowing choices appropriate to that application.
#[derive(Debug)]
pub struct DownstreamServices<F, D, W, Watch, S, Syntax, Completion> {
    /// Stable integration metadata.
    pub descriptor: IntegrationDescriptor,
    /// UTF-8 file adapter.
    pub files: F,
    /// Native or scripted dialog adapter.
    pub dialogs: D,
    /// Workspace scan/mutation adapter.
    pub workspace: W,
    /// Workspace event-delivery adapter.
    pub watcher: Watch,
    /// Persistent session adapter.
    pub sessions: S,
    /// Application-selected syntax provider.
    pub syntax: Syntax,
    /// Application-selected completion provider.
    pub completion: Completion,
}

impl<F, D, W, Watch, S, Syntax, Completion>
    DownstreamServices<F, D, W, Watch, S, Syntax, Completion>
{
    /// Composes independent adapters without introducing a global service locator.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        descriptor: IntegrationDescriptor,
        files: F,
        dialogs: D,
        workspace: W,
        watcher: Watch,
        sessions: S,
        syntax: Syntax,
        completion: Completion,
    ) -> Self {
        Self {
            descriptor,
            files,
            dialogs,
            workspace,
            watcher,
            sessions,
            syntax,
            completion,
        }
    }

    /// Returns a deterministic support-policy report for diagnostics or About surfaces.
    #[must_use]
    pub const fn platform_report(&self) -> IntegrationPlatformReport {
        IntegrationPlatformReport::current()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DownstreamServices, IntegrationDescriptor, IntegrationPlatformReport, ResourceError,
        ResourceLocator,
    };
    use luna_document_services::{MemoryTextFileService, ScriptedDialogService};
    use luna_editor::KeywordSyntaxProvider;
    use luna_session::MemorySessionStore;
    use luna_ui::ScriptedCompletionProvider;
    use luna_workspaces::{MemoryWorkspaceService, MemoryWorkspaceWatchService};
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn downstream_adapters_compose_without_product_policy() -> TestResult {
        let files = MemoryTextFileService::new("/integration-files")?;
        files.insert_utf8(Path::new("/integration-files/example.rs"), "fn main() {}")?;
        let workspace = MemoryWorkspaceService::new("/integration-workspace")?;
        let services = DownstreamServices::new(
            IntegrationDescriptor::new("org.example.LunaConsumer", "Luna Consumer")?,
            files,
            ScriptedDialogService::default(),
            workspace,
            MemoryWorkspaceWatchService::default(),
            MemorySessionStore::default(),
            KeywordSyntaxProvider::rust_demo(),
            ScriptedCompletionProvider::default(),
        );

        assert_eq!(services.descriptor.display_name, "Luna Consumer");
        assert_eq!(
            services.platform_report(),
            IntegrationPlatformReport::current()
        );
        Ok(())
    }

    #[test]
    fn descriptor_rejects_empty_identifiers() {
        assert!(IntegrationDescriptor::new("", "Consumer").is_err());
        assert!(IntegrationDescriptor::new("org.example.Consumer", " ").is_err());
    }

    static RESOURCE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn resource_locator_reads_strict_utf8_from_explicit_root() -> TestResult {
        let sequence = RESOURCE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "luna-integration-resource-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root)?;
        fs::write(root.join("sample.txt"), "qualified")?;
        let mut locator = ResourceLocator::new("org.example.Consumer")?;
        locator.push_root(&root);
        assert_eq!(locator.read_utf8(Path::new("sample.txt"))?, "qualified");
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn resource_locator_rejects_parent_traversal() -> TestResult {
        let mut locator = ResourceLocator::new("org.example.Consumer")?;
        locator.push_root("/tmp/luna-resources");
        assert!(matches!(
            locator.resolve(Path::new("../secret")),
            Err(ResourceError::InvalidRelativePath(_))
        ));
        Ok(())
    }
}
