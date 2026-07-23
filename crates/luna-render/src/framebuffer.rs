// SPDX-License-Identifier: MPL-2.0

use crate::{RasterImage, RasterImageError};
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

    /// Converts the framebuffer into an immutable raster-image snapshot without copying pixels.
    pub fn into_raster_image(self) -> Result<RasterImage, RasterImageError> {
        RasterImage::new(self.size, self.bytes)
    }

    /// Fills the complete framebuffer.
    pub fn clear(&mut self, color: Rgba8) {
        for pixel in self.bytes.chunks_exact_mut(BYTES_PER_PIXEL) {
            write_bgra(pixel, color);
        }
    }

    /// Fills the clipped intersection of `bounds` and the framebuffer.
    pub fn fill_rect(&mut self, bounds: RectI, color: Rgba8) {
        self.for_each_pixel_mut(bounds, |pixel| write_bgra(pixel, color));
    }

    /// Alpha-composites a solid color over the clipped rectangle.
    pub fn blend_rect(&mut self, bounds: RectI, color: Rgba8) {
        self.for_each_pixel_mut(bounds, |pixel| blend_bgra(pixel, color));
    }

    /// Alpha-composites an image into an arbitrary destination rectangle using nearest sampling.
    ///
    /// The destination rectangle is already expressed in physical pixels by the renderer. Source
    /// coordinates are derived with integer arithmetic and every read/write remains bounds checked.
    pub fn blend_image(&mut self, destination: RectI, image: &RasterImage) {
        self.blend_image_clipped(destination, image, None);
    }

    /// Alpha-composites an image while respecting an additional physical clip rectangle.
    pub fn blend_image_clipped(
        &mut self,
        destination: RectI,
        image: &RasterImage,
        clip: Option<RectI>,
    ) {
        if destination.is_empty() || image.size().is_empty() {
            return;
        }
        let framebuffer_bounds = RectI::new(0, 0, self.size.width, self.size.height);
        let target = match clip {
            Some(value) => {
                let Some(clipped_target) = value.intersection(framebuffer_bounds) else {
                    return;
                };
                clipped_target
            }
            None => framebuffer_bounds,
        };
        let Some(clipped) = destination.intersection(target) else {
            return;
        };
        let framebuffer_width = usize::try_from(self.size.width).unwrap_or(0);
        let source_width = usize::try_from(image.size().width).unwrap_or(0);
        let source_height = usize::try_from(image.size().height).unwrap_or(0);
        let destination_width = usize::try_from(destination.width).unwrap_or(0);
        let destination_height = usize::try_from(destination.height).unwrap_or(0);
        if source_width == 0
            || source_height == 0
            || destination_width == 0
            || destination_height == 0
        {
            return;
        }

        let start_x = usize::try_from(clipped.x).unwrap_or(0);
        let start_y = usize::try_from(clipped.y).unwrap_or(0);
        let end_x = start_x.saturating_add(usize::try_from(clipped.width).unwrap_or(0));
        let end_y = start_y.saturating_add(usize::try_from(clipped.height).unwrap_or(0));
        let destination_x = i64::from(destination.x);
        let destination_y = i64::from(destination.y);

        for output_y in start_y..end_y {
            let relative_y = i64::try_from(output_y)
                .unwrap_or(i64::MAX)
                .saturating_sub(destination_y);
            let source_y = usize::try_from(relative_y.max(0))
                .unwrap_or(0)
                .saturating_mul(source_height)
                / destination_height;
            for output_x in start_x..end_x {
                let relative_x = i64::try_from(output_x)
                    .unwrap_or(i64::MAX)
                    .saturating_sub(destination_x);
                let source_x = usize::try_from(relative_x.max(0))
                    .unwrap_or(0)
                    .saturating_mul(source_width)
                    / destination_width;
                let source_index = source_y
                    .min(source_height.saturating_sub(1))
                    .saturating_mul(source_width)
                    .saturating_add(source_x.min(source_width.saturating_sub(1)))
                    .saturating_mul(BYTES_PER_PIXEL);
                let output_index = output_y
                    .saturating_mul(framebuffer_width)
                    .saturating_add(output_x)
                    .saturating_mul(BYTES_PER_PIXEL);
                let Some(source) = image
                    .bytes()
                    .get(source_index..source_index.saturating_add(BYTES_PER_PIXEL))
                else {
                    continue;
                };
                let Some(output) = self
                    .bytes
                    .get_mut(output_index..output_index.saturating_add(BYTES_PER_PIXEL))
                else {
                    continue;
                };
                blend_bgra_slice(output, source);
            }
        }
    }

    fn for_each_pixel_mut(&mut self, bounds: RectI, mut operation: impl FnMut(&mut [u8])) {
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
                let Some(pixel) = self
                    .bytes
                    .get_mut(byte_index..byte_index.saturating_add(BYTES_PER_PIXEL))
                else {
                    continue;
                };
                operation(pixel);
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

fn blend_bgra(pixel: &mut [u8], color: Rgba8) {
    blend_channels(pixel, [color.blue, color.green, color.red, color.alpha]);
}

fn blend_bgra_slice(pixel: &mut [u8], source: &[u8]) {
    if let [blue, green, red, alpha] = source {
        blend_channels(pixel, [*blue, *green, *red, *alpha]);
    }
}

fn blend_channels(destination: &mut [u8], source: [u8; 4]) {
    let [source_blue, source_green, source_red, source_alpha] = source;
    if let [blue, green, red, destination_alpha] = destination {
        let source_alpha = u32::from(source_alpha);
        let destination_alpha_u32 = u32::from(*destination_alpha);
        let inverse_source_alpha = u32::from(u8::MAX).saturating_sub(source_alpha);

        // Keep framebuffer pixels in straight-alpha form. This matters for intermediate transparent
        // images such as glyph snapshots: storing premultiplied color here and multiplying it again
        // during final composition would make antialiased text visibly too dark.
        let output_alpha_numerator = source_alpha
            .saturating_mul(255)
            .saturating_add(destination_alpha_u32.saturating_mul(inverse_source_alpha));
        if output_alpha_numerator == 0 {
            *blue = 0;
            *green = 0;
            *red = 0;
            *destination_alpha = 0;
            return;
        }

        *blue = blend_straight_alpha_channel(
            source_blue,
            *blue,
            source_alpha,
            destination_alpha_u32,
            inverse_source_alpha,
            output_alpha_numerator,
        );
        *green = blend_straight_alpha_channel(
            source_green,
            *green,
            source_alpha,
            destination_alpha_u32,
            inverse_source_alpha,
            output_alpha_numerator,
        );
        *red = blend_straight_alpha_channel(
            source_red,
            *red,
            source_alpha,
            destination_alpha_u32,
            inverse_source_alpha,
            output_alpha_numerator,
        );
        let rounded_alpha = output_alpha_numerator.saturating_add(127) / 255;
        *destination_alpha = u8::try_from(rounded_alpha.min(255)).unwrap_or(u8::MAX);
    }
}

fn blend_straight_alpha_channel(
    source: u8,
    destination: u8,
    source_alpha: u32,
    destination_alpha: u32,
    inverse_source_alpha: u32,
    output_alpha_numerator: u32,
) -> u8 {
    let numerator = u32::from(source)
        .saturating_mul(source_alpha)
        .saturating_mul(255)
        .saturating_add(
            u32::from(destination)
                .saturating_mul(destination_alpha)
                .saturating_mul(inverse_source_alpha),
        );
    let rounded = numerator.saturating_add(output_alpha_numerator / 2) / output_alpha_numerator;
    u8::try_from(rounded.min(255)).unwrap_or(u8::MAX)
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
    use crate::RasterImage;
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

    #[test]
    fn translucent_rectangles_alpha_composite() -> Result<(), Box<dyn Error>> {
        let mut framebuffer = Framebuffer::new(SizeI::new(1, 1))?;
        framebuffer.clear(Rgba8::opaque(0, 0, 0));
        framebuffer.blend_rect(RectI::new(0, 0, 1, 1), Rgba8::new(200, 100, 50, 128));

        assert_eq!(&framebuffer.bytes()[0..4], &[25, 50, 100, 255]);
        Ok(())
    }

    #[test]
    fn transparent_intermediate_images_keep_straight_color() -> Result<(), Box<dyn Error>> {
        let mut framebuffer = Framebuffer::new(SizeI::new(1, 1))?;
        framebuffer.blend_rect(RectI::new(0, 0, 1, 1), Rgba8::new(255, 0, 0, 128));

        assert_eq!(&framebuffer.bytes()[0..4], &[0, 0, 255, 128]);
        Ok(())
    }

    #[test]
    fn disjoint_image_clip_draws_nothing() -> Result<(), Box<dyn Error>> {
        let image = RasterImage::new(SizeI::new(1, 1), vec![0, 0, 255, 255])?;
        let mut framebuffer = Framebuffer::new(SizeI::new(2, 2))?;
        framebuffer.blend_image_clipped(
            RectI::new(0, 0, 1, 1),
            &image,
            Some(RectI::new(10, 10, 1, 1)),
        );

        assert!(framebuffer.bytes().iter().all(|byte| *byte == 0));
        Ok(())
    }

    #[test]
    fn image_composition_uses_source_alpha() -> Result<(), Box<dyn Error>> {
        let image = RasterImage::new(SizeI::new(1, 1), vec![0, 0, 255, 128])?;
        let mut framebuffer = Framebuffer::new(SizeI::new(1, 1))?;
        framebuffer.clear(Rgba8::opaque(0, 0, 0));
        framebuffer.blend_image(RectI::new(0, 0, 1, 1), &image);

        assert_eq!(&framebuffer.bytes()[0..4], &[0, 0, 128, 255]);
        Ok(())
    }
}
