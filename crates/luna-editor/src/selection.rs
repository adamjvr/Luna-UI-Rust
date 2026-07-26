// SPDX-License-Identifier: MPL-2.0

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// One directional UTF-8 byte selection.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ByteSelection {
    /// Fixed endpoint used when extending the selection.
    pub anchor: usize,
    /// Moving endpoint and active insertion position.
    pub focus: usize,
}

impl ByteSelection {
    /// Creates a directional selection.
    #[must_use]
    pub const fn new(anchor: usize, focus: usize) -> Self {
        Self { anchor, focus }
    }

    /// Creates a collapsed insertion point.
    #[must_use]
    pub const fn caret(offset: usize) -> Self {
        Self::new(offset, offset)
    }

    /// Returns whether this selection has no selected bytes.
    #[must_use]
    pub const fn is_collapsed(self) -> bool {
        self.anchor == self.focus
    }

    /// Returns the ascending byte range covered by this selection.
    #[must_use]
    pub fn normalized_range(self) -> Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }
}

/// One simultaneous replacement applied by [`SelectionSet`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ByteEdit {
    /// Replaced range in the pre-edit UTF-8 text.
    pub old_range: Range<usize>,
    /// Inserted text.
    pub inserted_text: String,
}

/// Result of applying one operation across every selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiEditResult {
    /// Complete text after every edit.
    pub text: String,
    /// Post-edit collapsed selections.
    pub selections: SelectionSet,
    /// Individual edits in ascending pre-edit order.
    pub edits: Vec<ByteEdit>,
    /// Whether any bytes changed.
    pub did_change: bool,
}

/// Ordered non-overlapping set of directional UTF-8 selections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionSet {
    selections: Vec<ByteSelection>,
    primary_index: usize,
}

impl SelectionSet {
    /// Creates a set containing one primary selection.
    #[must_use]
    pub fn single(selection: ByteSelection) -> Self {
        Self {
            selections: vec![selection],
            primary_index: 0,
        }
    }

    /// Creates a selection set and validates it against `text`.
    pub fn new(
        text: &str,
        selections: impl IntoIterator<Item = ByteSelection>,
        primary_index: usize,
    ) -> Result<Self, SelectionError> {
        let selections = selections.into_iter().collect::<Vec<_>>();
        if selections.is_empty() {
            return Err(SelectionError::EmptySet);
        }
        if primary_index >= selections.len() {
            return Err(SelectionError::InvalidPrimaryIndex(primary_index));
        }
        let set = Self {
            selections,
            primary_index,
        };
        Ok(set.normalized(text))
    }

    /// Returns all selections in ascending document order.
    #[must_use]
    pub fn selections(&self) -> &[ByteSelection] {
        &self.selections
    }

    /// Returns the primary selection.
    #[must_use]
    pub fn primary(&self) -> ByteSelection {
        self.selections[self.primary_index]
    }

    /// Returns the number of selections.
    #[must_use]
    pub fn len(&self) -> usize {
        self.selections.len()
    }

