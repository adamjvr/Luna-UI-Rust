// SPDX-License-Identifier: MPL-2.0

//! Deterministic editor text coordinates and a compact editable document foundation.
//!
//! Luna stores durable text positions as a logical line plus a UTF-8 byte column. That mirrors
//! the Swift Luna implementation and keeps coordinates compatible with future rope or piece-table
//! storage without exposing implementation-specific iterators. Every public conversion explicitly
//! clamps and snaps to a valid UTF-8 boundary. User-visible left/right movement and deletion use
//! Unicode extended grapheme clusters, so combining marks and emoji sequences are not split.

use std::cmp::Ordering;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

/// Direction used when a requested UTF-8 byte column lands inside a scalar encoding.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SnapBias {
    /// Snap toward the beginning of the line.
    Backward,
    /// Snap toward the end of the line.
    Forward,
}

/// A stable logical position in a text document.
///
/// `utf8_column` is a byte offset inside the selected logical line, never a Unicode scalar or
/// grapheme count. Constructed values may be arbitrary; [`TextDocument::clamp_location`] is the
/// normalization boundary used before reading or mutating text.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextLocation {
    /// Zero-based logical line index.
    pub line_index: usize,
    /// Zero-based UTF-8 byte column within the line.
    pub utf8_column: usize,
}

impl TextLocation {
    /// Creates a logical text position.
    #[must_use]
    pub const fn new(line_index: usize, utf8_column: usize) -> Self {
        Self {
            line_index,
            utf8_column,
        }
    }
}

/// An anchor/focus text range that preserves selection direction.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextRange {
    /// Fixed end of an interactive selection.
    pub anchor: TextLocation,
    /// Moving end and current caret location.
    pub focus: TextLocation,
}

impl TextRange {
    /// Creates an anchor/focus range.
    #[must_use]
    pub const fn new(anchor: TextLocation, focus: TextLocation) -> Self {
        Self { anchor, focus }
    }

    /// Creates a collapsed range at one location.
    #[must_use]
    pub const fn collapsed(location: TextLocation) -> Self {
        Self::new(location, location)
    }

    /// Returns whether anchor and focus are equal.
    #[must_use]
    pub const fn is_collapsed(self) -> bool {
        self.anchor.line_index == self.focus.line_index
            && self.anchor.utf8_column == self.focus.utf8_column
    }

    /// Returns the range ordered from document start to document end.
    #[must_use]
    pub fn normalized(self) -> Self {
        if compare_locations(self.anchor, self.focus) == Ordering::Greater {
            Self::new(self.focus, self.anchor)
        } else {
            self
        }
    }
}

/// One immutable logical line and its absolute byte range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextLine {
    /// Zero-based line index.
    pub index: usize,
    /// Line content excluding the terminating line-feed byte.
    pub text: String,
    /// Absolute UTF-8 byte offset at which this line begins.
    pub utf8_offset: usize,
    /// UTF-8 byte length excluding the terminating line-feed byte.
    pub utf8_length: usize,
}

impl TextLine {
    /// Returns the one-based line number used by gutters and status displays.
    #[must_use]
    pub const fn line_number(&self) -> usize {
        self.index.saturating_add(1)
    }

    /// Returns the absolute half-open byte range occupied by this line's content.
    #[must_use]
    pub const fn absolute_range(&self) -> std::ops::Range<usize> {
        self.utf8_offset..self.utf8_offset.saturating_add(self.utf8_length)
    }
}

/// Immutable plain-text snapshot with stable UTF-8 line metadata.
///
/// Clones share both the UTF-8 storage and indexed line table. Editing replaces the complete
/// snapshot, while frame construction and retained layout caches can clone documents cheaply.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextDocument {
    text: Arc<str>,
    lines: Arc<[TextLine]>,
}

