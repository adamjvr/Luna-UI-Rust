// SPDX-License-Identifier: MPL-2.0

use luna_core::{RectI, SizeI};
use luna_theme::Rgba8;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

const BYTES_PER_PIXEL: usize = 4;

/// A tightly packed BGRA8 CPU framebuffer.
///
/// BGRA8 preserves compatibility with Luna's existing Swift CPU reference path. The buffer is
/// private so all mutation passes through bounds-checked methods. No unsafe code is required.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Framebuffer {
    size: SizeI,
    bytes: Vec<u8>,
}

impl Framebuffer {
    /// Allocates a zero-filled framebuffer.
    pub fn new(size: SizeI) -> Result<Self, FramebufferError> {
        let width = usize::try_from(size.width).map_err(|_| FramebufferError::SizeOverflow)?;
        let height = usize::try_from(size.height).map_err(|_| FramebufferError::SizeOverflow)?;
        let pixel_count = width
            .checked_mul(height)
            .ok_or(FramebufferError::SizeOverflow)?;
        let byte_count = pixel_count
            .checked_mul(BYTES_PER_PIXEL)
            .ok_or(FramebufferError::SizeOverflow)?;

        Ok(Self {
            size,
            bytes: vec![0; byte_count],
        })
    }

    /// Returns framebuffer dimensions.
    #[must_use]
    pub const fn size(&self) -> SizeI {
        self.size
    }

    /// Returns immutable tightly packed BGRA8 bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Fills the complete framebuffer.
    pub fn clear(&mut self, color: Rgba8) {
        for pixel in self.bytes.chunks_exact_mut(BYTES_PER_PIXEL) {
            write_bgra(pixel, color);
        }
    }

    /// Fills the clipped intersection of `bounds` and the framebuffer.
    pub fn fill_rect(&mut self, bounds: RectI, color: Rgba8) {
        let target = RectI::new(0, 0, self.size.width, self.size.height);
        let Some(clipped) = bounds.intersection(target) else {
            return;
        };

        let width = usize::try_from(self.size.width).unwrap_or(0);
        let start_x = usize::try_from(clipped.x).unwrap_or(0);
        let start_y = usize::try_from(clipped.y).unwrap_or(0);
        let end_x = start_x.saturating_add(usize::try_from(clipped.width).unwrap_or(0));
        let end_y = start_y.saturating_add(usize::try_from(clipped.height).unwrap_or(0));

        for y in start_y..end_y {
            for x in start_x..end_x {
                let pixel_index = y.saturating_mul(width).saturating_add(x);
                let byte_index = pixel_index.saturating_mul(BYTES_PER_PIXEL);
                let Some(pixel) = self.bytes.get_mut(byte_index..byte_index + BYTES_PER_PIXEL)
                else {
                    // Geometry was clipped before iteration, so this path means an internal
                    // arithmetic invariant changed. We fail closed rather than indexing blindly.
                    continue;
                };
                write_bgra(pixel, color);
            }
        }
    }
}

fn write_bgra(pixel: &mut [u8], color: Rgba8) {
    if let [blue, green, red, alpha] = pixel {
        *blue = color.blue;
        *green = color.green;
        *red = color.red;
        *alpha = color.alpha;
    }
}

/// Framebuffer allocation failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FramebufferError {
    /// Dimensions could not be represented safely as a byte allocation.
    SizeOverflow,
}

impl Display for FramebufferError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::SizeOverflow => formatter.write_str("framebuffer dimensions overflowed usize"),
        }
    }
}

impl Error for FramebufferError {}

#[cfg(test)]
mod tests {
    use super::Framebuffer;
    use luna_core::{RectI, SizeI};
    use luna_theme::Rgba8;
    use std::error::Error;

    #[test]
    fn rectangle_fill_clips_to_the_framebuffer() -> Result<(), Box<dyn Error>> {
        let mut framebuffer = Framebuffer::new(SizeI::new(2, 2))?;
        framebuffer.fill_rect(RectI::new(-1, -1, 2, 2), Rgba8::opaque(1, 2, 3));

        assert_eq!(&framebuffer.bytes()[0..4], &[3, 2, 1, 255]);
        assert_eq!(&framebuffer.bytes()[4..8], &[0, 0, 0, 0]);
        Ok(())
    }
}