    /// Returns whether there are no selections.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selections.is_empty()
    }

    /// Returns whether more than one insertion or selection is active.
    #[must_use]
    pub fn is_multiple(&self) -> bool {
        self.selections.len() > 1
    }

    /// Replaces this set with one primary selection.
    pub fn set_single(&mut self, selection: ByteSelection) {
        self.selections.clear();
        self.selections.push(selection);
        self.primary_index = 0;
    }

    /// Removes every secondary selection while preserving the primary selection.
    pub fn clear_secondary(&mut self) {
        let primary = self.primary();
        self.set_single(primary);
    }

    /// Adds one selection, then clamps, sorts, and merges the set.
    pub fn add(&mut self, text: &str, selection: ByteSelection) {
        self.selections.push(selection);
        self.primary_index = self.selections.len().saturating_sub(1);
        *self = self.normalized(text);
    }

    /// Returns a clamped, sorted, non-overlapping version of this set.
    #[must_use]
    pub fn normalized(&self, text: &str) -> Self {
        let primary_focus = snap_backward(text, self.primary().focus.min(text.len()));
        let mut selections = self
            .selections
            .iter()
            .map(|selection| {
                ByteSelection::new(
                    snap_backward(text, selection.anchor.min(text.len())),
                    snap_backward(text, selection.focus.min(text.len())),
                )
            })
            .collect::<Vec<_>>();
        selections.sort_by_key(|selection| {
            let range = selection.normalized_range();
            (range.start, range.end, selection.anchor, selection.focus)
        });

        let mut merged = Vec::<ByteSelection>::new();
        for selection in selections {
            let range = selection.normalized_range();
            let Some(previous) = merged.last_mut() else {
                merged.push(selection);
                continue;
            };
            let previous_range = previous.normalized_range();
            let overlaps = range.start < previous_range.end
                || (range.start == previous_range.end
                    && (!selection.is_collapsed() || !previous.is_collapsed()))
                || (selection.is_collapsed()
                    && previous.is_collapsed()
                    && selection.focus == previous.focus);
            if overlaps {
                let start = previous_range.start.min(range.start);
                let end = previous_range.end.max(range.end);
                *previous = ByteSelection::new(start, end);
            } else {
                merged.push(selection);
            }
        }
        if merged.is_empty() {
            merged.push(ByteSelection::caret(0));
        }
        let primary_index = merged
            .iter()
            .position(|selection| {
                let range = selection.normalized_range();
                if selection.is_collapsed() {
                    selection.focus == primary_focus
                } else {
                    range.start <= primary_focus && primary_focus <= range.end
                }
            })
            .unwrap_or_else(|| merged.len().saturating_sub(1));
        Self {
            selections: merged,
            primary_index,
        }
    }

    /// Replaces each selected range, or inserts at every caret, with the same text.
    #[must_use]
    pub fn replace_all(&self, text: &str, replacement: &str) -> MultiEditResult {
        let normalized = self.normalized(text);
        let ranges = normalized
            .selections
            .iter()
            .map(|selection| selection.normalized_range())
            .collect::<Vec<_>>();
        let did_change = ranges.iter().any(|range| {
            text.get(range.clone())
                .is_some_and(|existing| existing != replacement)
        });
        if !did_change {
            return MultiEditResult {
                text: text.to_owned(),
                selections: normalized,
                edits: Vec::new(),
                did_change: false,
            };
        }

        let mut next_text = text.to_owned();
        for range in ranges.iter().rev() {
            next_text.replace_range(range.clone(), replacement);
        }

        let mut delta = 0_i128;
        let mut next_selections = Vec::with_capacity(ranges.len());
        let mut edits = Vec::with_capacity(ranges.len());
        for range in ranges {
            let adjusted_start = add_signed(range.start, delta);
            let caret = adjusted_start.saturating_add(replacement.len());
            next_selections.push(ByteSelection::caret(caret));
            let removed =
                i128::try_from(range.end.saturating_sub(range.start)).unwrap_or(i128::MAX);
            let inserted = i128::try_from(replacement.len()).unwrap_or(i128::MAX);
            delta = delta.saturating_add(inserted.saturating_sub(removed));
            edits.push(ByteEdit {
                old_range: range,
                inserted_text: replacement.to_owned(),
            });
        }
        let primary_focus = normalized.primary().focus;
        let primary_index = normalized
            .selections
            .iter()
            .position(|selection| selection.focus == primary_focus)
            .unwrap_or(0)
            .min(next_selections.len().saturating_sub(1));
        MultiEditResult {
            text: next_text,
            selections: Self {
                selections: next_selections,
                primary_index,
            },
            edits,
            did_change: true,
        }
    }

    /// Deletes the preceding extended grapheme cluster at every collapsed caret.
    #[must_use]
    pub fn delete_backward(&self, text: &str) -> MultiEditResult {
        let delete_set = self.with_collapsed_ranges(text, previous_grapheme_boundary);
        delete_set.replace_all(text, "")
    }

    /// Deletes the following extended grapheme cluster at every collapsed caret.
    #[must_use]
    pub fn delete_forward(&self, text: &str) -> MultiEditResult {
        let delete_set = self.with_collapsed_ranges(text, next_grapheme_boundary);
        delete_set.replace_all(text, "")
    }

    /// Adds a caret on the logical line `line_delta` away from the primary caret.
    ///
    /// The new caret preserves the primary caret's UTF-8 byte column and clamps to the target
    /// line. A delta of zero or a move beyond the document returns `false`.
    pub fn add_cursor_vertical(&mut self, text: &str, line_delta: i32) -> bool {
        if line_delta == 0 {
            return false;
        }
        let primary = self.primary();
        let focus = snap_backward(text, primary.focus.min(text.len()));
        let (line_index, line_start, column) = line_information(text, focus);
        let target_line = if line_delta < 0 {
            line_index.checked_sub(usize::try_from(line_delta.unsigned_abs()).unwrap_or(usize::MAX))
        } else {
            line_index.checked_add(usize::try_from(line_delta.unsigned_abs()).unwrap_or(usize::MAX))
        };
        let Some(target_line) = target_line else {
            return false;
        };
        let Some((target_start, target_end)) = line_bounds(text, target_line) else {
            return false;
        };
        let target = snap_backward(text, target_start.saturating_add(column).min(target_end));
        if target == focus && target_start == line_start {
            return false;
        }
        let old_len = self.selections.len();
        self.add(text, ByteSelection::caret(target));
        self.selections.len() > old_len
    }

    fn with_collapsed_ranges(&self, text: &str, boundary: fn(&str, usize) -> usize) -> Self {
        let selections = self
            .normalized(text)
            .selections
            .iter()
            .map(|selection| {
                if selection.is_collapsed() {
                    let other = boundary(text, selection.focus);
                    ByteSelection::new(other, selection.focus)
                } else {
                    *selection
                }
            })
            .collect::<Vec<_>>();
        Self {
            primary_index: self.primary_index.min(selections.len().saturating_sub(1)),
            selections,
        }
        .normalized(text)
    }
}