impl TextDocument {
    /// Creates a snapshot and indexes every logical line.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        let text = Arc::<str>::from(text.into());
        let lines = Arc::<[TextLine]>::from(make_lines(text.as_ref()));
        Self { text, lines }
    }

    /// Returns the complete UTF-8 text.
    #[must_use]
    pub fn text(&self) -> &str {
        self.text.as_ref()
    }

    /// Returns all logical lines.
    #[must_use]
    pub fn lines(&self) -> &[TextLine] {
        self.lines.as_ref()
    }

    /// Returns the number of logical lines. Even an empty document has one line.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Returns one logical line.
    #[must_use]
    pub fn line(&self, index: usize) -> Option<&TextLine> {
        self.lines.get(index)
    }

    /// Returns the final valid insertion location.
    #[must_use]
    pub fn end_location(&self) -> TextLocation {
        let line_index = self.lines.len().saturating_sub(1);
        let utf8_column = self
            .lines
            .get(line_index)
            .map_or(0, |line| line.utf8_length);
        TextLocation::new(line_index, utf8_column)
    }

    /// Clamps a location to a valid line and UTF-8 scalar boundary.
    #[must_use]
    pub fn clamp_location(&self, location: TextLocation, bias: SnapBias) -> TextLocation {
        let line_index = location.line_index.min(self.lines.len().saturating_sub(1));
        let Some(line) = self.lines.get(line_index) else {
            return TextLocation::default();
        };
        TextLocation::new(
            line_index,
            snap_byte_boundary(&line.text, location.utf8_column, bias),
        )
    }

    /// Clamps both endpoints while retaining anchor/focus direction.
    #[must_use]
    pub fn clamp_range(&self, range: TextRange) -> TextRange {
        TextRange::new(
            self.clamp_location(range.anchor, SnapBias::Backward),
            self.clamp_location(range.focus, SnapBias::Forward),
        )
    }

    /// Converts a logical position to an absolute UTF-8 byte offset.
    #[must_use]
    pub fn absolute_offset(&self, location: TextLocation, bias: SnapBias) -> usize {
        let location = self.clamp_location(location, bias);
        self.lines.get(location.line_index).map_or(0, |line| {
            line.utf8_offset.saturating_add(location.utf8_column)
        })
    }

    /// Converts an absolute UTF-8 byte offset into a logical position.
    ///
    /// An offset on a line-feed byte maps to the end of the preceding line. An offset after the
    /// line feed maps to the beginning of the following line.
    #[must_use]
    pub fn location_for_offset(&self, offset: usize, bias: SnapBias) -> TextLocation {
        let offset = snap_byte_boundary(self.text.as_ref(), offset, bias);
        for line in self.lines.as_ref() {
            let end = line.utf8_offset.saturating_add(line.utf8_length);
            if offset <= end {
                return TextLocation::new(line.index, offset.saturating_sub(line.utf8_offset));
            }
        }
        self.end_location()
    }

    /// Returns the normalized absolute byte range corresponding to a logical range.
    #[must_use]
    pub fn absolute_range(&self, range: TextRange) -> std::ops::Range<usize> {
        let normalized = self.clamp_range(range).normalized();
        self.absolute_offset(normalized.anchor, SnapBias::Backward)
            ..self.absolute_offset(normalized.focus, SnapBias::Forward)
    }

    /// Returns the previous extended-grapheme insertion position.
    #[must_use]
    pub fn previous_grapheme(&self, location: TextLocation) -> TextLocation {
        let location = self.clamp_location(location, SnapBias::Backward);
        let Some(line) = self.lines.get(location.line_index) else {
            return TextLocation::default();
        };
        if location.utf8_column > 0 {
            let previous = line
                .text
                .grapheme_indices(true)
                .map(|(index, _)| index)
                .take_while(|index| *index < location.utf8_column)
                .last()
                .unwrap_or(0);
            return TextLocation::new(location.line_index, previous);
        }
        if location.line_index == 0 {
            return location;
        }
        let previous_line = location.line_index.saturating_sub(1);
        let previous_column = self
            .lines
            .get(previous_line)
            .map_or(0, |value| value.utf8_length);
        TextLocation::new(previous_line, previous_column)
    }

    /// Returns the next extended-grapheme insertion position.
    #[must_use]
    pub fn next_grapheme(&self, location: TextLocation) -> TextLocation {
        let location = self.clamp_location(location, SnapBias::Forward);
        let Some(line) = self.lines.get(location.line_index) else {
            return self.end_location();
        };
        if location.utf8_column < line.utf8_length {
            let next = line
                .text
                .grapheme_indices(true)
                .find_map(|(index, grapheme)| {
                    (index >= location.utf8_column).then_some(index.saturating_add(grapheme.len()))
                })
                .unwrap_or(line.utf8_length);
            return TextLocation::new(location.line_index, next);
        }
        if location.line_index.saturating_add(1) < self.lines.len() {
            TextLocation::new(location.line_index.saturating_add(1), 0)
        } else {
            location
        }
    }

    /// Returns every extended-grapheme insertion boundary for one line, including both edges.
    #[must_use]
    pub fn grapheme_boundaries(&self, line_index: usize) -> Vec<usize> {
        let Some(line) = self.lines.get(line_index) else {
            return Vec::new();
        };
        let mut boundaries = line
            .text
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if boundaries.first().copied() != Some(0) {
            boundaries.insert(0, 0);
        }
        if boundaries.last().copied() != Some(line.utf8_length) {
            boundaries.push(line.utf8_length);
        }
        boundaries
    }
}

