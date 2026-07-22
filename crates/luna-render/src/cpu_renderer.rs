// SPDX-License-Identifier: MPL-2.0

use crate::{DisplayCommand, DisplayList, Framebuffer};

/// Safe, deterministic reference renderer for tests and fallback operation.
///
/// The production GPU renderer will be a separate backend consuming the same display list. This
/// renderer is intentionally boring: it provides an executable specification for command order,
/// clipping, and pixel output.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CpuRenderer;

impl CpuRenderer {
    /// Executes every command in painter's order.
    pub fn render(display_list: &DisplayList, framebuffer: &mut Framebuffer) {
        for command in display_list.commands() {
            match *command {
                DisplayCommand::Clear(color) => framebuffer.clear(color),
                DisplayCommand::FillRect { bounds, color } => {
                    framebuffer.fill_rect(bounds, color);
                }
            }
        }
    }
}
