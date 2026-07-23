// SPDX-License-Identifier: MPL-2.0

//! Native M3 editor integration harness for Luna UI Rust.
//!
//! This application mirrors the purpose of Swift LunaUITestApp's default editor mode: reusable
//! shell anatomy and editor text are exercised together without embedding Moth Text product
//! policy in Luna. It provides menus, tabs, a project sidebar, status chrome, editable text,
//! command palette, find panel, dirty tracking, and accessibility from shared geometry.
//!
//! Shortcuts: Control-P command palette, Control-F find, Control-S save, Control-N new document,
//! Control-B sidebar, Control-W close tab, Control-A select all, Escape closes overlays or exits.

use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{InsetsI, NodeId, PointI, RectI, SizeI};
use luna_host_winit::{
    AccessibilityActionKind, AccessibilityActionRequest, ApplicationError, HostControl,
    NativeApplication, WindowConfig, run_native,
};
use luna_input::{InputEvent, Key, Modifiers, NamedKey, PointerButton, PointerEventKind};
use luna_render::DisplayList;
use luna_text::{EditableText, SnapBias, TextLocation, TextRange, TextScroll};
use luna_text_cosmic::{TextEngine, TextLayoutRequest, TextLayoutSnapshot};
use luna_theme::Theme;
use luna_ui::{
    CommandPalette, CommandPaletteState, EditorShell, EditorShellHit, EditorShellMetrics,
    EditorShellState, FindField, FindPanel, FindPanelState, PaletteItem, ShellTab, SidebarItem,
    TextAlignment, TextLabel, TextView, TextViewStyle, UiFrame, Widget,
};
use std::error::Error;
use std::ops::Range;

const ROOT_ID: &str = "m3-editor-window";
const SHELL_ID: &str = "m3-editor-shell";
const TEXT_ID: &str = "m3-editor-text";
const PALETTE_ID: &str = "m3-editor-palette";
const FIND_ID: &str = "m3-editor-find";

const README_TEXT: &str = "# Luna UI Rust\n\nM3 adds separate proof-gallery and editor integration demos.\n\n- Deterministic shell geometry\n- Editor-grade text shaping\n- Tabs, sidebar, status, quick panel, and find overlay\n- Shared accessibility semantics\n\nPress Control-P to inspect commands.\n";
const EDITOR_TEXT: &str = "// EditorSurface.rs\n\npub struct EditorSurface {\n    // Product-neutral geometry lives in Luna.\n    // Product command policy remains in the application.\n}\n\n// Type, select, scroll, resize, and open the find panel.\n";
const THEME_TEXT: &str =
    "{\n  \"name\": \"Luna Dark\",\n  \"background\": \"#121418\",\n  \"accent\": \"#8269ff\"\n}\n";

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    run_native(EditorDemoApplication::new()?)?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DemoDocument {
    id: String,
    title: String,
    editor: EditableText,
    saved_revision: u64,
    scroll: TextScroll,
}

impl DemoDocument {
    fn new(id: impl Into<String>, title: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            editor: EditableText::new(text),
            saved_revision: 0,
            scroll: TextScroll::default(),
        }
    }

    const fn is_dirty(&self) -> bool {
        self.editor.edit_revision() != self.saved_revision
    }
}

struct EditorDemoApplication {
    root_id: NodeId,
    shell_id: NodeId,
    text_id: NodeId,
    palette_id: NodeId,
    find_id: NodeId,
    documents: Vec<DemoDocument>,
    active_index: usize,
    engine: TextEngine,
    last_text_layout: Option<TextLayoutSnapshot>,
    last_editor_bounds: RectI,
    viewport: RectI,
    theme: Theme,
    sidebar_is_visible: bool,
    selected_sidebar_id: Option<String>,
    palette: Option<CommandPaletteState>,
    find: Option<FindPanelState>,
    find_matches: Vec<Range<usize>>,
    drag_anchor: Option<TextLocation>,
    text_is_focused: bool,
    reveal_caret_on_next_frame: bool,
    next_untitled_number: u32,
}

impl EditorDemoApplication {
    fn new() -> Result<Self, ApplicationError> {
        Ok(Self {
            root_id: NodeId::new(ROOT_ID)?,
            shell_id: NodeId::new(SHELL_ID)?,
            text_id: NodeId::new(TEXT_ID)?,
            palette_id: NodeId::new(PALETTE_ID)?,
            find_id: NodeId::new(FIND_ID)?,
            documents: vec![
                DemoDocument::new("readme", "README.md", README_TEXT),
                DemoDocument::new("editor", "EditorSurface.rs", EDITOR_TEXT),
                DemoDocument::new("theme", "Theme.json", THEME_TEXT),
            ],
            active_index: 1,
            engine: TextEngine::new(),
            last_text_layout: None,
            last_editor_bounds: RectI::new(0, 0, 1, 1),
            viewport: RectI::new(0, 0, 1_180, 760),
            theme: Theme::luna_dark(),
            sidebar_is_visible: true,
            selected_sidebar_id: Some("editor".to_owned()),
            palette: None,
            find: None,
            find_matches: Vec::new(),
            drag_anchor: None,
            text_is_focused: true,
            reveal_caret_on_next_frame: true,
            next_untitled_number: 1,
        })
    }