impl Default for SelectionSet {
    fn default() -> Self {
        Self::single(ByteSelection::caret(0))
    }
}

/// Validation failure while constructing a [`SelectionSet`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionError {
    /// At least one selection is required.
    EmptySet,
    /// The primary index did not identify an existing selection.
    InvalidPrimaryIndex(usize),
}

impl Display for SelectionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySet => formatter.write_str("a selection set cannot be empty"),
            Self::InvalidPrimaryIndex(index) => {
                write!(
                    formatter,
                    "selection primary index {index} is out of bounds"
                )
            }
        }
    }
}

impl Error for SelectionError {}

fn previous_grapheme_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|index| *index < offset)
        .last()
        .unwrap_or(0)
}

fn next_grapheme_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(index, grapheme)| index.saturating_add(grapheme.len()))
        .find(|index| *index > offset)
        .unwrap_or(text.len())
}

fn snap_backward(text: &str, mut offset: usize) -> usize {
    offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset = offset.saturating_sub(1);
    }
    offset
}

fn add_signed(value: usize, delta: i128) -> usize {
    if delta >= 0 {
        value.saturating_add(usize::try_from(delta).unwrap_or(usize::MAX))
    } else {
        value.saturating_sub(usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX))
    }
}

fn line_information(text: &str, offset: usize) -> (usize, usize, usize) {
    let prefix = text.get(..offset).unwrap_or(text);
    let line_index = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let line_start = prefix
        .rfind('\n')
        .map_or(0, |index| index.saturating_add(1));
    (line_index, line_start, offset.saturating_sub(line_start))
}

fn line_bounds(text: &str, target_line: usize) -> Option<(usize, usize)> {
    let mut current_line = 0_usize;
    let mut start = 0_usize;
    for (index, character) in text.char_indices() {
        if current_line == target_line && character == '\n' {
            return Some((start, index));
        }
        if character == '\n' {
            current_line = current_line.saturating_add(1);
            start = index.saturating_add(character.len_utf8());
        }
    }
    (current_line == target_line).then_some((start, text.len()))
}

#[cfg(test)]
mod tests {
    use super::{ByteSelection, SelectionSet};
    use std::error::Error;

    #[test]
    fn simultaneous_insertions_apply_from_right_to_left() -> Result<(), Box<dyn Error>> {
        let selections = SelectionSet::new(
            "alpha beta",
            [ByteSelection::caret(0), ByteSelection::caret(6)],
            1,
        )?;
        let result = selections.replace_all("alpha beta", "X");
        assert_eq!(result.text, "Xalpha Xbeta");
        assert_eq!(result.selections.selections()[0].focus, 1);
        assert_eq!(result.selections.selections()[1].focus, 8);
        Ok(())
    }

    #[test]
    fn overlapping_ranges_are_merged_before_replacement() -> Result<(), Box<dyn Error>> {
        let selections = SelectionSet::new(
            "abcdef",
            [ByteSelection::new(1, 4), ByteSelection::new(3, 5)],
            0,
        )?;
        let result = selections.replace_all("abcdef", "X");
        assert_eq!(result.text, "aXf");
        assert_eq!(result.edits.len(), 1);
        Ok(())
    }

    #[test]
    fn deletion_respects_extended_graphemes() {
        let text = "A👨‍👩‍👧‍👦B";
        let caret = text.len().saturating_sub(1);
        let selections = SelectionSet::single(ByteSelection::caret(caret));
        let result = selections.delete_backward(text);
        assert_eq!(result.text, "AB");
    }

    #[test]
    fn vertical_cursor_preserves_byte_column() {
        let mut selections = SelectionSet::single(ByteSelection::caret(2));
        assert!(selections.add_cursor_vertical("abcd\nxy\n1234", 1));
        assert_eq!(selections.selections()[1], ByteSelection::caret(7));
    }
}
