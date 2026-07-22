// SPDX-License-Identifier: MPL-2.0

/// Diagnostic severity emitted by Luna's deterministic core layers.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DiagnosticSeverity {
    /// Useful development information that does not indicate a fault.
    Info,
    /// A recoverable condition that deserves attention.
    Warning,
    /// A condition that prevented an operation from completing correctly.
    Error,
}

/// A structured, test-friendly diagnostic message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    /// Stable machine-readable code.
    pub code: &'static str,
    /// Human-readable explanation.
    pub message: String,
    /// Severity of the condition.
    pub severity: DiagnosticSeverity,
}

impl Diagnostic {
    /// Creates a diagnostic.
    #[must_use]
    pub fn new(
        code: &'static str,
        message: impl Into<String>,
        severity: DiagnosticSeverity,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            severity,
        }
    }
}

/// An append-only diagnostic collection.
///
/// The core does not print directly to stdout or stderr. Hosts and applications decide how and
/// where diagnostics are surfaced, keeping unit tests deterministic and library embedding quiet.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Diagnostics {
    entries: Vec<Diagnostic>,
}

impl Diagnostics {
    /// Appends one diagnostic.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.entries.push(diagnostic);
    }

    /// Returns all diagnostics in emission order.
    #[must_use]
    pub fn entries(&self) -> &[Diagnostic] {
        &self.entries
    }

    /// Removes and returns every accumulated diagnostic.
    pub fn drain(&mut self) -> impl Iterator<Item = Diagnostic> + '_ {
        self.entries.drain(..)
    }

    /// Returns whether no diagnostics are currently stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
