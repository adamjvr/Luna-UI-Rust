// SPDX-License-Identifier: MPL-2.0

//! Product-neutral primitives shared by every Luna UI Rust layer.
//!
//! This crate deliberately has no platform, rendering, windowing, or application dependencies.
//! Keeping these types small and deterministic makes them safe to reuse in layout, hit testing,
//! accessibility, tests, and alternate host backends.

mod diagnostics;
mod geometry;
mod node_id;

pub use diagnostics::{Diagnostic, DiagnosticSeverity, Diagnostics};
pub use geometry::{InsetsI, PointI, RectI, SizeI};
pub use node_id::{NodeId, NodeIdError};
