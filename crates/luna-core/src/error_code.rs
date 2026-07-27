// SPDX-License-Identifier: MPL-2.0

//! Stable machine-readable error identifiers for public Luna boundaries.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Stable machine-readable identifier attached to a public Luna error.
///
/// Error text may improve over time, but an [`ErrorCode`] is intended to remain stable throughout
/// compatible releases. Codes use lowercase dotted namespaces such as `workspace.not_found`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ErrorCode(&'static str);

impl ErrorCode {
    /// Creates a code from a checked-in static identifier.
    ///
    /// Public crates should expose named constants or return codes from [`CodedError`] rather than
    /// constructing user-controlled identifiers at runtime.
    #[must_use]
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the stable dotted identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl Display for ErrorCode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Public error contract exposing a stable machine-readable code.
///
/// Applications should use the code for telemetry, tests, and localized presentation while keeping
/// [`Error::source`] and [`Display`] for human-readable diagnostics.
pub trait CodedError: Error {
    /// Returns the stable code for this concrete failure.
    fn error_code(&self) -> ErrorCode;
}

#[cfg(test)]
mod tests {
    use super::ErrorCode;

    #[test]
    fn error_codes_preserve_static_identifiers() {
        let code = ErrorCode::new("workspace.not_found");
        assert_eq!(code.as_str(), "workspace.not_found");
        assert_eq!(code.to_string(), "workspace.not_found");
    }
}