impl Default for TextDocument {
    fn default() -> Self {
        Self::new(String::new())
    }
}

/// Result of one document mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextEditResult {
    /// Logical range replaced in the pre-edit document.
    pub old_range: TextRange,
    /// Text inserted in place of the old range.
    pub inserted_text: String,
    /// New caret position.
    pub new_caret: TextLocation,
    /// Whether the document bytes changed.
    pub did_change: bool,
}

/// Mutable single-caret editing state used to prove Luna's input pipeline.
///
/// This deliberately rebuilds the immutable [`TextDocument`] line snapshot after each edit. It is
/// not the final large-file storage engine; the stable coordinate and edit contracts are designed
/// so a rope or piece table can replace the internal `String` later.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditableText {
    document: TextDocument,
    caret: TextLocation,
    selection: Option<TextRange>,
    edit_revision: u64,
    preferred_utf8_column: Option<usize>,
}

impl EditableText {
    /// Creates editing state at the beginning of `text`.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            document: TextDocument::new(text),
            caret: TextLocation::default(),
            selection: None,
            edit_revision: 0,
            preferred_utf8_column: None,
        }
    }

    /// Returns the immutable document snapshot.
    #[must_use]
    pub const fn document(&self) -> &TextDocument {
        &self.document
    }

    /// Returns the active caret.
    #[must_use]
    pub const fn caret(&self) -> TextLocation {
        self.caret
    }

    /// Returns the active non-collapsed selection.
    #[must_use]
    pub const fn selection(&self) -> Option<TextRange> {
        self.selection
    }

    /// Returns the monotonically increasing edit revision.
    #[must_use]
    pub const fn edit_revision(&self) -> u64 {
        self.edit_revision
    }

    /// Replaces the shared document snapshot while preserving this view's caret and selection.
    ///
    /// Split-pane applications use this after another view edits the same logical buffer. The
    /// supplied revision must be the shared buffer revision, not a pane-local presentation token.
    pub fn synchronize_document(&mut self, text: impl Into<String>, edit_revision: u64) {
        let caret = self.caret;
        let selection = self.selection;
        self.document = TextDocument::new(text);
        self.caret = self.document.clamp_location(caret, SnapBias::Backward);
        self.selection = selection
            .map(|range| self.document.clamp_range(range))
            .filter(|range| !range.is_collapsed());
        if let Some(selection) = self.selection {
            self.caret = selection.focus;
        }
        self.edit_revision = edit_revision;
        self.preferred_utf8_column = None;
    }

    /// Sets a collapsed caret and clears selection.
    pub fn set_caret(&mut self, location: TextLocation) {
        self.caret = self.document.clamp_location(location, SnapBias::Backward);
        self.selection = None;
        self.preferred_utf8_column = None;
    }

    /// Sets an anchor/focus selection. Collapsed selections are normalized to a plain caret.
    pub fn set_selection(&mut self, range: TextRange) {
        let range = self.document.clamp_range(range);
        self.caret = range.focus;
        self.selection = (!range.is_collapsed()).then_some(range);
        self.preferred_utf8_column = None;
    }

    /// Replaces the current selection, or inserts at the caret when no selection exists.
    pub fn insert_text(&mut self, text: &str) -> TextEditResult {
        let range = self.selection.unwrap_or(TextRange::collapsed(self.caret));
        self.replace_range(range, text)
    }

    /// Inserts one line feed.
    pub fn insert_newline(&mut self) -> TextEditResult {
        self.insert_text("\n")
    }

    /// Deletes the selection or the preceding extended grapheme cluster.
    pub fn delete_backward(&mut self) -> TextEditResult {
        if let Some(selection) = self.selection {
            return self.replace_range(selection, "");
        }
        let previous = self.document.previous_grapheme(self.caret);
        self.replace_range(TextRange::new(previous, self.caret), "")
    }

    /// Deletes the selection or the following extended grapheme cluster.
    pub fn delete_forward(&mut self) -> TextEditResult {
        if let Some(selection) = self.selection {
            return self.replace_range(selection, "");
        }
        let next = self.document.next_grapheme(self.caret);
        self.replace_range(TextRange::new(self.caret, next), "")
    }

    /// Moves one logical grapheme backward, optionally extending selection.
    pub fn move_backward(&mut self, extending_selection: bool) {
        if !extending_selection && let Some(selection) = self.selection.take() {
            self.caret = selection.normalized().anchor;
            self.preferred_utf8_column = None;
            return;
        }
        let next = self.document.previous_grapheme(self.caret);
        self.apply_movement(next, extending_selection);
    }

    /// Moves one logical grapheme forward, optionally extending selection.
    pub fn move_forward(&mut self, extending_selection: bool) {
        if !extending_selection && let Some(selection) = self.selection.take() {
            self.caret = selection.normalized().focus;
            self.preferred_utf8_column = None;
            return;
        }
        let next = self.document.next_grapheme(self.caret);
        self.apply_movement(next, extending_selection);
    }

    /// Moves to the beginning of the current logical line.
    pub fn move_to_line_start(&mut self, extending_selection: bool) {
        self.apply_movement(
            TextLocation::new(self.caret.line_index, 0),
            extending_selection,
        );
    }

    /// Moves to the end of the current logical line.
    pub fn move_to_line_end(&mut self, extending_selection: bool) {
        let column = self
            .document
            .line(self.caret.line_index)
            .map_or(0, |line| line.utf8_length);
        self.apply_movement(
            TextLocation::new(self.caret.line_index, column),
            extending_selection,
        );
    }

    /// Moves one logical line upward while retaining a preferred UTF-8 column.
    pub fn move_up(&mut self, extending_selection: bool) {
        let preferred = self.preferred_utf8_column.unwrap_or(self.caret.utf8_column);
        self.preferred_utf8_column = Some(preferred);
        let line = self.caret.line_index.saturating_sub(1);
        let next = self
            .document
            .clamp_location(TextLocation::new(line, preferred), SnapBias::Backward);
        self.apply_movement_preserving_preferred(next, extending_selection);
    }

    /// Moves one logical line downward while retaining a preferred UTF-8 column.
    pub fn move_down(&mut self, extending_selection: bool) {
        let preferred = self.preferred_utf8_column.unwrap_or(self.caret.utf8_column);
        self.preferred_utf8_column = Some(preferred);
        let line = self
            .caret
            .line_index
            .saturating_add(1)
            .min(self.document.line_count().saturating_sub(1));
        let next = self
            .document
            .clamp_location(TextLocation::new(line, preferred), SnapBias::Backward);
        self.apply_movement_preserving_preferred(next, extending_selection);
    }

    fn replace_range(&mut self, range: TextRange, replacement: &str) -> TextEditResult {
        let range = self.document.clamp_range(range).normalized();
        let absolute = self.document.absolute_range(range);
        let old_text = &self.document.text()[absolute.clone()];
        let did_change = old_text != replacement;
        let start = absolute.start;
        let mut next_text = self.document.text().to_owned();
        next_text.replace_range(absolute, replacement);
        self.document = TextDocument::new(next_text);
        let new_offset = start.saturating_add(replacement.len());
        self.caret = self
            .document
            .location_for_offset(new_offset, SnapBias::Forward);
        self.selection = None;
        self.preferred_utf8_column = None;
        if did_change {
            self.edit_revision = self.edit_revision.saturating_add(1);
        }
        TextEditResult {
            old_range: range,
            inserted_text: replacement.to_owned(),
            new_caret: self.caret,
            did_change,
        }
    }

    fn apply_movement(&mut self, next: TextLocation, extending_selection: bool) {
        self.preferred_utf8_column = None;
        self.apply_movement_preserving_preferred(next, extending_selection);
    }

    fn apply_movement_preserving_preferred(
        &mut self,
        next: TextLocation,
        extending_selection: bool,
    ) {
        let next = self.document.clamp_location(next, SnapBias::Backward);
        if extending_selection {
            let anchor = self
                .selection
                .map_or(self.caret, |selection| selection.anchor);
            let range = TextRange::new(anchor, next);
            self.selection = (!range.is_collapsed()).then_some(range);
        } else {
            self.selection = None;
        }
        self.caret = next;
    }
}