    fn active_document(&self) -> &DemoDocument {
        &self.documents[self
            .active_index
            .min(self.documents.len().saturating_sub(1))]
    }

    fn active_document_mut(&mut self) -> &mut DemoDocument {
        let index = self
            .active_index
            .min(self.documents.len().saturating_sub(1));
        &mut self.documents[index]
    }

    fn shell_state(&self) -> EditorShellState {
        let active = self.active_document();
        EditorShellState {
            tabs: self
                .documents
                .iter()
                .map(|document| ShellTab {
                    id: document.id.clone(),
                    title: document.title.clone(),
                    is_dirty: document.is_dirty(),
                    is_closable: self.documents.len() > 1,
                })
                .collect(),
            active_tab_id: Some(active.id.clone()),
            sidebar_items: vec![
                SidebarItem::folder("workspace", "Luna-UI-Rust", 0, true),
                SidebarItem::file("readme", "README.md", 1),
                SidebarItem::file("editor", "EditorSurface.rs", 1),
                SidebarItem::file("theme", "Theme.json", 1),
            ],
            selected_sidebar_id: self.selected_sidebar_id.clone(),
            sidebar_is_visible: self.sidebar_is_visible,
            sidebar_width: 236,
            status_left: format!(
                "{}{}",
                active.title,
                if active.is_dirty() {
                    " — Modified"
                } else {
                    ""
                }
            ),
            status_right: format!(
                "Ln {}, Col {}  UTF-8",
                active.editor.caret().line_index.saturating_add(1),
                active.editor.caret().utf8_column.saturating_add(1)
            ),
            editor_children: vec![self.text_id.clone()],
            ..EditorShellState::default()
        }
    }

    fn create_shell(&self) -> Result<EditorShell, ApplicationError> {
        Ok(EditorShell::new(
            self.shell_id.clone(),
            self.viewport,
            self.theme,
            self.shell_state(),
            EditorShellMetrics::default(),
        )?)
    }

    fn text_width(&self, bounds: RectI) -> u32 {
        let style = TextViewStyle::from_theme(self.theme);
        bounds
            .inset(style.content_insets)
            .width
            .saturating_sub(style.gutter_width)
            .max(1)
    }

    fn text_view_from_layout(&self, layout: TextLayoutSnapshot) -> TextView {
        let document = self.active_document();
        TextView::new(
            self.text_id.clone(),
            self.last_editor_bounds,
            document.editor.document().clone(),
            layout,
            document.editor.caret(),
            document.editor.selection(),
            document.scroll,
            TextViewStyle::from_theme(self.theme),
            format!("Editor for {}", document.title),
            self.text_is_focused && self.palette.is_none() && self.find.is_none(),
            true,
        )
    }

    fn current_text_view(&self) -> Option<TextView> {
        self.last_text_layout
            .clone()
            .map(|layout| self.text_view_from_layout(layout))
    }

    fn append_label(
        &mut self,
        display_list: &mut DisplayList,
        id: &str,
        text: &str,
        bounds: RectI,
        alignment: TextAlignment,
        font_size: f32,
    ) -> Result<(), ApplicationError> {
        if bounds.is_empty() {
            return Ok(());
        }
        let node_id = NodeId::new(id)?;
        let layout = self.engine.shape(
            &luna_text::TextDocument::new(text),
            TextLayoutRequest::new(1, font_size, font_size + 6.0, self.theme.foreground)
                .with_maximum_raster_width(bounds.width.max(1)),
        )?;
        let label = TextLabel::new(node_id, bounds, text, layout, alignment);
        label.build_display_list(display_list);
        Ok(())
    }

    fn open_palette(&mut self) -> HostControl {
        self.palette = Some(CommandPaletteState {
            query: String::new(),
            items: vec![
                PaletteItem::new("new-file", "File: New File", "Ctrl+N"),
                PaletteItem::new("save", "File: Save", "Ctrl+S"),
                PaletteItem::new("close-tab", "File: Close Tab", "Ctrl+W"),
                PaletteItem::new("find", "Find: Open Find Panel", "Ctrl+F"),
                PaletteItem::new("toggle-sidebar", "View: Toggle Sidebar", "Ctrl+B"),
                PaletteItem::new("theme", "View: Toggle Light/Dark Theme", ""),
            ],
            selected_index: 0,
        });
        self.find = None;
        self.text_is_focused = false;
        HostControl::Redraw
    }

