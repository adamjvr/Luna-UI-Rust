// SPDX-License-Identifier: MPL-2.0

use crate::RasterImage;
use luna_core::{PointI, RectI};
use luna_theme::Rgba8;

/// One immutable backend-neutral paint operation.
#[derive(Clone, Debug, Eq, PartialEq)]
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
    /// Alpha-composite an immutable BGRA8 image.
    DrawImage {
        /// Logical destination origin. The image's logical size matches its pixel size.
        origin: PointI,
        /// Immutable source pixels.
        image: RasterImage,
        /// Optional logical clip rectangle.
        clip: Option<RectI>,
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

    /// Appends an immutable raster image.
    pub fn draw_image(&mut self, origin: PointI, image: RasterImage) {
        if !image.size().is_empty() {
            self.commands.push(DisplayCommand::DrawImage {
                origin,
                image,
                clip: None,
            });
        }
    }

    /// Appends an immutable raster image clipped to a logical rectangle.
    pub fn draw_image_clipped(&mut self, origin: PointI, image: RasterImage, clip: RectI) {
        if !image.size().is_empty() && !clip.is_empty() {
            self.commands.push(DisplayCommand::DrawImage {
                origin,
                image,
                clip: Some(clip),
            });
        }
    }

    /// Appends cloned commands from another immutable display-list snapshot.
    ///
    /// This is used by scene assembly when multiple reusable widgets contribute paint to one
    /// frame. Painter order is preserved exactly: existing commands remain first, followed by the
    /// supplied commands.
    pub fn extend(&mut self, other: &Self) {
        self.commands.extend(other.commands.iter().cloned());
    }

    /// Moves every command from `other` onto the end of this display list.
    pub fn append(&mut self, other: &mut Self) {
        self.commands.append(&mut other.commands);
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
