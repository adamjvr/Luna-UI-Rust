// SPDX-License-Identifier: MPL-2.0

//! Product-neutral composition helpers for downstream Luna applications.
//!
//! Luna's service traits intentionally remain independent. This crate demonstrates how a consumer
//! can gather file, dialog, workspace, watcher, session, syntax, and completion adapters into one
//! application-owned value without making Luna responsible for product workflow or policy.

use luna_host_core::{
    NativePlatform, PlatformSupportTier, current_native_platform, platform_support_tier,
};

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
    use super::{DownstreamServices, IntegrationDescriptor, IntegrationPlatformReport};
    use luna_document_services::{MemoryTextFileService, ScriptedDialogService};
    use luna_editor::KeywordSyntaxProvider;
    use luna_session::MemorySessionStore;
    use luna_ui::ScriptedCompletionProvider;
    use luna_workspaces::{MemoryWorkspaceService, MemoryWorkspaceWatchService};
    use std::path::Path;

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
}