    fn open_find(&mut self) -> HostControl {
        self.find = Some(FindPanelState {
            replacement_is_visible: true,
            ..FindPanelState::default()
        });
        self.palette = None;
        self.text_is_focused = false;
        self.refresh_find_matches();
        HostControl::Redraw
    }

    fn execute_command(&mut self, command: &str) -> HostControl {
        self.palette = None;
        match command {
            "new-file" => self.new_document(),
            "save" => self.save_active(),
            "close-tab" => self.close_active(),
            "find" => return self.open_find(),
            "toggle-sidebar" => self.sidebar_is_visible = !self.sidebar_is_visible,
            "theme" => {
                self.theme = if self.theme == Theme::luna_dark() {
                    Theme::luna_light()
                } else {
                    Theme::luna_dark()
                };
            }
            _ => {}
        }
        self.text_is_focused = true;
        HostControl::Redraw
    }

    fn new_document(&mut self) {
        let number = self.next_untitled_number;
        self.next_untitled_number = self.next_untitled_number.saturating_add(1);
        let id = format!("untitled-{number}");
        self.documents.push(DemoDocument::new(
            id.clone(),
            format!("Untitled-{number}"),
            "// New Luna document\n",
        ));
        self.active_index = self.documents.len().saturating_sub(1);
        self.selected_sidebar_id = None;
        self.reveal_caret_on_next_frame = true;
    }

    fn save_active(&mut self) {
        let revision = self.active_document().editor.edit_revision();
        self.active_document_mut().saved_revision = revision;
    }

    fn close_active(&mut self) {
        if self.documents.len() <= 1 {
            return;
        }
        self.documents.remove(self.active_index);
        self.active_index = self
            .active_index
            .min(self.documents.len().saturating_sub(1));
        self.selected_sidebar_id = Some(self.active_document().id.clone());
        self.reveal_caret_on_next_frame = true;
    }

    fn activate_document(&mut self, id: &str) {
        if let Some(index) = self.documents.iter().position(|document| document.id == id) {
            self.active_index = index;
            self.selected_sidebar_id = Some(id.to_owned());
            self.reveal_caret_on_next_frame = true;
            self.text_is_focused = true;
        }
    }

    fn refresh_find_matches(&mut self) {
        let query = self
            .find
            .as_ref()
            .map_or_else(String::new, |state| state.query.clone());
        let matches = if query.is_empty() {
            Vec::new()
        } else {
            self.active_document()
                .editor
                .document()
                .text()
                .match_indices(&query)
                .map(|(start, value)| start..start.saturating_add(value.len()))
                .collect()
        };
        self.find_matches = matches;
        if let Some(find) = self.find.as_mut() {
            find.match_count = self.find_matches.len();
            find.selected_match = if self.find_matches.is_empty() {
                0
            } else {
                find.selected_match.clamp(1, self.find_matches.len())
            };
        }
    }

    fn select_find_match(&mut self, delta: i32) {
        self.refresh_find_matches();
        if self.find_matches.is_empty() {
            return;
        }
        let count = self.find_matches.len();
        let current = self
            .find
            .as_ref()
            .map_or(0, |state| state.selected_match.saturating_sub(1));
        let next = if delta < 0 {
            if current == 0 {
                count.saturating_sub(1)
            } else {
                current.saturating_sub(1)
            }
        } else {
            current.saturating_add(1) % count
        };
        if let Some(find) = self.find.as_mut() {
            find.selected_match = next.saturating_add(1);
        }
        let range = self.find_matches[next].clone();
        let document = self.active_document().editor.document().clone();
        let anchor = document.location_for_offset(range.start, SnapBias::Backward);
        let focus = document.location_for_offset(range.end, SnapBias::Forward);
        self.active_document_mut()
            .editor
            .set_selection(TextRange::new(anchor, focus));
        self.reveal_caret_on_next_frame = true;
    }

    fn handle_palette_key(&mut self, key: NamedKey) -> HostControl {
        match key {
            NamedKey::Escape => {
                self.palette = None;
                self.text_is_focused = true;
            }
            NamedKey::ArrowDown => {
                if let Some(state) = self.palette.as_mut() {
                    state.select_next();
                }
            }
            NamedKey::ArrowUp => {
                if let Some(state) = self.palette.as_mut() {
                    state.select_previous();
                }
            }
            NamedKey::Backspace => {
                if let Some(state) = self.palette.as_mut() {
                    let _ = state.query.pop();
                    state.selected_index = 0;
                    state.normalize_selection();
                }
            }
            NamedKey::Enter => {
                let command = self
                    .palette
                    .as_ref()
                    .and_then(CommandPaletteState::selected_item)
                    .map(|item| item.id.clone());
                if let Some(command) = command {
                    return self.execute_command(&command);
                }
            }
            NamedKey::Tab
            | NamedKey::Delete
            | NamedKey::ArrowLeft
            | NamedKey::ArrowRight
            | NamedKey::Home
            | NamedKey::End
            | NamedKey::PageUp
            | NamedKey::PageDown => {}
        }
        HostControl::Redraw
    }

