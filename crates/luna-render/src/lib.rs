// SPDX-License-Identifier: MPL-2.0

//! Backend-neutral paint commands and Luna's safe CPU reference renderer.
//!
//! Widgets emit immutable display-list snapshots. A backend consumes that snapshot later. This is
//! the same architectural boundary the Swift implementation uses and is essential for testing,
//! alternate renderers, and eventual GPU submission without allowing widgets to call a graphics
//! API directly.

mod cpu_renderer;
mod display_list;
mod framebuffer;

pub use cpu_renderer::CpuRenderer;
pub use display_list::{DisplayCommand, DisplayList};
pub use framebuffer::{Framebuffer, FramebufferError};
