// SPDX-License-Identifier: MPL-2.0

//! Headless Luna UI Rust M0 proof application.
//!
//! The program emits a binary PPM image without windowing or image-codec dependencies. That keeps
//! the first milestone deterministic and makes it runnable in CI while the winit/wgpu host is
//! developed as the next isolated layer.

use luna_core::{NodeId, RectI, SizeI};
use luna_host_core::{FrameRuntime, InvalidationReason};
use luna_render::{CpuRenderer, Framebuffer};
use luna_theme::Theme;
use luna_ui::{DemoPanel, UiFrame};
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let output_path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("luna-ui-rust-m0.ppm"));

    let size = SizeI::new(960, 540);
    let theme = Theme::luna_dark();
    let panel = DemoPanel::new(
        NodeId::new("demo.panel")?,
        RectI::new(160, 110, 640, 320),
        theme,
        "Luna UI Rust M0 proof panel",
    );

    let mut runtime = FrameRuntime::new();
    runtime.request_frame(InvalidationReason::Explicit("headless-demo".to_owned()));

    let frame_token = runtime.begin_frame(1_000_000).ok_or_else(|| {
        std::io::Error::other("frame runtime did not produce the requested initial frame")
    })?;

    let frame = UiFrame::build(&panel, theme.background)?;
    let mut framebuffer = Framebuffer::new(size)?;
    CpuRenderer::render(&frame.display_list, &mut framebuffer);
    write_ppm(&output_path, &framebuffer)?;

    let stats = FrameRuntime::finish_frame(&frame_token, 1_000_725);
    println!(
        "wrote {} ({}x{}, frame {}, {} µs, {} accessibility node(s))",
        output_path.display(),
        size.width,
        size.height,
        stats.frame_number,
        stats.elapsed_micros,
        frame.accessibility_tree.nodes().count()
    );

    Ok(())
}

fn write_ppm(path: &Path, framebuffer: &Framebuffer) -> Result<(), Box<dyn Error>> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    let size = framebuffer.size();

    write!(writer, "P6\n{} {}\n255\n", size.width, size.height)?;

    // The reference framebuffer is BGRA8. PPM stores RGB, so alpha is intentionally discarded.
    for pixel in framebuffer.bytes().chunks_exact(4) {
        let blue = pixel[0];
        let green = pixel[1];
        let red = pixel[2];
        writer.write_all(&[red, green, blue])?;
    }

    writer.flush()?;
    Ok(())
}