    fn handle_find_key(&mut self, key: NamedKey) -> HostControl {
        match key {
            NamedKey::Escape => {
                self.find = None;
                self.text_is_focused = true;
            }
            NamedKey::Tab => {
                if let Some(find) = self.find.as_mut() {
                    find.active_field = match find.active_field {
                        FindField::Query => FindField::Replacement,
                        FindField::Replacement => FindField::Query,
                    };
                }
            }
            NamedKey::Backspace => {
                if let Some(find) = self.find.as_mut() {
                    match find.active_field {
                        FindField::Query => {
                            let _ = find.query.pop();
                        }
                        FindField::Replacement => {
                            let _ = find.replacement.pop();
                        }
                    }
                }
                self.refresh_find_matches();
            }
            NamedKey::Enter | NamedKey::ArrowDown => self.select_find_match(1),
            NamedKey::ArrowUp => self.select_find_match(-1),
            NamedKey::Delete
            | NamedKey::ArrowLeft
            | NamedKey::ArrowRight
            | NamedKey::Home
            | NamedKey::End
            | NamedKey::PageUp
            | NamedKey::PageDown => {}
        }
        HostControl::Redraw
    }

    fn handle_editor_key(&mut self, key: NamedKey, modifiers: Modifiers) -> HostControl {
        let extending = modifiers.contains(Modifiers::SHIFT);
        let viewport_height = self.last_editor_bounds.height;
        let mut reveal = true;
        match key {
            NamedKey::ArrowLeft => self.active_document_mut().editor.move_backward(extending),
            NamedKey::ArrowRight => self.active_document_mut().editor.move_forward(extending),
            NamedKey::ArrowUp => self.active_document_mut().editor.move_up(extending),
            NamedKey::ArrowDown => self.active_document_mut().editor.move_down(extending),
            NamedKey::Home => self
                .active_document_mut()
                .editor
                .move_to_line_start(extending),
            NamedKey::End => self
                .active_document_mut()
                .editor
                .move_to_line_end(extending),
            NamedKey::Backspace => {
                let _ = self.active_document_mut().editor.delete_backward();
            }
            NamedKey::Delete => {
                let _ = self.active_document_mut().editor.delete_forward();
            }
            NamedKey::Enter => {
                let _ = self.active_document_mut().editor.insert_newline();
            }
            NamedKey::PageUp => {
                let amount = i32::try_from(viewport_height).unwrap_or(i32::MAX);
                let current_y = self.active_document().scroll.y;
                self.active_document_mut().scroll.y = current_y.saturating_sub(amount).max(0);
                reveal = false;
            }
            NamedKey::PageDown => {
                let amount = i32::try_from(viewport_height).unwrap_or(i32::MAX);
                let current_y = self.active_document().scroll.y;
                self.active_document_mut().scroll.y = current_y.saturating_add(amount);
                reveal = false;
            }
            NamedKey::Escape | NamedKey::Tab => return HostControl::Continue,
        }
        self.reveal_caret_on_next_frame = reveal;
        HostControl::Redraw
    }

    fn apply_pointer_to_text(&mut self, position: PointI, extending: bool) -> bool {
        let Some(view) = self.current_text_view() else {
            return false;
        };
        let Some(location) = view.text_hit_test(position) else {
            return false;
        };
        if extending {
            let anchor = self
                .drag_anchor
                .unwrap_or(self.active_document().editor.caret());
            self.active_document_mut()
                .editor
                .set_selection(TextRange::new(anchor, location));
        } else {
            self.active_document_mut().editor.set_caret(location);
            self.drag_anchor = Some(location);
        }
        self.text_is_focused = true;
        self.reveal_caret_on_next_frame = true;
        true
    }

