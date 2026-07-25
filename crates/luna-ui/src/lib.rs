// SPDX-License-Identifier: MPL-2.0

//! Product-neutral widget composition for Luna UI Rust.
//!
//! The central invariant is deliberately explicit: a widget's paint, hit testing, and
//! accessibility semantics derive from the same deterministic geometry. Product behavior remains
//! in applications such as Moth Text; Luna supplies reusable UI anatomy and runtime contracts.

mod controls;
mod demo_panel;
mod dropdown_menu;
mod editor_panes;
mod editor_shell;
mod frame;
mod overlays;
mod proof_gallery;
mod text_label;
mod text_view;
mod widget;
mod workspace_demo;

pub use controls::{Button, ControlState, ProgressBar, Toggle, card_border};
pub use demo_panel::DemoPanel;
pub use dropdown_menu::{
    DropdownMenu, DropdownMenuLayout, DropdownMenuRowFrame, DropdownMenuState, MenuCommand,
    MenuDefinition, MenuItem,
};
pub use editor_panes::{
    EditorPaneSurface, EditorPaneSurfaceHit, EditorPaneSurfaceLayout, EditorPaneSurfaceState,
    PanePresentation, PaneTab, PaneTabFrame, PaneTabStripFrame, TabScrollDirection,
};
pub use editor_shell::{
    EditorShell, EditorShellHit, EditorShellLayout, EditorShellMetrics, EditorShellState,
    ShellItemFrame, ShellMenu, ShellTab, SidebarItem, SidebarItemKind,
};
pub use frame::{RetainedDisplayList, UiFrame, UiFrameError};
pub use overlays::{
    CommandPalette, CommandPaletteLayout, CommandPaletteState, CompletionItem, CompletionPopup,
    CompletionPopupLayout, CompletionPopupState, CompletionRowFrame, FindField, FindPanel,
    FindPanelLayout, FindPanelState, PaletteItem, PaletteRowFrame,
};
pub use proof_gallery::{ProofCardFrame, ProofGallery, ProofGalleryLayout, ProofGalleryState};
pub use text_label::{TextAlignment, TextLabel, TextLabelCache, TextLabelCacheStats};
pub use text_view::{TextView, TextViewStyle};
pub use widget::Widget;
pub use workspace_demo::{WorkspaceDemo, WorkspaceDemoLayout, WorkspaceDemoState};
