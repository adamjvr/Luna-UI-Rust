// SPDX-License-Identifier: MPL-2.0

use crate::{DisplayCommand, DisplayList, Framebuffer};
use luna_core::RectI;

/// Safe, deterministic reference renderer for tests and fallback operation.
///
/// The production GPU renderer will be a separate backend consuming the same display list. This
/// renderer is intentionally boring: it provides an executable specification for command order,
/// clipping, DPI conversion, image composition, and pixel output.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CpuRenderer;

impl CpuRenderer {
    /// Executes every command in painter's order at a one-to-one logical-to-physical scale.
    pub fn render(display_list: &DisplayList, framebuffer: &mut Framebuffer) {
        Self::render_scaled(display_list, framebuffer, 1.0);
    }

    /// Executes every command while converting logical geometry to physical pixels.
    ///
    /// Luna widget geometry remains integral and expressed in logical pixels. Native hosts pass
    /// the current window scale factor here rather than allowing platform DPI concerns to leak
    /// into layout, hit testing, or accessibility. Leading edges are rounded down and trailing
    /// edges are rounded up so non-empty logical content does not disappear at fractional scales.
    pub fn render_scaled(
        display_list: &DisplayList,
        framebuffer: &mut Framebuffer,
        scale_factor: f64,
    ) {
        let scale_factor = normalized_scale_factor(scale_factor);
        for command in display_list.commands() {
            match command {
                DisplayCommand::Clear(color) => framebuffer.clear(*color),
                DisplayCommand::FillRect { bounds, color } => {
                    framebuffer.fill_rect(scale_rect(*bounds, scale_factor), *color);
                }
                DisplayCommand::DrawImage {
                    origin,
                    image,
                    clip,
                } => {
                    let logical_bounds =
                        RectI::new(origin.x, origin.y, image.size().width, image.size().height);
                    let physical_clip = clip.map(|value| scale_rect(value, scale_factor));
                    framebuffer.blend_image_clipped(
                        scale_rect(logical_bounds, scale_factor),
                        image,
                        physical_clip,
                    );
                }
            }
        }
    }

    /// Converts one logical rectangle to the physical coverage used by the CPU renderer.
    ///
    /// Retained hosts use this exact conversion when restoring dirty regions, preserving the same
    /// floor-leading/ceil-trailing rule as ordinary display-list execution.
    #[must_use]
    pub fn scale_logical_rect(bounds: RectI, scale_factor: f64) -> RectI {
        scale_rect(bounds, normalized_scale_factor(scale_factor))
    }
}

fn normalized_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

fn scale_rect(bounds: RectI, scale_factor: f64) -> RectI {
    let left = (f64::from(bounds.x) * scale_factor).floor();
    let top = (f64::from(bounds.y) * scale_factor).floor();
    let right = ((bounds.right() as f64) * scale_factor).ceil();
    let bottom = ((bounds.bottom() as f64) * scale_factor).ceil();
    let x = clamped_i32(left);
    let y = clamped_i32(top);
    let width = clamped_u32((right - left).max(0.0));
    let height = clamped_u32((bottom - top).max(0.0));

    RectI::new(x, y, width, height)
}

fn clamped_i32(value: f64) -> i32 {
    if value <= f64::from(i32::MIN) {
        i32::MIN
    } else if value >= f64::from(i32::MAX) {
        i32::MAX
    } else {
        value as i32
    }
}

fn clamped_u32(value: f64) -> u32 {
    if value <= 0.0 {
        0
    } else if value >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        value as u32
    }
}

#[cfg(test)]
mod tests {
    use super::CpuRenderer;
    use crate::{DisplayList, Framebuffer, RasterImage};
    use luna_core::{PointI, RectI, SizeI};
    use luna_theme::Rgba8;
    use std::error::Error;

    #[test]
    fn fractional_dpi_expands_trailing_edges() -> Result<(), Box<dyn Error>> {
        let mut list = DisplayList::new();
        list.fill_rect(RectI::new(1, 1, 1, 1), Rgba8::opaque(10, 20, 30));
        let mut framebuffer = Framebuffer::new(SizeI::new(4, 4))?;

        CpuRenderer::render_scaled(&list, &mut framebuffer, 1.5);

        let width = 4_usize;
        for y in 1_usize..3 {
            for x in 1_usize..3 {
                let byte = (y * width + x) * 4;
                assert_eq!(&framebuffer.bytes()[byte..byte + 4], &[30, 20, 10, 255]);
            }
        }
        Ok(())
    }

    #[test]
    fn public_logical_rect_scaling_matches_render_coverage() {
        assert_eq!(
            CpuRenderer::scale_logical_rect(RectI::new(1, 1, 1, 1), 1.5),
            RectI::new(1, 1, 2, 2)
        );
    }

    #[test]
    fn invalid_scale_falls_back_to_one() -> Result<(), Box<dyn Error>> {
        let mut list = DisplayList::new();
        list.fill_rect(RectI::new(0, 0, 1, 1), Rgba8::opaque(1, 2, 3));
        let mut framebuffer = Framebuffer::new(SizeI::new(2, 2))?;

        CpuRenderer::render_scaled(&list, &mut framebuffer, f64::NAN);

        assert_eq!(&framebuffer.bytes()[0..4], &[3, 2, 1, 255]);
        assert_eq!(&framebuffer.bytes()[4..8], &[0, 0, 0, 0]);
        Ok(())
    }

    #[test]
    fn raster_images_scale_with_the_display_list() -> Result<(), Box<dyn Error>> {
        let image = RasterImage::new(SizeI::new(1, 1), vec![30, 20, 10, 255])?;
        let mut list = DisplayList::new();
        list.draw_image(PointI::new(1, 1), image);
        let mut framebuffer = Framebuffer::new(SizeI::new(4, 4))?;

        CpuRenderer::render_scaled(&list, &mut framebuffer, 2.0);

        let first = (2_usize * 4 + 2) * 4;
        assert_eq!(&framebuffer.bytes()[first..first + 4], &[30, 20, 10, 255]);
        Ok(())
    }
}
