// SPDX-License-Identifier: MPL-2.0

//! Product-neutral widget composition for Luna UI Rust.
//!
//! The central invariant is deliberately explicit: a widget's paint, hit testing, and
//! accessibility semantics derive from the same deterministic geometry. Product behavior remains
//! in applications such as Moth Text; Luna supplies reusable UI anatomy and runtime contracts.

mod demo_panel;
mod frame;
mod text_view;
mod widget;
mod workspace_demo;

pub use demo_panel::DemoPanel;
pub use frame::{UiFrame, UiFrameError};
pub use text_view::{TextView, TextViewStyle};
pub use widget::Widget;
pub use workspace_demo::{WorkspaceDemo, WorkspaceDemoLayout, WorkspaceDemoState};
