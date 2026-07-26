// SPDX-License-Identifier: MPL-2.0

use crate::SelectionSet;

/// Complete text and selection state captured at one transaction boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistorySnapshot {
    /// UTF-8 document text.
    pub text: String,
    /// Active directional selections.
    pub selections: SelectionSet,
}

impl HistorySnapshot {
    /// Creates one immutable history snapshot.
    #[must_use]
    pub fn new(text: impl Into<String>, selections: SelectionSet) -> Self {
        Self {
            text: text.into(),
            selections,
        }
    }
}

/// Semantic grouping used when coalescing adjacent edits.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EditGroup {
    /// Consecutive committed text insertion.
    Typing,
    /// Consecutive backward or forward deletion.
    Deletion,
    /// Explicit replacement, completion, or search operation.
    Replacement,
    /// Input-method composition commit.
    Ime,
    /// Discrete application or accessibility command.
    Command,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HistoryEntry {
    before: HistorySnapshot,
    after: HistorySnapshot,
    group: EditGroup,
}

/// Bounded undo/redo history with deterministic transaction coalescing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditHistory {
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    maximum_entries: usize,
    saved_fingerprint: Option<u64>,
}

impl EditHistory {
    /// Creates an empty history retaining at most `maximum_entries` undo transactions.
    #[must_use]
    pub fn new(maximum_entries: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            maximum_entries,
            saved_fingerprint: None,
        }
    }

    /// Returns whether an undo transaction is available.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Returns whether a redo transaction is available.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Returns the current undo depth.
    #[must_use]
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Returns the current redo depth.
    #[must_use]
    pub fn redo_depth(&self) -> usize {
        self.redo.len()
    }

    /// Removes all recorded transactions and saved-checkpoint state.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.saved_fingerprint = None;
    }

    /// Records one completed edit transaction.
    ///
    /// Adjacent typing or deletion transactions coalesce while the previous after-state exactly
    /// matches the new before-state. Replacement, IME, and command transactions remain discrete.
    pub fn record(&mut self, before: HistorySnapshot, after: HistorySnapshot, group: EditGroup) {
        if before == after || self.maximum_entries == 0 {
            return;
        }
        let can_coalesce = matches!(group, EditGroup::Typing | EditGroup::Deletion)
            && self
                .undo
                .last()
                .is_some_and(|entry| entry.group == group && entry.after == before);
        if can_coalesce {
            if let Some(entry) = self.undo.last_mut() {
                entry.after = after;
            }
        } else {
            self.undo.push(HistoryEntry {
                before,
                after,
                group,
            });
            if self.undo.len() > self.maximum_entries {
                let overflow = self.undo.len().saturating_sub(self.maximum_entries);
                self.undo.drain(..overflow);
            }
        }
        self.redo.clear();
    }

    /// Restores the previous transaction boundary.
    pub fn undo(&mut self, current: &HistorySnapshot) -> Option<HistorySnapshot> {
        let entry = self.undo.pop()?;
        let redo_after = if entry.after == *current {
            entry.after.clone()
        } else {
            current.clone()
        };
        let result = entry.before.clone();
        self.redo.push(HistoryEntry {
            before: result.clone(),
            after: redo_after,
            group: entry.group,
        });
        Some(result)
    }

    /// Reapplies the next transaction boundary.
    pub fn redo(&mut self, current: &HistorySnapshot) -> Option<HistorySnapshot> {
        let entry = self.redo.pop()?;
        let undo_before = if entry.before == *current {
            entry.before.clone()
        } else {
            current.clone()
        };
        let result = entry.after.clone();
        self.undo.push(HistoryEntry {
            before: undo_before,
            after: result.clone(),
            group: entry.group,
        });
        Some(result)
    }

    /// Marks the supplied state as the saved checkpoint.
    pub fn mark_saved(&mut self, snapshot: &HistorySnapshot) {
        self.saved_fingerprint = Some(snapshot_fingerprint(snapshot));
    }

    /// Returns whether `snapshot` matches the most recently marked saved checkpoint.
    #[must_use]
    pub fn is_at_saved_checkpoint(&self, snapshot: &HistorySnapshot) -> bool {
        self.saved_fingerprint == Some(snapshot_fingerprint(snapshot))
    }
}

impl Default for EditHistory {
    fn default() -> Self {
        Self::new(512)
    }
}

fn snapshot_fingerprint(snapshot: &HistorySnapshot) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in snapshot.text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{EditGroup, EditHistory, HistorySnapshot};
    use crate::{ByteSelection, SelectionSet};

    fn snapshot(text: &str, caret: usize) -> HistorySnapshot {
        HistorySnapshot::new(text, SelectionSet::single(ByteSelection::caret(caret)))
    }

    #[test]
    fn typing_coalesces_and_redo_round_trips() {
        let mut history = EditHistory::new(16);
        history.record(snapshot("", 0), snapshot("a", 1), EditGroup::Typing);
        history.record(snapshot("a", 1), snapshot("ab", 2), EditGroup::Typing);
        assert_eq!(history.undo_depth(), 1);

        let undone = history.undo(&snapshot("ab", 2));
        assert_eq!(undone, Some(snapshot("", 0)));
        let redone = history.redo(&snapshot("", 0));
        assert_eq!(redone, Some(snapshot("ab", 2)));
    }

    #[test]
    fn replacement_stays_discrete_and_saved_checkpoint_tracks_state() {
        let mut history = EditHistory::new(16);
        let before = snapshot("cat", 3);
        let after = snapshot("dog", 3);
        history.mark_saved(&before);
        let moved_caret = snapshot("cat", 0);
        assert!(history.is_at_saved_checkpoint(&moved_caret));
        history.record(before.clone(), after.clone(), EditGroup::Replacement);
        assert!(history.is_at_saved_checkpoint(&before));
        assert!(!history.is_at_saved_checkpoint(&after));
        assert_eq!(history.undo(&after), Some(before));
    }
}
