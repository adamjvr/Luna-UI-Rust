// SPDX-License-Identifier: MPL-2.0

use crate::{DisplayCommand, DisplayList, Framebuffer};
use luna_core::RectI;

/// Safe, deterministic reference renderer for tests and fallback operation.
///
/// The `luna-render-wgpu` backend consumes the same display list. This renderer is intentionally
/// boring: it provides an executable specification for command order,
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
        let framebuffer_bounds =
            RectI::new(0, 0, framebuffer.size().width, framebuffer.size().height);
        let mut clip_stack = vec![framebuffer_bounds];
        for command in display_list.commands() {
            match command {
                DisplayCommand::Clear(color) => framebuffer.clear(*color),
                DisplayCommand::PushClip(clip) => {
                    let physical = scale_rect(*clip, scale_factor);
                    let current = clip_stack.last().copied().unwrap_or(framebuffer_bounds);
                    let intersection = current
                        .intersection(physical)
                        .unwrap_or_else(|| RectI::new(0, 0, 0, 0));
                    clip_stack.push(intersection);
                }
                DisplayCommand::PopClip => {
                    if clip_stack.len() > 1 {
                        let _ = clip_stack.pop();
                    }
                }
                DisplayCommand::FillRect { bounds, color } => {
                    let physical = scale_rect(*bounds, scale_factor);
                    let clip = clip_stack.last().copied().unwrap_or(framebuffer_bounds);
                    if let Some(clipped) = physical.intersection(clip) {
                        framebuffer.fill_rect(clipped, *color);
                    }
                }
                DisplayCommand::DrawImage {
                    origin,
                    image,
                    clip,
                } => {
                    let logical_bounds =
                        RectI::new(origin.x, origin.y, image.size().width, image.size().height);
                    let stack_clip = clip_stack.last().copied().unwrap_or(framebuffer_bounds);
                    let physical_clip = match clip {
                        Some(value) => scale_rect(*value, scale_factor).intersection(stack_clip),
                        None => Some(stack_clip),
                    };
                    if let Some(physical_clip) = physical_clip {
                        framebuffer.blend_image_clipped(
                            scale_rect(logical_bounds, scale_factor),
                            image,
                            Some(physical_clip),
                        );
                    }
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
    fn clip_stack_intersects_nested_content() -> Result<(), Box<dyn Error>> {
        let mut list = DisplayList::new();
        list.clear(Rgba8::opaque(0, 0, 0));
        list.push_clip(RectI::new(1, 1, 2, 2));
        list.push_clip(RectI::new(2, 0, 2, 2));
        list.fill_rect(RectI::new(0, 0, 4, 4), Rgba8::opaque(10, 20, 30));
        list.pop_clip();
        list.pop_clip();
        let mut framebuffer = Framebuffer::new(SizeI::new(4, 4))?;

        CpuRenderer::render(&list, &mut framebuffer);

        let lit = (1_usize * 4 + 2) * 4;
        assert_eq!(&framebuffer.bytes()[lit..lit + 4], &[30, 20, 10, 255]);
        let dark = (1_usize * 4 + 1) * 4;
        assert_eq!(&framebuffer.bytes()[dark..dark + 4], &[0, 0, 0, 255]);
        Ok(())
    }

    #[test]
    fn empty_clip_suppresses_content_until_pop() -> Result<(), Box<dyn Error>> {
        let mut list = DisplayList::new();
        list.clear(Rgba8::opaque(0, 0, 0));
        list.push_clip(RectI::new(0, 0, 0, 0));
        list.fill_rect(RectI::new(0, 0, 2, 2), Rgba8::opaque(10, 20, 30));
        list.pop_clip();
        list.fill_rect(RectI::new(1, 1, 1, 1), Rgba8::opaque(40, 50, 60));
        let mut framebuffer = Framebuffer::new(SizeI::new(2, 2))?;

        CpuRenderer::render(&list, &mut framebuffer);

        assert_eq!(&framebuffer.bytes()[0..4], &[0, 0, 0, 255]);
        let visible = (1_usize * 2 + 1) * 4;
        assert_eq!(
            &framebuffer.bytes()[visible..visible + 4],
            &[60, 50, 40, 255]
        );
        Ok(())
    }

    #[test]
    fn disjoint_image_clip_draws_nothing() -> Result<(), Box<dyn Error>> {
        let image = RasterImage::new(SizeI::new(1, 1), vec![30, 20, 10, 255])?;
        let mut list = DisplayList::new();
        list.clear(Rgba8::opaque(0, 0, 0));
        list.draw_image_clipped(PointI::new(0, 0), image, RectI::new(1, 1, 1, 1));
        let mut framebuffer = Framebuffer::new(SizeI::new(2, 2))?;

        CpuRenderer::render(&list, &mut framebuffer);

        assert!(
            framebuffer
                .bytes()
                .chunks_exact(4)
                .all(|pixel| pixel == [0, 0, 0, 255])
        );
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