    fn append_shell_labels(
        &mut self,
        shell: &EditorShell,
        display_list: &mut DisplayList,
    ) -> Result<(), ApplicationError> {
        for (index, frame) in shell.layout().menus.iter().enumerate() {
            self.append_label(
                display_list,
                &format!("m3-editor-menu-label-{index}"),
                &frame.title,
                frame.bounds.inset(InsetsI::symmetric(8, 2)),
                TextAlignment::Leading,
                13.0,
            )?;
        }
        for (index, frame) in shell.layout().tabs.iter().enumerate() {
            let title = self
                .documents
                .iter()
                .find(|document| document.id == frame.id)
                .map_or(frame.title.clone(), |document| {
                    format!(
                        "{}{}",
                        document.title,
                        if document.is_dirty() { " •" } else { "" }
                    )
                });
            let width = frame.bounds.width.saturating_sub(34);
            self.append_label(
                display_list,
                &format!("m3-editor-tab-label-{index}"),
                &title,
                RectI::new(
                    frame.bounds.x.saturating_add(10),
                    frame.bounds.y,
                    width,
                    frame.bounds.height,
                ),
                TextAlignment::Leading,
                13.0,
            )?;
        }
        self.append_label(
            display_list,
            "m3-editor-sidebar-header-label",
            "PROJECT",
            shell
                .layout()
                .sidebar_header
                .inset(InsetsI::symmetric(10, 2)),
            TextAlignment::Leading,
            12.0,
        )?;
        for (index, frame) in shell.layout().sidebar_rows.iter().enumerate() {
            let indent = 10_u32.saturating_add(u32::from(frame.depth).saturating_mul(14));
            self.append_label(
                display_list,
                &format!("m3-editor-sidebar-label-{index}"),
                &frame.title,
                RectI::new(
                    frame
                        .bounds
                        .x
                        .saturating_add(i32::try_from(indent).unwrap_or(i32::MAX)),
                    frame.bounds.y,
                    frame.bounds.width.saturating_sub(indent + 6),
                    frame.bounds.height,
                ),
                TextAlignment::Leading,
                13.0,
            )?;
        }
        let state = self.shell_state();
        self.append_label(
            display_list,
            "m3-editor-status-left-label",
            &state.status_left,
            shell.layout().status_left,
            TextAlignment::Leading,
            12.0,
        )?;
        self.append_label(
            display_list,
            "m3-editor-status-right-label",
            &state.status_right,
            shell.layout().status_right,
            TextAlignment::Trailing,
            12.0,
        )?;
        Ok(())
    }

    fn append_palette_labels(
        &mut self,
        palette: &CommandPalette,
        display_list: &mut DisplayList,
    ) -> Result<(), ApplicationError> {
        self.append_label(
            display_list,
            "m3-editor-palette-title-label",
            "Command Palette",
            palette.layout().title.inset(InsetsI::symmetric(12, 4)),
            TextAlignment::Leading,
            15.0,
        )?;
        let query = self
            .palette
            .as_ref()
            .map_or_else(String::new, |state| state.query.clone());
        self.append_label(
            display_list,
            "m3-editor-palette-query-label",
            if query.is_empty() {
                "Type a command…"
            } else {
                &query
            },
            palette.layout().input.inset(InsetsI::symmetric(8, 3)),
            TextAlignment::Leading,
            14.0,
        )?;
        for (index, row) in palette.layout().rows.iter().enumerate() {
            self.append_label(
                display_list,
                &format!("m3-editor-palette-row-title-{index}"),
                &row.title,
                RectI::new(
                    row.bounds.x.saturating_add(10),
                    row.bounds.y,
                    row.bounds.width.saturating_sub(120),
                    row.bounds.height,
                ),
                TextAlignment::Leading,
                13.0,
            )?;
            self.append_label(
                display_list,
                &format!("m3-editor-palette-row-detail-{index}"),
                &row.detail,
                RectI::new(
                    i32::try_from(row.bounds.right().saturating_sub(110)).unwrap_or(row.bounds.x),
                    row.bounds.y,
                    100,
                    row.bounds.height,
                ),
                TextAlignment::Trailing,
                12.0,
            )?;
        }
        Ok(())
    }

    fn append_find_labels(
        &mut self,
        panel: &FindPanel,
        display_list: &mut DisplayList,
    ) -> Result<(), ApplicationError> {
        let state = self.find.clone().unwrap_or_default();
        self.append_label(
            display_list,
            "m3-editor-find-query-label",
            if state.query.is_empty() {
                "Find…"
            } else {
                &state.query
            },
            panel.layout().query.inset(InsetsI::symmetric(8, 3)),
            TextAlignment::Leading,
            13.0,
        )?;
        if state.replacement_is_visible {
            self.append_label(
                display_list,
                "m3-editor-find-replacement-label",
                if state.replacement.is_empty() {
                    "Replace…"
                } else {
                    &state.replacement
                },
                panel.layout().replacement.inset(InsetsI::symmetric(8, 3)),
                TextAlignment::Leading,
                13.0,
            )?;
        }
        self.append_label(
            display_list,
            "m3-editor-find-previous-label",
            "‹",
            panel.layout().previous,
            TextAlignment::Center,
            16.0,
        )?;
        self.append_label(
            display_list,
            "m3-editor-find-next-label",
            "›",
            panel.layout().next,
            TextAlignment::Center,
            16.0,
        )?;
        self.append_label(
            display_list,
            "m3-editor-find-close-label",
            "×",
            panel.layout().close,
            TextAlignment::Center,
            16.0,
        )?;
        let status = if state.match_count == 0 {
            if state.query.is_empty() {
                "No query".to_owned()
            } else {
                "No matches".to_owned()
            }
        } else {
            format!("{} of {}", state.selected_match, state.match_count)
        };
        self.append_label(
            display_list,
            "m3-editor-find-status-label",
            &status,
            panel.layout().status,
            TextAlignment::Trailing,
            12.0,
        )?;
        Ok(())
    }
}

