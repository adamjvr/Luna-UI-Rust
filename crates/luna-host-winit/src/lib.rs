// SPDX-License-Identifier: MPL-2.0

//! Native desktop host for Luna UI Rust.
//!
//! This leaf crate translates winit lifecycle/input events, presents Luna display lists through
//! softbuffer, performs logical/physical DPI conversion, and connects Luna semantic trees to
//! AccessKit. Core widgets and applications remain free of native platform types.

mod host;
mod input;

pub use host::{
    AccessibilityActionKind, AccessibilityActionRequest, ApplicationError, HostControl, HostError,
    NativeApplication, WindowConfig, run_native,
};
pub use input::WinitInputTranslator;
