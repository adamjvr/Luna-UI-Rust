// SPDX-License-Identifier: MPL-2.0

use luna_core::RectI;
use luna_theme::Rgba8;

/// One immutable backend-neutral paint operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayCommand {
    /// Clear the complete target.
    Clear(Rgba8),
    /// Fill an axis-aligned rectangle.
    FillRect {
        /// Rectangle to fill.
        bounds: RectI,
        /// Source color.
        color: Rgba8,
    },
}

/// Ordered paint operations produced by Luna widgets.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DisplayList {
    commands: Vec<DisplayCommand>,
}

impl DisplayList {
    /// Creates an empty display list.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Appends a complete-target clear.
    pub fn clear(&mut self, color: Rgba8) {
        self.commands.push(DisplayCommand::Clear(color));
    }

    /// Appends an axis-aligned rectangle fill.
    pub fn fill_rect(&mut self, bounds: RectI, color: Rgba8) {
        if !bounds.is_empty() {
            self.commands
                .push(DisplayCommand::FillRect { bounds, color });
        }
    }

    /// Returns paint operations in painter's order.
    #[must_use]
    pub fn commands(&self) -> &[DisplayCommand] {
        &self.commands
    }

    /// Returns whether the display list contains no paint operations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
