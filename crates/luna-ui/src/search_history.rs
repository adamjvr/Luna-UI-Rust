// SPDX-License-Identifier: MPL-2.0

//! Bounded deterministic search-history state for find and replace surfaces.

use std::collections::VecDeque;

/// Bounded most-recent-first search history with keyboard traversal state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchHistory {
    capacity: usize,
    entries: VecDeque<String>,
    cursor: Option<usize>,
}

impl SearchHistory {
    /// Creates an empty history with at least one retained entry.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: VecDeque::new(),
            cursor: None,
        }
    }

    /// Records a non-empty query, removing older duplicates.
    pub fn record(&mut self, query: impl Into<String>) -> bool {
        let query = query.into();
        if query.is_empty() {
            self.cursor = None;
            return false;
        }
        let unchanged = self.entries.front() == Some(&query);
        self.entries.retain(|entry| entry != &query);
        self.entries.push_front(query);
        self.entries.truncate(self.capacity);
        self.cursor = None;
        !unchanged
    }

    /// Returns entries in most-recent-first order.
    #[must_use]
    pub fn entries(&self) -> impl ExactSizeIterator<Item = &str> {
        self.entries.iter().map(String::as_str)
    }

    /// Moves toward older queries and returns the selected entry.
    pub fn previous(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let next = self.cursor.map_or(0, |cursor| {
            cursor
                .saturating_add(1)
                .min(self.entries.len().saturating_sub(1))
        });
        self.cursor = Some(next);
        self.entries.get(next).map(String::as_str)
    }

    /// Moves toward newer queries, returning an empty string after the newest entry.
    pub fn next(&mut self) -> Option<&str> {
        let cursor = self.cursor?;
        if cursor == 0 {
            self.cursor = None;
            return Some("");
        }
        let next = cursor.saturating_sub(1);
        self.cursor = Some(next);
        self.entries.get(next).map(String::as_str)
    }

    /// Clears traversal while preserving retained entries.
    pub fn reset_navigation(&mut self) {
        self.cursor = None;
    }
}

impl Default for SearchHistory {
    fn default() -> Self {
        Self::new(32)
    }
}

#[cfg(test)]
mod tests {
    use super::SearchHistory;

    #[test]
    fn history_deduplicates_bounds_and_navigates_both_directions() {
        let mut history = SearchHistory::new(2);
        assert!(history.record("alpha"));
        assert!(history.record("beta"));
        assert!(history.record("alpha"));
        assert_eq!(history.entries().collect::<Vec<_>>(), vec!["alpha", "beta"]);
        assert_eq!(history.previous(), Some("alpha"));
        assert_eq!(history.previous(), Some("beta"));
        assert_eq!(history.next(), Some("alpha"));
        assert_eq!(history.next(), Some(""));
    }
}