impl Default for EditableText {
    fn default() -> Self {
        Self::new(String::new())
    }
}

/// Logical scroll offsets for a text viewport.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TextScroll {
    /// Horizontal logical-pixel offset.
    pub x: i32,
    /// Vertical logical-pixel offset.
    pub y: i32,
}

impl TextScroll {
    /// Creates a scroll position and clamps negative values to zero.
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self {
            x: if x < 0 { 0 } else { x },
            y: if y < 0 { 0 } else { y },
        }
    }

    /// Applies deltas using saturating arithmetic and clamps to supplied maxima.
    pub fn scroll_by(&mut self, delta_x: i32, delta_y: i32, max_x: i32, max_y: i32) {
        self.x = self.x.saturating_add(delta_x).clamp(0, max_x.max(0));
        self.y = self.y.saturating_add(delta_y).clamp(0, max_y.max(0));
    }

    /// Clamps an existing position to supplied maxima.
    pub fn clamp(&mut self, max_x: i32, max_y: i32) {
        self.x = self.x.clamp(0, max_x.max(0));
        self.y = self.y.clamp(0, max_y.max(0));
    }
}

fn compare_locations(left: TextLocation, right: TextLocation) -> Ordering {
    left.line_index
        .cmp(&right.line_index)
        .then_with(|| left.utf8_column.cmp(&right.utf8_column))
}