impl NativeApplication for EditorDemoApplication {
    fn window_config(&self) -> WindowConfig {
        WindowConfig {
            title: "Luna UI Rust — Editor Demo".to_owned(),
            initial_size: SizeI::new(1_180, 760),
            minimum_size: Some(SizeI::new(700, 440)),
        }
    }

    fn build_frame(&mut self, viewport: RectI) -> Result<UiFrame, ApplicationError> {
        self.viewport = viewport;
        let shell = self.create_shell()?;
        self.last_editor_bounds = shell.layout().editor;
        let document = self.active_document().editor.document().clone();
        let caret = self.active_document().editor.caret();
        let selection = self.active_document().editor.selection();
        let mut scroll = self.active_document().scroll;
        let editor_text_width = self.text_width(self.last_editor_bounds);
        let foreground = self.theme.foreground;
        let layout = self.engine.shape(
            &document,
            TextLayoutRequest::new(editor_text_width, 15.0, 22.0, foreground),
        )?;
        if self.reveal_caret_on_next_frame {
            let provisional = TextView::new(
                self.text_id.clone(),
                self.last_editor_bounds,
                document.clone(),
                layout.clone(),
                caret,
                selection,
                scroll,
                TextViewStyle::from_theme(self.theme),
                "Editor",
                true,
                true,
            );
            scroll = provisional.scroll_revealing_caret();
            self.active_document_mut().scroll = scroll;
            self.reveal_caret_on_next_frame = false;
        }
        let text_view = TextView::new(
            self.text_id.clone(),
            self.last_editor_bounds,
            document,
            layout.clone(),
            caret,
            selection,
            scroll,
            TextViewStyle::from_theme(self.theme),
            format!("Editor for {}", self.active_document().title),
            self.text_is_focused && self.palette.is_none() && self.find.is_none(),
            true,
        );
        self.last_text_layout = Some(layout);

        let mut display_list = DisplayList::new();
        display_list.clear(self.theme.background);
        shell.build_display_list(&mut display_list);
        text_view.build_display_list(&mut display_list);
        let mut root_children = vec![self.shell_id.clone()];
        let mut nodes = vec![
            AccessibilityNode::new(self.root_id.clone(), AccessibilityRole::Window, viewport)
                .with_label("Luna UI Rust editor demo window")
                .with_children(root_children.clone()),
        ];
        nodes.extend(shell.accessibility_nodes());
        nodes.extend(text_view.accessibility_nodes());
        self.append_shell_labels(&shell, &mut display_list)?;

        if let Some(state) = self.palette.clone() {
            let palette =
                CommandPalette::new(self.palette_id.clone(), viewport, self.theme, state)?;
            palette.build_display_list(&mut display_list);
            nodes.extend(palette.accessibility_nodes());
            root_children.push(self.palette_id.clone());
            self.append_palette_labels(&palette, &mut display_list)?;
        }
        if let Some(state) = self.find.clone() {
            let panel = FindPanel::new(self.find_id.clone(), viewport, self.theme, state)?;
            panel.build_display_list(&mut display_list);
            nodes.extend(panel.accessibility_nodes());
            root_children.push(self.find_id.clone());
            self.append_find_labels(&panel, &mut display_list)?;
        }
        nodes[0] =
            AccessibilityNode::new(self.root_id.clone(), AccessibilityRole::Window, viewport)
                .with_label("Luna UI Rust editor demo window")
                .with_children(root_children);
        Ok(UiFrame::from_parts(
            display_list,
            self.root_id.clone(),
            nodes,
        )?)
    }

