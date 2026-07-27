// SPDX-License-Identifier: MPL-2.0

use crate::{CodedError, ErrorCode};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// A stable semantic identifier for a UI node.
///
/// Luna uses the same identifier across rendering-adjacent state, hit testing, accessibility,
/// focus, and command routing. The identifier is intentionally a validated string rather than a
/// process-local integer so tests and debugging output remain readable and deterministic.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(String);

impl NodeId {
    /// Creates a validated node identifier.
    ///
    /// Identifiers must be non-empty and cannot contain whitespace. Dots are reserved as the
    /// conventional parent/child separator, but are otherwise treated as ordinary characters.
    pub fn new(value: impl Into<String>) -> Result<Self, NodeIdError> {
        let value = value.into();

        if value.is_empty() {
            return Err(NodeIdError::Empty);
        }
        if value.chars().any(char::is_whitespace) {
            return Err(NodeIdError::ContainsWhitespace(value));
        }

        Ok(Self(value))
    }

    /// Creates a deterministic child identifier below this node.
    pub fn child(&self, segment: &str) -> Result<Self, NodeIdError> {
        if segment.is_empty() {
            return Err(NodeIdError::EmptyChildSegment);
        }
        if segment.contains('.') {
            return Err(NodeIdError::ChildSegmentContainsSeparator(
                segment.to_owned(),
            ));
        }
        if segment.chars().any(char::is_whitespace) {
            return Err(NodeIdError::ContainsWhitespace(segment.to_owned()));
        }

        Self::new(format!("{}.{}", self.0, segment))
    }

    /// Returns the identifier as a borrowed string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for NodeId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Validation failures returned while constructing a [`NodeId`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NodeIdError {
    /// The full identifier was empty.
    Empty,
    /// A child segment was empty.
    EmptyChildSegment,
    /// The identifier or child segment contained whitespace.
    ContainsWhitespace(String),
    /// A child segment attempted to contain Luna's hierarchy separator.
    ChildSegmentContainsSeparator(String),
}

impl Display for NodeIdError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("a Luna node identifier cannot be empty"),
            Self::EmptyChildSegment => {
                formatter.write_str("a Luna node identifier child segment cannot be empty")
            }
            Self::ContainsWhitespace(value) => {
                write!(
                    formatter,
                    "Luna node identifier contains whitespace: {value:?}"
                )
            }
            Self::ChildSegmentContainsSeparator(value) => write!(
                formatter,
                "Luna node identifier child segment contains '.': {value:?}"
            ),
        }
    }
}

impl Error for NodeIdError {}

impl CodedError for NodeIdError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self {
            Self::Empty => "core.node_id.empty",
            Self::EmptyChildSegment => "core.node_id.empty_child_segment",
            Self::ContainsWhitespace(_) => "core.node_id.contains_whitespace",
            Self::ChildSegmentContainsSeparator(_) => "core.node_id.child_contains_separator",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::NodeId;
    use std::error::Error;

    #[test]
    fn child_ids_are_stable_and_readable() -> Result<(), Box<dyn Error>> {
        let root = NodeId::new("editor")?;
        let child = root.child("viewport")?;

        assert_eq!(child.as_str(), "editor.viewport");
        Ok(())
    }

    #[test]
    fn whitespace_is_rejected() {
        assert!(NodeId::new("editor viewport").is_err());
    }
}