fn snap_byte_boundary(text: &str, requested: usize, bias: SnapBias) -> usize {
    let mut offset = requested.min(text.len());
    if text.is_char_boundary(offset) {
        return offset;
    }
    match bias {
        SnapBias::Backward => {
            while offset > 0 && !text.is_char_boundary(offset) {
                offset = offset.saturating_sub(1);
            }
        }
        SnapBias::Forward => {
            while offset < text.len() && !text.is_char_boundary(offset) {
                offset = offset.saturating_add(1);
            }
        }
    }
    offset
}

fn make_lines(text: &str) -> Vec<TextLine> {
    let mut lines = Vec::new();
    let mut start = 0_usize;
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            push_line(&mut lines, text, start, index);
            start = index.saturating_add(1);
        }
    }
    push_line(&mut lines, text, start, text.len());
    lines
}

fn push_line(lines: &mut Vec<TextLine>, text: &str, start: usize, end: usize) {
    let line_text = text.get(start..end).unwrap_or_default();
    lines.push(TextLine {
        index: lines.len(),
        text: line_text.to_owned(),
        utf8_offset: start,
        utf8_length: end.saturating_sub(start),
    });
}

#[cfg(test)]
mod tests {
    use super::{EditableText, SnapBias, TextDocument, TextLocation, TextRange, TextScroll};
    use std::sync::Arc;

    #[test]
    fn immutable_document_clones_share_snapshot_storage() {
        let document = TextDocument::new("alpha\nbeta");
        let cloned = document.clone();

        assert!(Arc::ptr_eq(&document.text, &cloned.text));
        assert!(Arc::ptr_eq(&document.lines, &cloned.lines));
    }

