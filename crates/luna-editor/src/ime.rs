// SPDX-License-Identifier: MPL-2.0

use std::ops::Range;

/// Committed result produced when an active IME composition finishes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImeCommit {
    /// Range replaced in the underlying document.
    pub replacement: Range<usize>,
    /// Committed UTF-8 text.
    pub text: String,
}

/// Product-neutral pre-edit state for one focused text surface.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ImeComposition {
    preedit: String,
    selected_range: Option<Range<usize>>,
    replacement: Range<usize>,
    active: bool,
}

impl ImeComposition {
    /// Creates an inactive composition.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            preedit: String::new(),
            selected_range: None,
            replacement: 0..0,
            active: false,
        }
    }

    /// Returns whether pre-edit text is active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Returns the current pre-edit text.
    #[must_use]
    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    /// Returns the selected byte range inside the pre-edit string.
    #[must_use]
    pub fn selected_range(&self) -> Option<Range<usize>> {
        self.selected_range.clone()
    }

    /// Returns the document range that will be replaced on commit.
    #[must_use]
    pub fn replacement(&self) -> Range<usize> {
        self.replacement.clone()
    }

    /// Begins or updates a composition.
    pub fn update(
        &mut self,
        replacement: Range<usize>,
        preedit: impl Into<String>,
        selected_range: Option<Range<usize>>,
    ) {
        let preedit = preedit.into();
        let selected_range = selected_range.and_then(|range| {
            let start = snap_backward(&preedit, range.start.min(preedit.len()));
            let end = snap_backward(&preedit, range.end.min(preedit.len()));
            (start <= end).then_some(start..end)
        });
        self.replacement = replacement;
        self.preedit = preedit;
        self.selected_range = selected_range;
        self.active = true;
    }

    /// Commits text and clears the active composition.
    pub fn commit(&mut self, text: impl Into<String>) -> Option<ImeCommit> {
        if !self.active {
            return None;
        }
        let commit = ImeCommit {
            replacement: self.replacement.clone(),
            text: text.into(),
        };
        self.cancel();
        Some(commit)
    }

    /// Cancels the composition without modifying the document.
    pub fn cancel(&mut self) {
        self.preedit.clear();
        self.selected_range = None;
        self.replacement = 0..0;
        self.active = false;
    }
}

fn snap_backward(text: &str, mut offset: usize) -> usize {
    while offset > 0 && !text.is_char_boundary(offset) {
        offset = offset.saturating_sub(1);
    }
    offset
}

#[cfg(test)]
mod tests {
    use super::{ImeCommit, ImeComposition};

    #[test]
    fn composition_updates_commit_and_cancel_deterministically() {
        let mut composition = ImeComposition::new();
        composition.update(2..4, "かな", Some(0..3));
        assert!(composition.is_active());
        assert_eq!(composition.selected_range(), Some(0..3));
        assert_eq!(
            composition.commit("仮名"),
            Some(ImeCommit {
                replacement: 2..4,
                text: "仮名".to_owned(),
            })
        );
        assert!(!composition.is_active());
    }
}
