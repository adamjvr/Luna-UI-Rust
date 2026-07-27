// SPDX-License-Identifier: MPL-2.0

use crate::{ByteSelection, EditGroup, EditHistory, HistorySnapshot, SelectionError, SelectionSet};
use luna_core::{CodedError, ErrorCode};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// One deterministic editor operation used by cross-implementation behavior fixtures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParityOperation {
    /// Replace every active selection with committed UTF-8 text.
    Insert(String),
    /// Delete one grapheme cluster backward at every collapsed caret.
    DeleteBackward,
    /// Delete one grapheme cluster forward at every collapsed caret.
    DeleteForward,
    /// Replace the active selections with an explicit set.
    SetSelections {
        /// Directional selections in document order.
        selections: Vec<ByteSelection>,
        /// Index of the primary selection before normalization.
        primary_index: usize,
    },
    /// Add a caret on the logical line above the primary caret.
    AddCursorAbove,
    /// Add a caret on the logical line below the primary caret.
    AddCursorBelow,
    /// Remove every secondary selection.
    ClearSecondaryCursors,
    /// Restore the preceding transaction.
    Undo,
    /// Restore the next transaction.
    Redo,
}

/// Expected terminal state for one reusable editor behavior fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParityResult {
    /// Complete UTF-8 text after all operations.
    pub text: String,
    /// Final normalized selection set.
    pub selections: SelectionSet,
    /// Remaining undo depth.
    pub undo_depth: usize,
    /// Remaining redo depth.
    pub redo_depth: usize,
}

/// Cross-language behavior fixture independent of rendering and platform hosts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorParityFixture {
    /// Human-readable fixture name shared by Swift and Rust test suites.
    pub name: String,
    /// Initial UTF-8 document.
    pub initial_text: String,
    /// Initial directional selections.
    pub initial_selections: SelectionSet,
    /// Operations to replay in order.
    pub operations: Vec<ParityOperation>,
    /// Expected terminal state.
    pub expected: ParityResult,
}

impl EditorParityFixture {
    /// Replays this fixture using Luna's product-neutral editor mechanics.
    pub fn replay(&self) -> Result<ParityResult, ParityError> {
        let mut text = self.initial_text.clone();
        let mut selections = self.initial_selections.normalized(&text);
        let mut history = EditHistory::new(512);

        for (operation_index, operation) in self.operations.iter().enumerate() {
            match operation {
                ParityOperation::Insert(inserted) => {
                    apply_edit(
                        &mut text,
                        &mut selections,
                        &mut history,
                        EditGroup::Typing,
                        |text, selections| selections.replace_all(text, inserted),
                    );
                }
                ParityOperation::DeleteBackward => {
                    apply_edit(
                        &mut text,
                        &mut selections,
                        &mut history,
                        EditGroup::Deletion,
                        |text, selections| selections.delete_backward(text),
                    );
                }
                ParityOperation::DeleteForward => {
                    apply_edit(
                        &mut text,
                        &mut selections,
                        &mut history,
                        EditGroup::Deletion,
                        |text, selections| selections.delete_forward(text),
                    );
                }
                ParityOperation::SetSelections {
                    selections: replacement,
                    primary_index,
                } => {
                    selections = SelectionSet::new(&text, replacement.clone(), *primary_index)
                        .map_err(|source| ParityError::InvalidSelection {
                            operation_index,
                            source,
                        })?;
                }
                ParityOperation::AddCursorAbove => {
                    let _ = selections.add_cursor_vertical(&text, -1);
                }
                ParityOperation::AddCursorBelow => {
                    let _ = selections.add_cursor_vertical(&text, 1);
                }
                ParityOperation::ClearSecondaryCursors => selections.clear_secondary(),
                ParityOperation::Undo => {
                    let current = HistorySnapshot::new(text.clone(), selections.clone());
                    if let Some(snapshot) = history.undo(&current) {
                        text = snapshot.text;
                        selections = snapshot.selections;
                    }
                }
                ParityOperation::Redo => {
                    let current = HistorySnapshot::new(text.clone(), selections.clone());
                    if let Some(snapshot) = history.redo(&current) {
                        text = snapshot.text;
                        selections = snapshot.selections;
                    }
                }
            }
        }

        Ok(ParityResult {
            text,
            selections,
            undo_depth: history.undo_depth(),
            redo_depth: history.redo_depth(),
        })
    }

    /// Replays and compares the actual result with the checked-in expectation.
    pub fn verify(&self) -> Result<(), ParityError> {
        let actual = self.replay()?;
        if actual == self.expected {
            Ok(())
        } else {
            Err(ParityError::Mismatch {
                fixture: self.name.clone(),
                expected: self.expected.clone(),
                actual,
            })
        }
    }
}

fn apply_edit(
    text: &mut String,
    selections: &mut SelectionSet,
    history: &mut EditHistory,
    group: EditGroup,
    operation: impl FnOnce(&str, &SelectionSet) -> crate::MultiEditResult,
) {
    let before = HistorySnapshot::new(text.clone(), selections.clone());
    let result = operation(text, selections);
    if !result.did_change {
        return;
    }
    *text = result.text;
    *selections = result.selections;
    let after = HistorySnapshot::new(text.clone(), selections.clone());
    history.record(before, after, group);
}

/// Failure while replaying or checking an editor parity fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParityError {
    /// A fixture operation supplied an invalid selection set.
    InvalidSelection {
        /// Zero-based operation index.
        operation_index: usize,
        /// Validation failure.
        source: SelectionError,
    },
    /// The replayed terminal state differed from the expected state.
    Mismatch {
        /// Fixture name.
        fixture: String,
        /// Checked-in expected state.
        expected: ParityResult,
        /// Actual Rust result.
        actual: ParityResult,
    },
}

impl Display for ParityError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSelection {
                operation_index,
                source,
            } => write!(
                formatter,
                "parity operation {operation_index} supplied invalid selections: {source}"
            ),
            Self::Mismatch {
                fixture,
                expected,
                actual,
            } => write!(
                formatter,
                "editor parity fixture {fixture:?} mismatch: expected {expected:?}, got {actual:?}"
            ),
        }
    }
}

impl Error for ParityError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidSelection { source, .. } => Some(source),
            Self::Mismatch { .. } => None,
        }
    }
}

impl CodedError for ParityError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self {
            Self::InvalidSelection { .. } => "editor.parity.invalid_selection",
            Self::Mismatch { .. } => "editor.parity.mismatch",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorParityFixture, ParityOperation, ParityResult};
    use crate::{ByteSelection, SelectionSet};
    use std::error::Error;

    #[test]
    fn deterministic_multi_cursor_fixture_matches() -> Result<(), Box<dyn Error>> {
        let fixture = EditorParityFixture {
            name: "two-line insertion and undo".to_owned(),
            initial_text: "one\ntwo".to_owned(),
            initial_selections: SelectionSet::single(ByteSelection::caret(0)),
            operations: vec![
                ParityOperation::AddCursorBelow,
                ParityOperation::Insert("> ".to_owned()),
                ParityOperation::Undo,
                ParityOperation::Redo,
            ],
            expected: ParityResult {
                text: "> one\n> two".to_owned(),
                selections: SelectionSet::new(
                    "> one\n> two",
                    [ByteSelection::caret(2), ByteSelection::caret(8)],
                    1,
                )?,
                undo_depth: 1,
                redo_depth: 0,
            },
        };
        fixture.verify()?;
        Ok(())
    }
}