    #[test]
    fn swift_fixture_locations_map_to_absolute_utf8_offsets() {
        let document = TextDocument::new("alpha\nbeta\ngamma");

        assert_eq!(
            document.clamp_location(TextLocation::new(99, 99), SnapBias::Backward),
            TextLocation::new(2, 5)
        );
        assert_eq!(
            document.clamp_location(TextLocation::new(1, 99), SnapBias::Backward),
            TextLocation::new(1, 4)
        );
        assert_eq!(
            document.absolute_offset(TextLocation::new(1, 2), SnapBias::Backward),
            8
        );
        assert_eq!(
            document.location_for_offset(8, SnapBias::Backward),
            TextLocation::new(1, 2)
        );
    }

    #[test]
    fn trailing_newline_produces_an_empty_final_line() {
        let document = TextDocument::new("alpha\n");

        assert_eq!(document.line_count(), 2);
        assert_eq!(document.end_location(), TextLocation::new(1, 0));
    }

    #[test]
    fn scalar_interior_offsets_snap_without_invalid_slices() {
        let document = TextDocument::new("é");

        assert_eq!(
            document.clamp_location(TextLocation::new(0, 1), SnapBias::Backward),
            TextLocation::new(0, 0)
        );
        assert_eq!(
            document.clamp_location(TextLocation::new(0, 1), SnapBias::Forward),
            TextLocation::new(0, 2)
        );
    }

    #[test]
    fn deleting_backward_removes_one_extended_grapheme() {
        let mut state = EditableText::new("A👨‍👩‍👧‍👦B");
        state.set_caret(TextLocation::new(0, "A👨‍👩‍👧‍👦".len()));

        let result = state.delete_backward();

        assert!(result.did_change);
        assert_eq!(state.document().text(), "AB");
        assert_eq!(state.caret(), TextLocation::new(0, 1));
    }

    #[test]
    fn selection_replacement_reuses_anchor_focus_range() {
        let mut state = EditableText::new("hello world");
        state.set_selection(TextRange::new(
            TextLocation::new(0, 6),
            TextLocation::new(0, 11),
        ));

        let result = state.insert_text("Luna");

        assert!(result.did_change);
        assert_eq!(state.document().text(), "hello Luna");
        assert_eq!(state.caret(), TextLocation::new(0, 10));
        assert_eq!(state.selection(), None);
    }

    #[test]
    fn extending_and_collapsing_selection_matches_editor_behavior() {
        let mut state = EditableText::new("abcdef");
        state.set_caret(TextLocation::new(0, 3));

        state.move_forward(true);
        state.move_forward(true);
        assert_eq!(
            state.selection(),
            Some(TextRange::new(
                TextLocation::new(0, 3),
                TextLocation::new(0, 5)
            ))
        );

        state.move_backward(false);
        assert_eq!(state.selection(), None);
        assert_eq!(state.caret(), TextLocation::new(0, 3));
    }

    #[test]
    fn vertical_motion_preserves_preferred_column() {
        let mut state = EditableText::new("abcdef\nx\nabcdef");
        state.set_caret(TextLocation::new(0, 5));

        state.move_down(false);
        assert_eq!(state.caret(), TextLocation::new(1, 1));
        state.move_down(false);
        assert_eq!(state.caret(), TextLocation::new(2, 5));
    }

    #[test]
    fn synchronizing_shared_text_preserves_and_clamps_view_state() {
        let mut state = EditableText::new("alpha beta");
        state.set_selection(TextRange::new(
            TextLocation::new(0, 6),
            TextLocation::new(0, 10),
        ));

        state.synchronize_document("short", 41);

        assert_eq!(state.document().text(), "short");
        assert_eq!(state.edit_revision(), 41);
        assert_eq!(state.caret(), TextLocation::new(0, 5));
        assert_eq!(state.selection(), None);
    }

    #[test]
    fn scroll_offsets_saturate_and_clamp() {
        let mut scroll = TextScroll::new(0, 0);
        scroll.scroll_by(10, 20, 5, 15);
        assert_eq!(scroll, TextScroll::new(5, 15));
        scroll.scroll_by(-100, -100, 5, 15);
        assert_eq!(scroll, TextScroll::new(0, 0));
    }
}