    fn handle_input(&mut self, event: InputEvent) -> HostControl {
        match event {
            InputEvent::Keyboard(keyboard) if keyboard.is_pressed => {
                if self.palette.is_some() {
                    if let Key::Named(key) = &keyboard.key {
                        return self.handle_palette_key(*key);
                    }
                    if let Some(text) = keyboard.text.as_deref() {
                        if let Some(state) = self.palette.as_mut() {
                            state.query.push_str(text);
                            state.selected_index = 0;
                            state.normalize_selection();
                        }
                        return HostControl::Redraw;
                    }
                    return HostControl::Continue;
                }
                if self.find.is_some() {
                    if let Key::Named(key) = &keyboard.key {
                        return self.handle_find_key(*key);
                    }
                    if let Some(text) = keyboard.text.as_deref() {
                        if let Some(find) = self.find.as_mut() {
                            match find.active_field {
                                FindField::Query => find.query.push_str(text),
                                FindField::Replacement => find.replacement.push_str(text),
                            }
                        }
                        self.refresh_find_matches();
                        return HostControl::Redraw;
                    }
                    return HostControl::Continue;
                }

                let command_modified = keyboard.modifiers.contains(Modifiers::SUPER)
                    || (keyboard.modifiers.contains(Modifiers::CONTROL)
                        && !keyboard.modifiers.contains(Modifiers::ALT));
                if command_modified {
                    let command = match &keyboard.key {
                        Key::Character(value) if value.eq_ignore_ascii_case("p") => Some("palette"),
                        Key::Character(value) if value.eq_ignore_ascii_case("f") => Some("find"),
                        Key::Character(value) if value.eq_ignore_ascii_case("s") => Some("save"),
                        Key::Character(value) if value.eq_ignore_ascii_case("n") => Some("new"),
                        Key::Character(value) if value.eq_ignore_ascii_case("b") => Some("sidebar"),
                        Key::Character(value) if value.eq_ignore_ascii_case("w") => Some("close"),
                        Key::Character(value) if value.eq_ignore_ascii_case("a") => {
                            Some("select-all")
                        }
                        _ => None,
                    };
                    match command {
                        Some("palette") => return self.open_palette(),
                        Some("find") => return self.open_find(),
                        Some("save") => self.save_active(),
                        Some("new") => self.new_document(),
                        Some("sidebar") => self.sidebar_is_visible = !self.sidebar_is_visible,
                        Some("close") => self.close_active(),
                        Some("select-all") => {
                            let end = self.active_document().editor.document().end_location();
                            self.active_document_mut()
                                .editor
                                .set_selection(TextRange::new(TextLocation::default(), end));
                            self.reveal_caret_on_next_frame = true;
                        }
                        Some(_) | None => {}
                    }
                    return HostControl::Redraw;
                }
                if keyboard.key == Key::Named(NamedKey::Escape) {
                    return HostControl::Exit;
                }
                if let Key::Named(key) = &keyboard.key {
                    return self.handle_editor_key(*key, keyboard.modifiers);
                }
                let logical_fallback = match &keyboard.key {
                    Key::Character(value) => Some(value.as_str()),
                    Key::Named(_) | Key::Unidentified => None,
                };
                if let Some(text) = keyboard.text.as_deref().or(logical_fallback)
                    && !text.is_empty()
                    && !text.chars().all(char::is_control)
                {
                    let _ = self.active_document_mut().editor.insert_text(text);
                    self.reveal_caret_on_next_frame = true;
                    return HostControl::Redraw;
                }
            }
            InputEvent::Text(text) => {
                if let Some(state) = self.palette.as_mut() {
                    state.query.push_str(&text);
                    state.normalize_selection();
                } else if self.find.is_some() {
                    if let Some(find) = self.find.as_mut() {
                        match find.active_field {
                            FindField::Query => find.query.push_str(&text),
                            FindField::Replacement => find.replacement.push_str(&text),
                        }
                    }
                    self.refresh_find_matches();
                } else {
                    let _ = self.active_document_mut().editor.insert_text(&text);
                    self.reveal_caret_on_next_frame = true;
                }
                return HostControl::Redraw;
            }
            InputEvent::Pointer(pointer) => {
                if pointer.kind == PointerEventKind::Pressed(PointerButton::Primary) {
                    if let Some(state) = self.palette.clone() {
                        if let Ok(palette) = CommandPalette::new(
                            self.palette_id.clone(),
                            self.viewport,
                            self.theme,
                            state,
                        ) && let Some(command) =
                            palette.command_at(pointer.position).map(str::to_owned)
                        {
                            return self.execute_command(&command);
                        }
                    } else if let Some(state) = self.find.clone() {
                        if let Ok(panel) =
                            FindPanel::new(self.find_id.clone(), self.viewport, self.theme, state)
                        {
                            let layout = panel.layout();
                            if layout.close.contains(pointer.position) {
                                self.find = None;
                                self.text_is_focused = true;
                                return HostControl::Redraw;
                            }
                            if layout.next.contains(pointer.position) {
                                self.select_find_match(1);
                                return HostControl::Redraw;
                            }
                            if layout.previous.contains(pointer.position) {
                                self.select_find_match(-1);
                                return HostControl::Redraw;
                            }
                            if layout.query.contains(pointer.position) {
                                if let Some(find) = self.find.as_mut() {
                                    find.active_field = FindField::Query;
                                }
                                return HostControl::Redraw;
                            }
                            if layout.replacement.contains(pointer.position) {
                                if let Some(find) = self.find.as_mut() {
                                    find.active_field = FindField::Replacement;
                                }
                                return HostControl::Redraw;
                            }
                        }
                    } else if let Ok(shell) = self.create_shell() {
                        match shell.semantic_hit_test(pointer.position) {
                            Some(EditorShellHit::Tab(id)) => self.activate_document(&id),
                            Some(EditorShellHit::CloseTab(id)) => {
                                self.activate_document(&id);
                                self.close_active();
                            }
                            Some(EditorShellHit::SidebarItem(id)) => self.activate_document(&id),
                            Some(EditorShellHit::Menu(_)) => return self.open_palette(),
                            Some(EditorShellHit::Editor) => {
                                let extending = pointer.modifiers.contains(Modifiers::SHIFT);
                                if self.apply_pointer_to_text(pointer.position, extending) {
                                    return HostControl::Redraw;
                                }
                            }
                            None => {}
                        }
                    }
                    return HostControl::Redraw;
                }
                if pointer.kind == PointerEventKind::Moved
                    && self.drag_anchor.is_some()
                    && self.apply_pointer_to_text(pointer.position, true)
                {
                    return HostControl::Redraw;
                }
                if matches!(
                    pointer.kind,
                    PointerEventKind::Released(PointerButton::Primary) | PointerEventKind::Left
                ) {
                    self.drag_anchor = None;
                }
            }
            InputEvent::Scroll(scroll) if self.palette.is_none() && self.find.is_none() => {
                if let Some(view) = self.current_text_view() {
                    let maximum = view.maximum_scroll();
                    let (delta_x, delta_y) =
                        if scroll.modifiers.contains(Modifiers::SHIFT) && scroll.delta_x == 0 {
                            (scroll.delta_y, 0)
                        } else {
                            (scroll.delta_x, scroll.delta_y)
                        };
                    let document = self.active_document_mut();
                    document
                        .scroll
                        .scroll_by(delta_x, delta_y, maximum.x, maximum.y);
                    self.reveal_caret_on_next_frame = false;
                    return HostControl::Redraw;
                }
            }
            InputEvent::FocusGained => {
                self.text_is_focused = true;
                return HostControl::Redraw;
            }
            InputEvent::FocusLost => {
                self.text_is_focused = false;
                self.drag_anchor = None;
                return HostControl::Redraw;
            }
            InputEvent::Keyboard(_) | InputEvent::Scroll(_) => {}
        }
        HostControl::Continue
    }

