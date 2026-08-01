// SPDX-License-Identifier: MPL-2.0

//! Validated downstream-product identity and namespace contracts.
//!
//! This module deliberately validates names only. It does not create commands, choose
//! storage locations, load resources, or own product policy. A downstream application
//! remains authoritative for the meaning of every namespace.

use crate::IntegrationDescriptor;
use luna_core::{CodedError, ErrorCode};
use std::error::Error;
use std::fmt::{Display, Formatter};

/// One validated downstream-owned namespace.
///
/// Namespaces are intentionally opaque to Luna. They must be non-empty, contain no
/// whitespace, and begin and end with an ASCII alphanumeric character. Dots, dashes,
/// underscores, and additional ASCII alphanumeric characters are accepted internally.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IntegrationNamespace(String);

impl IntegrationNamespace {
    /// Creates a validated namespace.
    ///
    /// # Errors
    ///
    /// Returns [`IntegrationProfileError::InvalidNamespace`] when the value is blank,
    /// contains whitespace or unsupported characters, or begins/ends with punctuation.
    pub fn new(value: impl Into<String>) -> Result<Self, IntegrationProfileError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
            })
            && value
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_alphanumeric())
            && value
                .chars()
                .next_back()
                .is_some_and(|character| character.is_ascii_alphanumeric());

        if !valid {
            return Err(IntegrationProfileError::InvalidNamespace(value));
        }
        Ok(Self(value))
    }

    /// Returns the validated namespace.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for IntegrationNamespace {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Downstream-owned identity used to keep command, session, and resource concerns distinct.
///
/// Luna stores these values for composition and diagnostics only. The downstream product
/// decides how each namespace maps to command identifiers, persistence, packages, and
/// resource lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownstreamApplicationProfile {
    /// Stable application identity and human-readable name.
    pub descriptor: IntegrationDescriptor,
    /// Prefix used by product command identifiers.
    pub command_namespace: IntegrationNamespace,
    /// Namespace used by the product's session and recovery storage.
    pub session_namespace: IntegrationNamespace,
    /// Namespace used by packaged resources and data lookup.
    pub resource_namespace: IntegrationNamespace,
}

impl DownstreamApplicationProfile {
    /// Creates a complete downstream application profile.
    #[must_use]
    pub fn new(
        descriptor: IntegrationDescriptor,
        command_namespace: IntegrationNamespace,
        session_namespace: IntegrationNamespace,
        resource_namespace: IntegrationNamespace,
    ) -> Self {
        Self {
            descriptor,
            command_namespace,
            session_namespace,
            resource_namespace,
        }
    }
}

/// Validation error for a downstream integration profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IntegrationProfileError {
    /// A namespace was empty or contained unsupported syntax.
    InvalidNamespace(String),
}

impl Display for IntegrationProfileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidNamespace(value) => {
                write!(
                    formatter,
                    "invalid downstream integration namespace: {value:?}"
                )
            }
        }
    }
}

impl Error for IntegrationProfileError {}

impl CodedError for IntegrationProfileError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self {
            Self::InvalidNamespace(_) => "integration.profile.invalid_namespace",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{DownstreamApplicationProfile, IntegrationNamespace};
    use crate::IntegrationDescriptor;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn profile_keeps_product_namespaces_explicit() -> TestResult {
        let profile = DownstreamApplicationProfile::new(
            IntegrationDescriptor::new("org.example.Editor", "Example Editor")?,
            IntegrationNamespace::new("example")?,
            IntegrationNamespace::new("org.example.Editor.session")?,
            IntegrationNamespace::new("org.example.Editor.resources")?,
        );

        assert_eq!(profile.descriptor.display_name, "Example Editor");
        assert_eq!(profile.command_namespace.as_str(), "example");
        assert_eq!(
            profile.session_namespace.as_str(),
            "org.example.Editor.session"
        );
        assert_eq!(
            profile.resource_namespace.as_str(),
            "org.example.Editor.resources"
        );
        Ok(())
    }

    #[test]
    fn namespace_rejects_ambiguous_or_shell_like_values() {
        for invalid in [
            "",
            " ",
            ".example",
            "example.",
            "example command",
            "example/command",
            "example;command",
        ] {
            assert!(IntegrationNamespace::new(invalid).is_err());
        }
    }
}
