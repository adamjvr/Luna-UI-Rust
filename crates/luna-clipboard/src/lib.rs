// SPDX-License-Identifier: MPL-2.0

//! Product-neutral UTF-8 text clipboard services.
//!
//! Luna owns the clipboard contract while platform integration stays behind a small adapter. The
//! in-memory implementation supports deterministic tests and downstream applications may supply
//! their own adapter. [`SystemClipboardService`] uses `arboard` for the native desktop clipboard.

use arboard::{Clipboard, Error as ArboardError};
use luna_core::{CodedError, ErrorCode};
use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};

/// Product-neutral UTF-8 text clipboard boundary.
pub trait ClipboardService {
    /// Returns whether this adapter initialized a usable clipboard backend.
    fn is_available(&self) -> bool;

    /// Reads UTF-8 text from the clipboard.
    ///
    /// `Ok(None)` means the clipboard contains no non-empty UTF-8 text.
    ///
    /// # Errors
    ///
    /// Returns [`ClipboardError`] when the backend is unavailable or the clipboard cannot be read.
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError>;

    /// Writes UTF-8 text to the clipboard.
    ///
    /// # Errors
    ///
    /// Returns [`ClipboardError`] when the backend is unavailable or the text cannot be written.
    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError>;
}

/// Stable category for one clipboard failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipboardErrorKind {
    /// No native clipboard backend could be initialized.
    Unavailable,
    /// Clipboard text could not be read.
    Read,
    /// Clipboard text could not be written.
    Write,
}

/// Failure returned by a clipboard service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardError {
    kind: ClipboardErrorKind,
    message: String,
}

impl ClipboardError {
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            kind: ClipboardErrorKind::Unavailable,
            message: message.into(),
        }
    }

    fn read(message: impl Into<String>) -> Self {
        Self {
            kind: ClipboardErrorKind::Read,
            message: message.into(),
        }
    }

    fn write(message: impl Into<String>) -> Self {
        Self {
            kind: ClipboardErrorKind::Write,
            message: message.into(),
        }
    }

    /// Returns the stable failure category.
    #[must_use]
    pub const fn kind(&self) -> ClipboardErrorKind {
        self.kind
    }

    /// Returns the adapter-provided diagnostic text.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for ClipboardError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "clipboard {:?}: {}", self.kind, self.message)
    }
}

impl Error for ClipboardError {}

impl CodedError for ClipboardError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self.kind {
            ClipboardErrorKind::Unavailable => "clipboard.unavailable",
            ClipboardErrorKind::Read => "clipboard.read",
            ClipboardErrorKind::Write => "clipboard.write",
        })
    }
}

/// Native desktop clipboard adapter backed by `arboard`.
pub struct SystemClipboardService {
    clipboard: Option<Clipboard>,
    initialization_error: Option<String>,
}

impl SystemClipboardService {
    /// Detects and initializes the current desktop clipboard backend.
    ///
    /// Initialization failure is retained so applications may keep running with clipboard commands
    /// disabled instead of failing application startup.
    #[must_use]
    pub fn detect() -> Self {
        match Clipboard::new() {
            Ok(clipboard) => Self {
                clipboard: Some(clipboard),
                initialization_error: None,
            },
            Err(error) => Self {
                clipboard: None,
                initialization_error: Some(error.to_string()),
            },
        }
    }

    fn unavailable_error(&self) -> ClipboardError {
        ClipboardError::unavailable(
            self.initialization_error
                .as_deref()
                .unwrap_or("no clipboard backend is available"),
        )
    }
}

impl Default for SystemClipboardService {
    fn default() -> Self {
        Self::detect()
    }
}

impl Debug for SystemClipboardService {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SystemClipboardService")
            .field("is_available", &self.is_available())
            .field("initialization_error", &self.initialization_error)
            .finish_non_exhaustive()
    }
}

impl ClipboardService for SystemClipboardService {
    fn is_available(&self) -> bool {
        self.clipboard.is_some()
    }

    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        let Some(clipboard) = self.clipboard.as_mut() else {
            return Err(self.unavailable_error());
        };
        match clipboard.get_text() {
            Ok(text) => Ok((!text.is_empty()).then_some(text)),
            Err(ArboardError::ContentNotAvailable) => Ok(None),
            Err(error) => Err(ClipboardError::read(error.to_string())),
        }
    }

    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        let Some(clipboard) = self.clipboard.as_mut() else {
            return Err(self.unavailable_error());
        };
        clipboard
            .set_text(text)
            .map_err(|error| ClipboardError::write(error.to_string()))
    }
}

/// Deterministic in-memory clipboard for tests and embedded consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryClipboardService {
    is_available: bool,
    text: Option<String>,
}

impl MemoryClipboardService {
    /// Creates an available empty clipboard.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            is_available: true,
            text: None,
        }
    }

    /// Creates an available clipboard containing `text`.
    #[must_use]
    pub fn with_text(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            is_available: true,
            text: (!text.is_empty()).then_some(text),
        }
    }

    /// Creates an unavailable clipboard adapter for error-path tests.
    #[must_use]
    pub const fn unavailable() -> Self {
        Self {
            is_available: false,
            text: None,
        }
    }

    /// Returns the currently retained text.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }
}

impl Default for MemoryClipboardService {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardService for MemoryClipboardService {
    fn is_available(&self) -> bool {
        self.is_available
    }

    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        if !self.is_available {
            return Err(ClipboardError::unavailable(
                "memory clipboard was configured as unavailable",
            ));
        }
        Ok(self.text.clone())
    }

    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        if !self.is_available {
            return Err(ClipboardError::unavailable(
                "memory clipboard was configured as unavailable",
            ));
        }
        self.text = (!text.is_empty()).then(|| text.to_owned());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ClipboardErrorKind, ClipboardService, MemoryClipboardService};
    use luna_core::CodedError;
    use std::error::Error;

    #[test]
    fn memory_clipboard_round_trips_utf8_text() -> Result<(), Box<dyn Error>> {
        let mut clipboard = MemoryClipboardService::new();
        clipboard.write_text("Luna 🌙")?;
        assert_eq!(clipboard.read_text()?.as_deref(), Some("Luna 🌙"));
        assert_eq!(clipboard.text(), Some("Luna 🌙"));
        Ok(())
    }

    #[test]
    fn empty_text_clears_memory_clipboard() -> Result<(), Box<dyn Error>> {
        let mut clipboard = MemoryClipboardService::with_text("temporary");
        clipboard.write_text("")?;
        assert_eq!(clipboard.read_text()?, None);
        Ok(())
    }

    #[test]
    fn unavailable_clipboard_has_stable_error_code() {
        let mut clipboard = MemoryClipboardService::unavailable();
        let error = match clipboard.read_text() {
            Ok(value) => {
                assert_eq!(value, None);
                return;
            }
            Err(error) => error,
        };
        assert_eq!(error.kind(), ClipboardErrorKind::Unavailable);
        assert_eq!(error.error_code().as_str(), "clipboard.unavailable");
    }
}