    fn handle_accessibility_action(&mut self, request: AccessibilityActionRequest) -> HostControl {
        let Some(target) = request.target else {
            return HostControl::Continue;
        };
        if request.kind == AccessibilityActionKind::Other {
            return HostControl::Continue;
        }

        if let Some(state) = self.palette.clone()
            && let Ok(palette) =
                CommandPalette::new(self.palette_id.clone(), self.viewport, self.theme, state)
        {
            if &target == palette.input_node_id() {
                self.text_is_focused = false;
                return HostControl::Redraw;
            }
            if request.kind == AccessibilityActionKind::Click {
                let command = palette.command_for_node(&target).map(str::to_owned);
                if let Some(command) = command {
                    return self.execute_command(&command);
                }
            }
        }

        if let Some(state) = self.find.clone()
            && let Ok(panel) =
                FindPanel::new(self.find_id.clone(), self.viewport, self.theme, state)
        {
            if &target == panel.query_node_id() {
                if let Some(find) = self.find.as_mut() {
                    find.active_field = FindField::Query;
                }
                return HostControl::Redraw;
            }
            if &target == panel.replacement_node_id() {
                if let Some(find) = self.find.as_mut() {
                    find.active_field = FindField::Replacement;
                }
                return HostControl::Redraw;
            }
            if request.kind == AccessibilityActionKind::Click {
                if &target == panel.previous_node_id() {
                    self.select_find_match(-1);
                    return HostControl::Redraw;
                }
                if &target == panel.next_node_id() {
                    self.select_find_match(1);
                    return HostControl::Redraw;
                }
                if &target == panel.close_node_id() {
                    self.find = None;
                    self.text_is_focused = true;
                    return HostControl::Redraw;
                }
            }
        }

        if let Ok(shell) = self.create_shell() {
            match shell.semantic_target(&target) {
                Some(EditorShellHit::Menu(_)) if request.kind == AccessibilityActionKind::Click => {
                    return self.open_palette();
                }
                Some(EditorShellHit::Tab(id) | EditorShellHit::SidebarItem(id)) => {
                    self.activate_document(&id);
                    return HostControl::Redraw;
                }
                Some(EditorShellHit::Editor) => {
                    self.text_is_focused = true;
                    return HostControl::Redraw;
                }
                Some(EditorShellHit::CloseTab(_) | EditorShellHit::Menu(_)) | None => {}
            }
        }

        if target == self.text_id {
            self.text_is_focused = true;
            return HostControl::Redraw;
        }
        HostControl::Continue
    }
}
