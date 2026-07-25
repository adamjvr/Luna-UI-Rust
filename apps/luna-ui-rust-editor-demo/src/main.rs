// SPDX-License-Identifier: MPL-2.0

//! Native M3.2a editor integration harness for Luna UI Rust.
//!
//! This application mirrors the purpose of Swift LunaUITestApp's default editor mode: reusable
//! shell anatomy and editor text are exercised together without embedding Moth Text product
//! policy in Luna. It provides menus, tabs, a project sidebar, status chrome, editable text,
//! real dropdown menus, a separate command palette, find panel, document lifecycle tracking,
//! accessibility,
//! retained document layouts, overscanned glyph rasters, and stable-slot chrome-label caching.
//!
//! Shortcuts: Control-P command palette, Control-F find, Control-H replace, Control-S save,
//! Control-N new document, Control-B sidebar, Control-W close tab, Control-A select all, and Escape
//! closes the active menu/overlay or exits when no transient surface is open.

use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{InsetsI, NodeId, PointI, RectI, SizeI};
use luna_documents::{
    CloseRequirement, DocumentId, DocumentRecord, DocumentRegistry, DocumentSource, SaveRequirement,
};
use luna_host_winit::{
    AccessibilityActionKind, AccessibilityActionRequest, ApplicationError, HostControl,
    InvalidationClass, NativeApplication, WindowConfig, run_native,
};
use luna_input::{
    InputEvent, Key, Modifiers, NamedKey, PointerButton, PointerEvent, PointerEventKind,
};
use luna_render::DisplayList;
use luna_text::{EditableText, SnapBias, TextLocation, TextRange, TextScroll};
use luna_text_cosmic::{
    TextEngine, TextLayoutCache, TextLayoutCacheStats, TextLayoutRequest, TextLayoutSnapshot,
};
use luna_theme::{Rgba8, Theme};
use luna_ui::{
    CommandPalette, CommandPaletteState, DropdownMenu, DropdownMenuState, EditorShell,
    EditorShellHit, EditorShellMetrics, EditorShellState, FindField, FindPanel, FindPanelState,
    MenuCommand, MenuDefinition, MenuItem, PaletteItem, ShellMenu, ShellTab, SidebarItem,
    TextAlignment, TextLabel, TextLabelCache, TextView, TextViewStyle, UiFrame, Widget,
};
use std::collections::HashMap;
use std::error::Error;
use std::ops::Range;

const ROOT_ID: &str = "m3-editor-window";
const SHELL_ID: &str = "m3-editor-shell";
const TEXT_ID: &str = "m3-editor-text";
const PALETTE_ID: &str = "m3-editor-palette";
const FIND_ID: &str = "m3-editor-find";
const MENU_ID: &str = "m3-editor-dropdown-menu";

const README_TEXT: &str = concat!(
    "# Luna UI Rust\n\n",
    "M3.2a adds product-neutral document identity and lifecycle state.\n\n",
    "- Deterministic shell geometry\n",
    "- Revision-keyed document shaping\n",
    "- Viewport-band glyph rasterization\n",
    "- Stable-slot editor chrome labels\n",
    "- Shared menu and palette command IDs\n",
    "- Stable document IDs and monotonic untitled names\n",
    "- Explicit save and close requirements\n",
    "- Shared accessibility geometry\n\n",
    "Click File/Edit/Find/View/Help, or press Control-P for the palette.\n",
);
const EDITOR_TEXT: &str = concat!(
    "// EditorSurface.rs\n\n",
    "pub struct EditorSurface {\n",
    "    // Product-neutral geometry lives in Luna.\n",
    "    // Product command policy remains in the application.\n",
    "}\n\n",
    "// Type, select, scroll, resize, and open the find panel.\n",
);
const THEME_TEXT: &str =
    "{\n  \"name\": \"Luna Dark\",\n  \"background\": \"#121418\",\n  \"accent\": \"#8269ff\"\n}\n";

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    run_native(EditorDemoApplication::new()?)?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DemoDocument {
    id: DocumentId,
    editor: EditableText,
    scroll: TextScroll,
}

impl DemoDocument {
    fn new(id: DocumentId, text: impl Into<String>) -> Self {
        Self {
            id,
            editor: EditableText::new(text),
            scroll: TextScroll::default(),
        }
    }

    fn title<'a>(&self, registry: &'a DocumentRegistry) -> &'a str {
        registry
            .get(self.id)
            .map_or("Unknown document", DocumentRecord::title)
    }

    fn is_dirty(&self, registry: &DocumentRegistry) -> bool {
        registry
            .get(self.id)
            .is_some_and(|record| record.is_dirty(self.editor.edit_revision()))
    }

    fn stable_key(&self) -> String {
        self.id.stable_key()
    }
}

struct EditorDemoApplication {
    root_id: NodeId,
    shell_id: NodeId,
    text_id: NodeId,
    palette_id: NodeId,
    find_id: NodeId,
    menu_id: NodeId,
    document_registry: DocumentRegistry,
    documents: Vec<DemoDocument>,
    active_index: usize,
    engine: TextEngine,
    text_layouts: HashMap<String, TextLayoutCache>,
    label_cache: TextLabelCache,
    last_editor_bounds: RectI,
    viewport: RectI,
    theme: Theme,
    sidebar_is_visible: bool,
    selected_sidebar_id: Option<String>,
    palette: Option<CommandPaletteState>,
    menu: DropdownMenuState,
    find: Option<FindPanelState>,
    find_matches: Vec<Range<usize>>,
    drag_anchor: Option<TextLocation>,
    text_is_focused: bool,
    reveal_caret_on_next_frame: bool,
    lifecycle_notice: Option<String>,
    frame_build_count: u64,
}

impl EditorDemoApplication {
    fn new() -> Result<Self, ApplicationError> {
        let mut document_registry = DocumentRegistry::new();
        let readme_id = document_registry.register_virtual("readme", "README.md", 0)?;
        let editor_id = document_registry.register_virtual("editor", "EditorSurface.rs", 0)?;
        let theme_id = document_registry.register_virtual("theme", "Theme.json", 0)?;
        Ok(Self {
            root_id: NodeId::new(ROOT_ID)?,
            shell_id: NodeId::new(SHELL_ID)?,
            text_id: NodeId::new(TEXT_ID)?,
            palette_id: NodeId::new(PALETTE_ID)?,
            find_id: NodeId::new(FIND_ID)?,
            menu_id: NodeId::new(MENU_ID)?,
            document_registry,
            documents: vec![
                DemoDocument::new(readme_id, README_TEXT),
                DemoDocument::new(editor_id, EDITOR_TEXT),
                DemoDocument::new(theme_id, THEME_TEXT),
            ],
            active_index: 1,
            engine: TextEngine::new(),
            text_layouts: HashMap::new(),
            label_cache: TextLabelCache::new(),
            last_editor_bounds: RectI::new(0, 0, 1, 1),
            viewport: RectI::new(0, 0, 1_180, 760),
            theme: Theme::luna_dark(),
            sidebar_is_visible: true,
            selected_sidebar_id: Some(editor_id.stable_key()),
            palette: None,
            menu: DropdownMenuState::default(),
            find: None,
            find_matches: Vec::new(),
            drag_anchor: None,
            text_is_focused: true,
            reveal_caret_on_next_frame: true,
            lifecycle_notice: None,
            frame_build_count: 0,
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

    fn menu_definitions(&self) -> Vec<MenuDefinition> {
        let active = self.active_document();
        let save_is_enabled = self
            .document_registry
            .get(active.id)
            .map(|record| record.save_requirement(active.editor.edit_revision()))
            .is_some_and(|requirement| {
                matches!(
                    requirement,
                    SaveRequirement::SaveAs | SaveRequirement::WriteFile { .. }
                )
            });
        let find_navigation_is_enabled = self.find.is_some() && !self.find_matches.is_empty();
        vec![
            MenuDefinition::new(
                "file",
                "File",
                vec![
                    MenuItem::command(MenuCommand::new("new-file", "New File", "Ctrl+N")),
                    MenuItem::command(
                        MenuCommand::new("open", "Open…", "Ctrl+O").with_enabled(false),
                    ),
                    MenuItem::Separator,
                    MenuItem::command(
                        MenuCommand::new("save", "Save", "Ctrl+S").with_enabled(save_is_enabled),
                    ),
                    MenuItem::command(
                        MenuCommand::new("save-as", "Save As…", "Ctrl+Shift+S").with_enabled(false),
                    ),
                    MenuItem::Separator,
                    MenuItem::command(
                        MenuCommand::new("close-tab", "Close Tab", "Ctrl+W")
                            .with_enabled(self.documents.len() > 1),
                    ),
                    MenuItem::Separator,
                    MenuItem::command(MenuCommand::new("exit", "Exit", "")),
                ],
            ),
            MenuDefinition::new(
                "edit",
                "Edit",
                vec![
                    MenuItem::command(
                        MenuCommand::new("undo", "Undo", "Ctrl+Z").with_enabled(false),
                    ),
                    MenuItem::command(
                        MenuCommand::new("redo", "Redo", "Ctrl+Shift+Z").with_enabled(false),
                    ),
                    MenuItem::Separator,
                    MenuItem::command(MenuCommand::new("cut", "Cut", "Ctrl+X").with_enabled(false)),
                    MenuItem::command(
                        MenuCommand::new("copy", "Copy", "Ctrl+C").with_enabled(false),
                    ),
                    MenuItem::command(
                        MenuCommand::new("paste", "Paste", "Ctrl+V").with_enabled(false),
                    ),
                    MenuItem::Separator,
                    MenuItem::command(
                        MenuCommand::new("select-all", "Select All", "Ctrl+A")
                            .with_enabled(!active.editor.document().text().is_empty()),
                    ),
                ],
            ),
            MenuDefinition::new(
                "find",
                "Find",
                vec![
                    MenuItem::command(MenuCommand::new("find", "Find…", "Ctrl+F")),
                    MenuItem::command(MenuCommand::new("replace", "Replace…", "Ctrl+H")),
                    MenuItem::Separator,
                    MenuItem::command(
                        MenuCommand::new("find-next", "Find Next", "F3")
                            .with_enabled(find_navigation_is_enabled),
                    ),
                    MenuItem::command(
                        MenuCommand::new("find-previous", "Find Previous", "Shift+F3")
                            .with_enabled(find_navigation_is_enabled),
                    ),
                ],
            ),
            MenuDefinition::new(
                "view",
                "View",
                vec![
                    MenuItem::command(
                        MenuCommand::new("toggle-sidebar", "Show Sidebar", "Ctrl+B")
                            .with_checked(self.sidebar_is_visible),
                    ),
                    MenuItem::command(
                        MenuCommand::new("theme", "Light Theme", "")
                            .with_checked(self.theme == Theme::luna_light()),
                    ),
                ],
            ),
            MenuDefinition::new(
                "help",
                "Help",
                vec![MenuItem::command(
                    MenuCommand::new("about", "About Luna UI Rust", "").with_enabled(false),
                )],
            ),
        ]
    }

    fn palette_items(&self) -> Vec<PaletteItem> {
        self.menu_definitions()
            .into_iter()
            .flat_map(|menu| {
                let menu_title = menu.title;
                menu.items.into_iter().filter_map(move |item| {
                    let MenuItem::Command(command) = item else {
                        return None;
                    };
                    command.is_enabled.then(|| {
                        PaletteItem::new(
                            command.id,
                            format!("{menu_title}: {}", command.title),
                            command.shortcut,
                        )
                    })
                })
            })
            .collect()
    }

    fn shell_state(&self) -> EditorShellState {
        let active = self.active_document();
        let active_title = active.title(&self.document_registry);
        let menu_definitions = self.menu_definitions();
        let mut sidebar_items = vec![SidebarItem::folder("workspace", "Open Documents", 0, true)];
        sidebar_items.extend(self.documents.iter().map(|document| {
            SidebarItem::file(
                document.stable_key(),
                document.title(&self.document_registry),
                1,
            )
        }));
        let status_left = self.lifecycle_notice.as_ref().map_or_else(
            || {
                format!(
                    "{}{}",
                    active_title,
                    if active.is_dirty(&self.document_registry) {
                        " — Modified"
                    } else {
                        ""
                    }
                )
            },
            |notice| format!("{active_title} — {notice}"),
        );
        let source_label = self
            .document_registry
            .get(active.id)
            .map_or("Unknown", |record| match record.source() {
                DocumentSource::Untitled { .. } => "Untitled",
                DocumentSource::File(_) => "File",
                DocumentSource::Virtual { .. } => "Virtual",
            });
        EditorShellState {
            menus: menu_definitions
                .iter()
                .map(|menu| ShellMenu::new(menu.id.clone(), menu.title.clone()))
                .collect(),
            tabs: self
                .documents
                .iter()
                .map(|document| ShellTab {
                    id: document.stable_key(),
                    title: document.title(&self.document_registry).to_owned(),
                    is_dirty: document.is_dirty(&self.document_registry),
                    is_closable: self.documents.len() > 1,
                })
                .collect(),
            active_menu_id: self.menu.active_menu_id.clone(),
            active_tab_id: Some(active.stable_key()),
            sidebar_items,
            selected_sidebar_id: self.selected_sidebar_id.clone(),
            sidebar_is_visible: self.sidebar_is_visible,
            sidebar_width: 236,
            status_left,
            status_right: format!(
                "Ln {}, Col {}  UTF-8  {source_label}",
                active.editor.caret().line_index.saturating_add(1),
                active.editor.caret().utf8_column.saturating_add(1)
            ),
            editor_children: vec![self.text_id.clone()],
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

    fn top_level_menu_at(&self, position: PointI) -> Option<String> {
        self.create_shell()
            .ok()
            .and_then(|shell| match shell.semantic_hit_test(position) {
                Some(EditorShellHit::Menu(menu_id)) => Some(menu_id),
                Some(
                    EditorShellHit::Tab(_)
                    | EditorShellHit::CloseTab(_)
                    | EditorShellHit::SidebarItem(_)
                    | EditorShellHit::Editor,
                )
                | None => None,
            })
    }

    fn transient_surface_count(&self) -> usize {
        usize::from(self.menu.is_open())
            .saturating_add(usize::from(self.palette.is_some()))
            .saturating_add(usize::from(self.find.is_some()))
    }

    fn text_viewport_size(&self, bounds: RectI) -> SizeI {
        let style = TextViewStyle::from_theme(self.theme);
        let inner = bounds.inset(style.content_insets);
        SizeI::new(
            inner.width.saturating_sub(style.gutter_width).max(1),
            inner.height.max(1),
        )
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
            format!("Editor for {}", document.title(&self.document_registry)),
            self.text_is_focused
                && self.palette.is_none()
                && self.find.is_none()
                && !self.menu.is_open(),
            true,
        )
    }

    fn current_text_view(&self) -> Option<TextView> {
        let document_id = self.active_document().stable_key();
        self.text_layouts
            .get(&document_id)
            .and_then(TextLayoutCache::snapshot)
            .cloned()
            .map(|layout| self.text_view_from_layout(layout))
    }

    fn update_active_text_layout(
        &mut self,
        request: TextLayoutRequest,
        scroll_y: i32,
        viewport_height: u32,
    ) -> Result<TextLayoutSnapshot, ApplicationError> {
        let active_index = self
            .active_index
            .min(self.documents.len().saturating_sub(1));
        let document_id = self.documents[active_index].stable_key();
        let revision = self.documents[active_index].editor.edit_revision();
        let document = self.documents[active_index].editor.document();
        let cache = self.text_layouts.entry(document_id).or_default();
        Ok(cache
            .update(
                &mut self.engine,
                document,
                revision,
                request,
                scroll_y,
                viewport_height,
            )?
            .clone())
    }

    fn aggregate_text_cache_stats(&self) -> TextLayoutCacheStats {
        let mut aggregate = TextLayoutCacheStats::default();
        for cache in self.text_layouts.values() {
            aggregate.accumulate(cache.stats());
        }
        aggregate
    }

    fn report_cache_metrics(&self) {
        if self.frame_build_count != 1 && !self.frame_build_count.is_multiple_of(32) {
            return;
        }
        let text = self.aggregate_text_cache_stats();
        let labels = self.label_cache.stats();
        eprintln!(
            concat!(
                "[luna-editor cache] frames={} ",
                "text={{layout_hits:{}, layout_misses:{}, raster_hits:{}, raster_misses:{}}} ",
                "labels={{hits:{}, misses:{}, entries:{}}}",
            ),
            self.frame_build_count,
            text.layout_hits,
            text.layout_misses,
            text.raster_hits,
            text.raster_misses,
            labels.hits,
            labels.misses,
            labels.entries,
        );
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
        self.append_colored_label(
            display_list,
            id,
            text,
            bounds,
            alignment,
            font_size,
            self.theme.foreground,
        )
    }

    fn append_colored_label(
        &mut self,
        display_list: &mut DisplayList,
        id: &str,
        text: &str,
        bounds: RectI,
        alignment: TextAlignment,
        font_size: f32,
        color: Rgba8,
    ) -> Result<(), ApplicationError> {
        if bounds.is_empty() {
            return Ok(());
        }
        let node_id = NodeId::new(id)?;
        let layout = self.label_cache.layout(
            &mut self.engine,
            id,
            text,
            bounds.width,
            font_size,
            font_size + 6.0,
            color,
        )?;
        let label = TextLabel::new(node_id, bounds, text, layout, alignment);
        label.build_display_list(display_list);
        Ok(())
    }

    fn active_menu_definition(&self) -> Option<MenuDefinition> {
        let active_id = self.menu.active_menu_id.as_deref()?;
        self.menu_definitions()
            .into_iter()
            .find(|definition| definition.id == active_id)
    }

    fn create_dropdown_menu(
        &self,
        shell: &EditorShell,
    ) -> Result<Option<DropdownMenu>, ApplicationError> {
        let Some(definition) = self.active_menu_definition() else {
            return Ok(None);
        };
        let Some(anchor) = shell
            .layout()
            .menus
            .iter()
            .find(|frame| frame.id == definition.id)
            .map(|frame| frame.bounds)
        else {
            return Ok(None);
        };
        Ok(Some(DropdownMenu::new(
            self.menu_id.clone(),
            self.viewport,
            anchor,
            self.theme,
            definition,
            self.menu.selected_index,
        )?))
    }

    fn open_menu(&mut self, menu_id: &str) -> HostControl {
        if self.menu.active_menu_id.as_deref() == Some(menu_id)
            && self.palette.is_none()
            && self.find.is_none()
        {
            self.menu.close();
            self.text_is_focused = true;
            eprintln!("[luna-editor menu] close={menu_id}");
            return HostControl::Invalidate(InvalidationClass::TextOverlay);
        }
        let Some(definition) = self
            .menu_definitions()
            .into_iter()
            .find(|definition| definition.id == menu_id)
        else {
            return HostControl::Continue;
        };
        self.palette = None;
        self.find = None;
        self.menu.open(&definition);
        self.text_is_focused = false;
        eprintln!("[luna-editor menu] open={menu_id} palette=false find=false");
        debug_assert_eq!(self.transient_surface_count(), 1);
        HostControl::Invalidate(InvalidationClass::TextOverlay)
    }

    fn close_menu(&mut self) -> HostControl {
        if !self.menu.is_open() {
            return HostControl::Continue;
        }
        self.menu.close();
        self.text_is_focused = self.palette.is_none() && self.find.is_none();
        HostControl::Invalidate(InvalidationClass::TextOverlay)
    }

    fn switch_menu(&mut self, delta: i32) -> HostControl {
        let definitions = self.menu_definitions();
        let Some(active_id) = self.menu.active_menu_id.as_deref() else {
            return HostControl::Continue;
        };
        let Some(current) = definitions
            .iter()
            .position(|definition| definition.id == active_id)
        else {
            return HostControl::Continue;
        };
        let count = definitions.len();
        if count == 0 {
            return HostControl::Continue;
        }
        let next = if delta < 0 {
            if current == 0 {
                count.saturating_sub(1)
            } else {
                current.saturating_sub(1)
            }
        } else {
            current.saturating_add(1) % count
        };
        self.menu.open(&definitions[next]);
        HostControl::Invalidate(InvalidationClass::TextOverlay)
    }

    fn handle_menu_key(&mut self, key: NamedKey) -> HostControl {
        let Some(definition) = self.active_menu_definition() else {
            return HostControl::Continue;
        };
        match key {
            NamedKey::Escape => self.close_menu(),
            NamedKey::ArrowDown => {
                self.menu.select_next(&definition);
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::ArrowUp => {
                self.menu.select_previous(&definition);
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::ArrowLeft => self.switch_menu(-1),
            NamedKey::ArrowRight => self.switch_menu(1),
            NamedKey::Home => {
                self.menu.select_first(&definition);
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::End => {
                self.menu.select_last(&definition);
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::Enter => {
                let command = self.menu.selected_command(&definition).map(str::to_owned);
                command.map_or(HostControl::Continue, |command| {
                    self.execute_command(&command)
                })
            }
            NamedKey::Tab
            | NamedKey::Backspace
            | NamedKey::Delete
            | NamedKey::PageUp
            | NamedKey::PageDown => HostControl::Continue,
        }
    }

    fn open_palette(&mut self) -> HostControl {
        self.menu.close();
        self.find = None;
        let items = self.palette_items();
        self.palette = Some(CommandPaletteState {
            query: String::new(),
            items,
            selected_index: 0,
        });
        self.text_is_focused = false;
        debug_assert_eq!(self.transient_surface_count(), 1);
        HostControl::Invalidate(InvalidationClass::TextOverlay)
    }

    fn open_find(&mut self) -> HostControl {
        self.find = Some(FindPanelState {
            replacement_is_visible: true,
            ..FindPanelState::default()
        });
        self.palette = None;
        self.menu.close();
        self.text_is_focused = false;
        self.refresh_find_matches();
        debug_assert_eq!(self.transient_surface_count(), 1);
        HostControl::Invalidate(InvalidationClass::TextOverlay)
    }

    fn execute_command(&mut self, command: &str) -> HostControl {
        self.palette = None;
        self.menu.close();
        let invalidation = match command {
            "new-file" => {
                self.new_document();
                InvalidationClass::WidgetLayout
            }
            "save" => {
                self.save_active();
                InvalidationClass::TextOverlay
            }
            "close-tab" if self.documents.len() > 1 => {
                self.close_active();
                InvalidationClass::WidgetLayout
            }
            "close-tab" => return HostControl::Continue,
            "find" | "replace" => return self.open_find(),
            "find-next" if self.find.is_some() && !self.find_matches.is_empty() => {
                self.select_find_match(1);
                InvalidationClass::TextOverlay
            }
            "find-previous" if self.find.is_some() && !self.find_matches.is_empty() => {
                self.select_find_match(-1);
                InvalidationClass::TextOverlay
            }
            "find-next" | "find-previous" => return HostControl::Continue,
            "select-all" => {
                let end = self.active_document().editor.document().end_location();
                self.active_document_mut()
                    .editor
                    .set_selection(TextRange::new(TextLocation::default(), end));
                self.reveal_caret_on_next_frame = true;
                InvalidationClass::TextOverlay
            }
            "toggle-sidebar" => {
                self.sidebar_is_visible = !self.sidebar_is_visible;
                InvalidationClass::WidgetLayout
            }
            "theme" => {
                self.theme = if self.theme == Theme::luna_dark() {
                    Theme::luna_light()
                } else {
                    Theme::luna_dark()
                };
                self.label_cache.clear();
                for cache in self.text_layouts.values_mut() {
                    cache.invalidate_raster();
                }
                InvalidationClass::FullFrame
            }
            "command-palette" => return self.open_palette(),
            "exit" => return HostControl::Exit,
            _ => return HostControl::Continue,
        };
        self.text_is_focused = self.find.is_none();
        HostControl::Invalidate(invalidation)
    }

    fn new_document(&mut self) {
        let editor = EditableText::new(String::new());
        let id = self
            .document_registry
            .create_untitled(editor.edit_revision());
        self.documents.push(DemoDocument {
            id,
            editor,
            scroll: TextScroll::default(),
        });
        self.active_index = self.documents.len().saturating_sub(1);
        self.selected_sidebar_id = Some(id.stable_key());
        self.lifecycle_notice = Some(format!(
            "Created {}; Save requires a destination",
            self.active_document().title(&self.document_registry)
        ));
        self.reveal_caret_on_next_frame = true;
    }

    fn save_active(&mut self) {
        let document = self.active_document();
        let requirement = self
            .document_registry
            .get(document.id)
            .map(|record| record.save_requirement(document.editor.edit_revision()));
        self.lifecycle_notice = match requirement {
            Some(SaveRequirement::None) => Some("Document is already saved".to_owned()),
            Some(SaveRequirement::SaveAs) => {
                Some("Save As is required; filesystem adapters arrive in M3.2b".to_owned())
            }
            Some(SaveRequirement::WriteFile { .. }) => {
                Some("File write adapter arrives in M3.2b".to_owned())
            }
            Some(SaveRequirement::Unsupported) => {
                Some("Generated demo documents do not have a writable file target".to_owned())
            }
            None => Some("Document lifecycle record is unavailable".to_owned()),
        };
    }

    fn close_active(&mut self) {
        if self.documents.len() <= 1 {
            self.lifecycle_notice = Some("At least one document must remain open".to_owned());
            return;
        }
        let active_id = self.active_document().id;
        let revision = self.active_document().editor.edit_revision();
        let close_requirement = self
            .document_registry
            .get(active_id)
            .map(|record| record.close_requirement(revision));
        if close_requirement == Some(CloseRequirement::SaveOrDiscard) {
            self.lifecycle_notice = Some(format!(
                "{} has unsaved changes; Save, Discard, or Cancel is required",
                self.active_document().title(&self.document_registry)
            ));
            return;
        }
        let removed_key = active_id.stable_key();
        self.documents.remove(self.active_index);
        let _ = self.document_registry.remove(active_id);
        self.text_layouts.remove(&removed_key);
        self.active_index = self
            .active_index
            .min(self.documents.len().saturating_sub(1));
        self.selected_sidebar_id = Some(self.active_document().stable_key());
        self.lifecycle_notice = None;
        self.reveal_caret_on_next_frame = true;
    }

    fn activate_document(&mut self, id: &str) {
        if let Some(index) = self
            .documents
            .iter()
            .position(|document| document.stable_key() == id)
        {
            self.active_index = index;
            self.selected_sidebar_id = Some(id.to_owned());
            self.lifecycle_notice = None;
            self.reveal_caret_on_next_frame = true;
            self.text_is_focused =
                self.palette.is_none() && self.find.is_none() && !self.menu.is_open();
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
        let invalidation = match key {
            NamedKey::Escape => {
                self.palette = None;
                self.text_is_focused = true;
                InvalidationClass::TextOverlay
            }
            NamedKey::ArrowDown => {
                if let Some(state) = self.palette.as_mut() {
                    state.select_next();
                }
                InvalidationClass::PaintOverlay
            }
            NamedKey::ArrowUp => {
                if let Some(state) = self.palette.as_mut() {
                    state.select_previous();
                }
                InvalidationClass::PaintOverlay
            }
            NamedKey::Backspace => {
                if let Some(state) = self.palette.as_mut() {
                    let _ = state.query.pop();
                    state.selected_index = 0;
                    state.normalize_selection();
                }
                InvalidationClass::TextOverlay
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
                InvalidationClass::PaintOverlay
            }
            NamedKey::Tab
            | NamedKey::Delete
            | NamedKey::ArrowLeft
            | NamedKey::ArrowRight
            | NamedKey::Home
            | NamedKey::End
            | NamedKey::PageUp
            | NamedKey::PageDown => return HostControl::Continue,
        };
        HostControl::Invalidate(invalidation)
    }

    fn handle_find_key(&mut self, key: NamedKey) -> HostControl {
        let invalidation = match key {
            NamedKey::Escape => {
                self.find = None;
                self.text_is_focused = true;
                InvalidationClass::TextOverlay
            }
            NamedKey::Tab => {
                if let Some(find) = self.find.as_mut() {
                    find.active_field = match find.active_field {
                        FindField::Query => FindField::Replacement,
                        FindField::Replacement => FindField::Query,
                    };
                }
                InvalidationClass::PaintOverlay
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
                InvalidationClass::TextOverlay
            }
            NamedKey::Enter | NamedKey::ArrowDown => {
                self.select_find_match(1);
                InvalidationClass::TextOverlay
            }
            NamedKey::ArrowUp => {
                self.select_find_match(-1);
                InvalidationClass::TextOverlay
            }
            NamedKey::Delete
            | NamedKey::ArrowLeft
            | NamedKey::ArrowRight
            | NamedKey::Home
            | NamedKey::End
            | NamedKey::PageUp
            | NamedKey::PageDown => return HostControl::Continue,
        };
        HostControl::Invalidate(invalidation)
    }

    fn handle_editor_key(&mut self, key: NamedKey, modifiers: Modifiers) -> HostControl {
        let extending = modifiers.contains(Modifiers::SHIFT);
        let viewport_height = self.text_viewport_size(self.last_editor_bounds).height;
        let maximum_scroll_y = self
            .current_text_view()
            .map_or(i32::MAX, |view| view.maximum_scroll().y);
        let previous_caret = self.active_document().editor.caret();
        let previous_selection = self.active_document().editor.selection();
        let previous_revision = self.active_document().editor.edit_revision();
        let previous_scroll = self.active_document().scroll;
        let mut reveal = true;
        let invalidation = match key {
            NamedKey::ArrowLeft => {
                self.active_document_mut().editor.move_backward(extending);
                InvalidationClass::TextOverlay
            }
            NamedKey::ArrowRight => {
                self.active_document_mut().editor.move_forward(extending);
                InvalidationClass::TextOverlay
            }
            NamedKey::ArrowUp => {
                self.active_document_mut().editor.move_up(extending);
                InvalidationClass::TextOverlay
            }
            NamedKey::ArrowDown => {
                self.active_document_mut().editor.move_down(extending);
                InvalidationClass::TextOverlay
            }
            NamedKey::Home => {
                self.active_document_mut()
                    .editor
                    .move_to_line_start(extending);
                InvalidationClass::TextOverlay
            }
            NamedKey::End => {
                self.active_document_mut()
                    .editor
                    .move_to_line_end(extending);
                InvalidationClass::TextOverlay
            }
            NamedKey::Backspace => {
                let _ = self.active_document_mut().editor.delete_backward();
                InvalidationClass::TextLayout
            }
            NamedKey::Delete => {
                let _ = self.active_document_mut().editor.delete_forward();
                InvalidationClass::TextLayout
            }
            NamedKey::Enter => {
                let _ = self.active_document_mut().editor.insert_newline();
                InvalidationClass::TextLayout
            }
            NamedKey::PageUp => {
                let amount = i32::try_from(viewport_height).unwrap_or(i32::MAX);
                let current_y = self.active_document().scroll.y;
                self.active_document_mut().scroll.y = current_y.saturating_sub(amount).max(0);
                reveal = false;
                InvalidationClass::TextRaster
            }
            NamedKey::PageDown => {
                let amount = i32::try_from(viewport_height).unwrap_or(i32::MAX);
                let current_y = self.active_document().scroll.y;
                self.active_document_mut().scroll.y = current_y
                    .saturating_add(amount)
                    .min(maximum_scroll_y.max(0));
                reveal = false;
                InvalidationClass::TextRaster
            }
            NamedKey::Escape | NamedKey::Tab => return HostControl::Continue,
        };
        let changed = match invalidation {
            InvalidationClass::TextLayout => {
                self.active_document().editor.edit_revision() != previous_revision
            }
            InvalidationClass::TextRaster => self.active_document().scroll != previous_scroll,
            InvalidationClass::TextOverlay => {
                self.active_document().editor.caret() != previous_caret
                    || self.active_document().editor.selection() != previous_selection
            }
            _ => true,
        };
        if !changed {
            return HostControl::Continue;
        }
        if invalidation == InvalidationClass::TextLayout {
            self.lifecycle_notice = None;
        }
        self.reveal_caret_on_next_frame = reveal;
        HostControl::Invalidate(invalidation)
    }

    fn handle_open_menu_pointer(&mut self, pointer: &PointerEvent) -> HostControl {
        let Ok(shell) = self.create_shell() else {
            return HostControl::Continue;
        };
        let top_menu = match shell.semantic_hit_test(pointer.position) {
            Some(EditorShellHit::Menu(menu_id)) => Some(menu_id),
            Some(
                EditorShellHit::Tab(_)
                | EditorShellHit::CloseTab(_)
                | EditorShellHit::SidebarItem(_)
                | EditorShellHit::Editor,
            )
            | None => None,
        };
        let dropdown = self.create_dropdown_menu(&shell).ok().flatten();
        match pointer.kind {
            PointerEventKind::Moved => {
                if let Some(menu_id) = top_menu
                    && self.menu.active_menu_id.as_deref() != Some(menu_id.as_str())
                {
                    return self.open_menu(&menu_id);
                }
                let Some(dropdown) = dropdown else {
                    return HostControl::Continue;
                };
                let Some(item_index) = dropdown.item_index_at(pointer.position) else {
                    return HostControl::Continue;
                };
                let definition = dropdown.definition().clone();
                if self.menu.select_hovered(&definition, item_index) {
                    HostControl::Invalidate(InvalidationClass::PaintOverlay)
                } else {
                    HostControl::Continue
                }
            }
            PointerEventKind::Pressed(PointerButton::Primary) => {
                if let Some(menu_id) = top_menu {
                    return self.open_menu(&menu_id);
                }
                let Some(dropdown) = dropdown else {
                    return self.close_menu();
                };
                if let Some(command) = dropdown.command_at(pointer.position).map(str::to_owned) {
                    return self.execute_command(&command);
                }
                if dropdown.contains(pointer.position) {
                    HostControl::Continue
                } else {
                    self.close_menu()
                }
            }
            PointerEventKind::Pressed(_)
            | PointerEventKind::Released(_)
            | PointerEventKind::Left => HostControl::Continue,
        }
    }

    fn apply_pointer_to_text(&mut self, position: PointI, extending: bool) -> bool {
        let Some(view) = self.current_text_view() else {
            return false;
        };
        let Some(location) = view.text_hit_test(position) else {
            return false;
        };
        let previous_caret = self.active_document().editor.caret();
        let previous_selection = self.active_document().editor.selection();
        let was_focused = self.text_is_focused;
        if extending {
            let anchor = self.drag_anchor.unwrap_or(previous_caret);
            self.active_document_mut()
                .editor
                .set_selection(TextRange::new(anchor, location));
        } else {
            self.active_document_mut().editor.set_caret(location);
            self.drag_anchor = Some(location);
        }
        self.text_is_focused = true;
        self.reveal_caret_on_next_frame = true;
        previous_caret != self.active_document().editor.caret()
            || previous_selection != self.active_document().editor.selection()
            || !was_focused
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
                .find(|document| document.stable_key() == frame.id)
                .map_or(frame.title.clone(), |document| {
                    format!(
                        "{}{}",
                        document.title(&self.document_registry),
                        if document.is_dirty(&self.document_registry) {
                            " •"
                        } else {
                            ""
                        }
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

    fn append_dropdown_labels(
        &mut self,
        menu: &DropdownMenu,
        display_list: &mut DisplayList,
    ) -> Result<(), ApplicationError> {
        for row in &menu.layout().rows {
            if row.is_separator {
                continue;
            }
            let color = if row.is_enabled {
                self.theme.foreground
            } else {
                self.theme.muted_foreground()
            };
            self.append_colored_label(
                display_list,
                &format!(
                    "m3-editor-dropdown-{}-title-{}",
                    menu.definition().id,
                    row.item_index
                ),
                &row.title,
                RectI::new(
                    row.bounds.x.saturating_add(28),
                    row.bounds.y,
                    row.bounds.width.saturating_sub(128),
                    row.bounds.height,
                ),
                TextAlignment::Leading,
                13.0,
                color,
            )?;
            if !row.shortcut.is_empty() {
                self.append_colored_label(
                    display_list,
                    &format!(
                        "m3-editor-dropdown-{}-shortcut-{}",
                        menu.definition().id,
                        row.item_index
                    ),
                    &row.shortcut,
                    RectI::new(
                        i32::try_from(row.bounds.right().saturating_sub(94))
                            .unwrap_or(row.bounds.x),
                        row.bounds.y,
                        84,
                        row.bounds.height,
                    ),
                    TextAlignment::Trailing,
                    12.0,
                    color,
                )?;
            }
        }
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
        let text_viewport = self.text_viewport_size(self.last_editor_bounds);
        let request =
            TextLayoutRequest::new(text_viewport.width, 15.0, 22.0, self.theme.foreground);
        let mut layout = self.update_active_text_layout(request, scroll.y, text_viewport.height)?;
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
            let revealed_scroll = provisional.scroll_revealing_caret();
            if revealed_scroll != scroll {
                scroll = revealed_scroll;
                self.active_document_mut().scroll = scroll;
                layout = self.update_active_text_layout(request, scroll.y, text_viewport.height)?;
            }
            self.reveal_caret_on_next_frame = false;
        }
        let text_view = TextView::new(
            self.text_id.clone(),
            self.last_editor_bounds,
            document,
            layout,
            caret,
            selection,
            scroll,
            TextViewStyle::from_theme(self.theme),
            format!(
                "Editor for {}",
                self.active_document().title(&self.document_registry)
            ),
            self.text_is_focused
                && self.palette.is_none()
                && self.find.is_none()
                && !self.menu.is_open(),
            true,
        );

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
        debug_assert!(self.transient_surface_count() <= 1);

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
        if let Some(menu) = self.create_dropdown_menu(&shell)? {
            menu.build_display_list(&mut display_list);
            nodes.extend(menu.accessibility_nodes());
            root_children.push(self.menu_id.clone());
            self.append_dropdown_labels(&menu, &mut display_list)?;
        }
        nodes[0] =
            AccessibilityNode::new(self.root_id.clone(), AccessibilityRole::Window, viewport)
                .with_label("Luna UI Rust editor demo window")
                .with_children(root_children);
        self.frame_build_count = self.frame_build_count.saturating_add(1);
        self.report_cache_metrics();
        Ok(UiFrame::from_parts(
            display_list,
            self.root_id.clone(),
            nodes,
        )?)
    }

    fn handle_input(&mut self, event: InputEvent) -> HostControl {
        match event {
            InputEvent::Keyboard(keyboard) if keyboard.is_pressed => {
                if self.menu.is_open() {
                    if let Key::Named(key) = &keyboard.key {
                        return self.handle_menu_key(*key);
                    }
                    if matches!(&keyboard.key, Key::Character(value) if value == " ") {
                        let command = self.active_menu_definition().and_then(|definition| {
                            self.menu.selected_command(&definition).map(str::to_owned)
                        });
                        return command.map_or(HostControl::Continue, |command| {
                            self.execute_command(&command)
                        });
                    }
                    return HostControl::Continue;
                }
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
                        return HostControl::Invalidate(InvalidationClass::TextOverlay);
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
                        return HostControl::Invalidate(InvalidationClass::TextOverlay);
                    }
                    return HostControl::Continue;
                }

                let command_modified = keyboard.modifiers.contains(Modifiers::SUPER)
                    || (keyboard.modifiers.contains(Modifiers::CONTROL)
                        && !keyboard.modifiers.contains(Modifiers::ALT));
                if command_modified {
                    let command = match &keyboard.key {
                        Key::Character(value) if value.eq_ignore_ascii_case("p") => {
                            Some("command-palette")
                        }
                        Key::Character(value) if value.eq_ignore_ascii_case("f") => Some("find"),
                        Key::Character(value) if value.eq_ignore_ascii_case("h") => Some("replace"),
                        Key::Character(value)
                            if value.eq_ignore_ascii_case("s")
                                && !keyboard.modifiers.contains(Modifiers::SHIFT) =>
                        {
                            Some("save")
                        }
                        Key::Character(value) if value.eq_ignore_ascii_case("n") => {
                            Some("new-file")
                        }
                        Key::Character(value) if value.eq_ignore_ascii_case("b") => {
                            Some("toggle-sidebar")
                        }
                        Key::Character(value) if value.eq_ignore_ascii_case("w") => {
                            Some("close-tab")
                        }
                        Key::Character(value) if value.eq_ignore_ascii_case("a") => {
                            Some("select-all")
                        }
                        _ => None,
                    };
                    return command.map_or(HostControl::Continue, |command| {
                        self.execute_command(command)
                    });
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
                    let result = self.active_document_mut().editor.insert_text(text);
                    if result.did_change {
                        self.lifecycle_notice = None;
                        self.reveal_caret_on_next_frame = true;
                        return HostControl::Invalidate(InvalidationClass::TextLayout);
                    }
                }
            }
            InputEvent::Text(text) => {
                if self.menu.is_open() {
                    return HostControl::Continue;
                }
                let invalidation = if let Some(state) = self.palette.as_mut() {
                    state.query.push_str(&text);
                    state.normalize_selection();
                    InvalidationClass::TextOverlay
                } else if self.find.is_some() {
                    if let Some(find) = self.find.as_mut() {
                        match find.active_field {
                            FindField::Query => find.query.push_str(&text),
                            FindField::Replacement => find.replacement.push_str(&text),
                        }
                    }
                    self.refresh_find_matches();
                    InvalidationClass::TextOverlay
                } else {
                    let result = self.active_document_mut().editor.insert_text(&text);
                    if !result.did_change {
                        return HostControl::Continue;
                    }
                    self.lifecycle_notice = None;
                    self.reveal_caret_on_next_frame = true;
                    InvalidationClass::TextLayout
                };
                return HostControl::Invalidate(invalidation);
            }
            InputEvent::Pointer(pointer) => {
                if pointer.kind == PointerEventKind::Pressed(PointerButton::Primary)
                    && let Some(menu_id) = self.top_level_menu_at(pointer.position)
                {
                    return self.open_menu(&menu_id);
                }
                if self.menu.is_open() {
                    return self.handle_open_menu_pointer(&pointer);
                }
                if pointer.kind == PointerEventKind::Pressed(PointerButton::Primary) {
                    if let Some(state) = self.palette.clone() {
                        if let Ok(palette) = CommandPalette::new(
                            self.palette_id.clone(),
                            self.viewport,
                            self.theme,
                            state,
                        ) {
                            if let Some(command) =
                                palette.command_at(pointer.position).map(str::to_owned)
                            {
                                return self.execute_command(&command);
                            }
                            if !palette.layout().panel.contains(pointer.position) {
                                self.palette = None;
                                self.text_is_focused = true;
                                return HostControl::Invalidate(InvalidationClass::TextOverlay);
                            }
                        }
                        return HostControl::Continue;
                    }
                    if let Some(state) = self.find.clone() {
                        if let Ok(panel) =
                            FindPanel::new(self.find_id.clone(), self.viewport, self.theme, state)
                        {
                            let layout = panel.layout();
                            if layout.close.contains(pointer.position) {
                                self.find = None;
                                self.text_is_focused = true;
                                return HostControl::Invalidate(InvalidationClass::TextOverlay);
                            }
                            if layout.next.contains(pointer.position) {
                                self.select_find_match(1);
                                return HostControl::Invalidate(InvalidationClass::TextOverlay);
                            }
                            if layout.previous.contains(pointer.position) {
                                self.select_find_match(-1);
                                return HostControl::Invalidate(InvalidationClass::TextOverlay);
                            }
                            if layout.query.contains(pointer.position) {
                                if let Some(find) = self.find.as_mut() {
                                    find.active_field = FindField::Query;
                                }
                                return HostControl::Invalidate(InvalidationClass::PaintOverlay);
                            }
                            if layout.replacement.contains(pointer.position) {
                                if let Some(find) = self.find.as_mut() {
                                    find.active_field = FindField::Replacement;
                                }
                                return HostControl::Invalidate(InvalidationClass::PaintOverlay);
                            }
                        }
                        return HostControl::Continue;
                    }
                    if let Ok(shell) = self.create_shell() {
                        match shell.semantic_hit_test(pointer.position) {
                            Some(EditorShellHit::Tab(id)) => {
                                self.activate_document(&id);
                                return HostControl::Invalidate(InvalidationClass::WidgetLayout);
                            }
                            Some(EditorShellHit::CloseTab(id)) => {
                                self.activate_document(&id);
                                self.close_active();
                                return HostControl::Invalidate(InvalidationClass::WidgetLayout);
                            }
                            Some(EditorShellHit::SidebarItem(id)) => {
                                self.activate_document(&id);
                                return HostControl::Invalidate(InvalidationClass::WidgetLayout);
                            }
                            Some(EditorShellHit::Menu(_)) => {}
                            Some(EditorShellHit::Editor) => {
                                let extending = pointer.modifiers.contains(Modifiers::SHIFT);
                                if self.apply_pointer_to_text(pointer.position, extending) {
                                    return HostControl::Invalidate(InvalidationClass::TextOverlay);
                                }
                            }
                            None => {}
                        }
                    }
                    return HostControl::Continue;
                }
                if pointer.kind == PointerEventKind::Moved
                    && self.drag_anchor.is_some()
                    && self.apply_pointer_to_text(pointer.position, true)
                {
                    return HostControl::Invalidate(InvalidationClass::TextOverlay);
                }
                if matches!(
                    pointer.kind,
                    PointerEventKind::Released(PointerButton::Primary) | PointerEventKind::Left
                ) {
                    self.drag_anchor = None;
                }
            }
            InputEvent::Scroll(scroll)
                if self.palette.is_none() && self.find.is_none() && !self.menu.is_open() =>
            {
                if let Some(view) = self.current_text_view() {
                    let maximum = view.maximum_scroll();
                    let (delta_x, delta_y) =
                        if scroll.modifiers.contains(Modifiers::SHIFT) && scroll.delta_x == 0 {
                            (scroll.delta_y, 0)
                        } else {
                            (scroll.delta_x, scroll.delta_y)
                        };
                    let previous = self.active_document().scroll;
                    let changed = {
                        let document = self.active_document_mut();
                        document
                            .scroll
                            .scroll_by(delta_x, delta_y, maximum.x, maximum.y);
                        document.scroll != previous
                    };
                    self.reveal_caret_on_next_frame = false;
                    if changed {
                        return HostControl::Invalidate(InvalidationClass::TextRaster);
                    }
                }
            }
            InputEvent::FocusGained
                if !self.text_is_focused
                    && self.palette.is_none()
                    && self.find.is_none()
                    && !self.menu.is_open() =>
            {
                self.text_is_focused = true;
                return HostControl::Invalidate(InvalidationClass::PaintOverlay);
            }
            InputEvent::FocusLost
                if self.text_is_focused || self.drag_anchor.is_some() || self.menu.is_open() =>
            {
                self.text_is_focused = false;
                self.drag_anchor = None;
                self.menu.close();
                return HostControl::Invalidate(InvalidationClass::TextOverlay);
            }
            InputEvent::Keyboard(_)
            | InputEvent::Scroll(_)
            | InputEvent::FocusGained
            | InputEvent::FocusLost => {}
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

        if self.menu.is_open()
            && let Ok(shell) = self.create_shell()
            && let Ok(Some(menu)) = self.create_dropdown_menu(&shell)
            && request.kind == AccessibilityActionKind::Click
        {
            let command = menu.command_for_node(&target).map(str::to_owned);
            if let Some(command) = command {
                return self.execute_command(&command);
            }
        }

        if let Some(state) = self.palette.clone()
            && let Ok(palette) =
                CommandPalette::new(self.palette_id.clone(), self.viewport, self.theme, state)
        {
            if &target == palette.input_node_id() {
                self.text_is_focused = false;
                return HostControl::Invalidate(InvalidationClass::Accessibility);
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
                return HostControl::Invalidate(InvalidationClass::Accessibility);
            }
            if &target == panel.replacement_node_id() {
                if let Some(find) = self.find.as_mut() {
                    find.active_field = FindField::Replacement;
                }
                return HostControl::Invalidate(InvalidationClass::Accessibility);
            }
            if request.kind == AccessibilityActionKind::Click {
                if &target == panel.previous_node_id() {
                    self.select_find_match(-1);
                    return HostControl::Invalidate(InvalidationClass::TextOverlay);
                }
                if &target == panel.next_node_id() {
                    self.select_find_match(1);
                    return HostControl::Invalidate(InvalidationClass::TextOverlay);
                }
                if &target == panel.close_node_id() {
                    self.find = None;
                    self.text_is_focused = true;
                    return HostControl::Invalidate(InvalidationClass::Accessibility);
                }
            }
        }

        if let Ok(shell) = self.create_shell() {
            match shell.semantic_target(&target) {
                Some(EditorShellHit::Menu(menu_id))
                    if request.kind == AccessibilityActionKind::Click =>
                {
                    return self.open_menu(&menu_id);
                }
                Some(EditorShellHit::Tab(id) | EditorShellHit::SidebarItem(id)) => {
                    self.activate_document(&id);
                    return HostControl::Invalidate(InvalidationClass::Accessibility);
                }
                Some(EditorShellHit::CloseTab(id))
                    if request.kind == AccessibilityActionKind::Click =>
                {
                    self.activate_document(&id);
                    self.close_active();
                    return HostControl::Invalidate(InvalidationClass::Accessibility);
                }
                Some(EditorShellHit::Editor) => {
                    self.menu.close();
                    self.palette = None;
                    self.find = None;
                    self.text_is_focused = true;
                    return HostControl::Invalidate(InvalidationClass::Accessibility);
                }
                Some(EditorShellHit::CloseTab(_) | EditorShellHit::Menu(_)) | None => {}
            }
        }

        if target == self.text_id {
            self.menu.close();
            self.palette = None;
            self.find = None;
            self.text_is_focused = true;
            return HostControl::Invalidate(InvalidationClass::Accessibility);
        }
        HostControl::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::EditorDemoApplication;
    use luna_core::PointI;
    use luna_host_winit::NativeApplication;
    use luna_input::{InputEvent, Modifiers, PointerButton, PointerEvent, PointerEventKind};
    use std::error::Error;

    #[test]
    fn dropdown_and_palette_states_remain_independent() -> Result<(), Box<dyn Error + Send + Sync>>
    {
        let mut application = EditorDemoApplication::new()?;

        let _ = application.open_menu("file");
        assert_eq!(application.menu.active_menu_id.as_deref(), Some("file"));
        assert!(application.palette.is_none());

        let _ = application.open_palette();
        assert!(!application.menu.is_open());
        assert!(application.palette.is_some());
        Ok(())
    }

    #[test]
    fn menu_heading_pointer_replaces_palette_with_dropdown()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut application = EditorDemoApplication::new()?;
        let _ = application.open_palette();
        let shell = application.create_shell()?;
        let file = shell
            .layout()
            .menus
            .iter()
            .find(|menu| menu.id == "file")
            .ok_or_else(|| std::io::Error::other("missing File menu heading"))?;
        let position = PointI::new(
            file.bounds.x.saturating_add(2),
            file.bounds.y.saturating_add(2),
        );

        let _ = application.handle_input(InputEvent::Pointer(PointerEvent {
            kind: PointerEventKind::Pressed(PointerButton::Primary),
            position,
            modifiers: Modifiers::NONE,
            timestamp_micros: 1,
        }));

        assert_eq!(application.menu.active_menu_id.as_deref(), Some("file"));
        assert!(application.palette.is_none());
        assert!(application.find.is_none());
        assert_eq!(application.transient_surface_count(), 1);
        let dropdown = application
            .create_dropdown_menu(&application.create_shell()?)?
            .ok_or_else(|| std::io::Error::other("File dropdown was not constructed"))?;
        assert_eq!(
            dropdown.layout().panel.y,
            i32::try_from(file.bounds.bottom()).unwrap_or(i32::MAX),
        );
        Ok(())
    }

    #[test]
    fn command_palette_is_not_projected_into_any_dropdown()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        let application = EditorDemoApplication::new()?;
        assert!(application.menu_definitions().iter().all(|menu| {
            menu.items.iter().all(|item| {
                item.as_command()
                    .is_none_or(|command| command.id != "command-palette")
            })
        }));
        Ok(())
    }

    #[test]
    fn palette_projection_excludes_disabled_menu_commands()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        let application = EditorDemoApplication::new()?;
        let items = application.palette_items();

        assert!(items.iter().any(|item| item.id == "new-file"));
        assert!(!items.iter().any(|item| item.id == "open"));
        assert!(!items.iter().any(|item| item.id == "save-as"));
        Ok(())
    }

    #[test]
    fn untitled_documents_use_monotonic_registry_titles() -> Result<(), Box<dyn Error + Send + Sync>>
    {
        let mut application = EditorDemoApplication::new()?;
        application.new_document();
        assert_eq!(
            application
                .document_registry
                .get(application.active_document().id)
                .map(|record| record.title()),
            Some("Untitled-1")
        );

        application.close_active();
        application.new_document();
        assert_eq!(
            application
                .document_registry
                .get(application.active_document().id)
                .map(|record| record.title()),
            Some("Untitled-2")
        );
        Ok(())
    }

    #[test]
    fn dirty_document_close_is_blocked_pending_user_decision()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut application = EditorDemoApplication::new()?;
        application.new_document();
        let count = application.documents.len();
        let result = application
            .active_document_mut()
            .editor
            .insert_text("changed");
        assert!(result.did_change);

        application.close_active();

        assert_eq!(application.documents.len(), count);
        assert!(
            application
                .lifecycle_notice
                .as_deref()
                .is_some_and(|notice| { notice.contains("Save, Discard, or Cancel") })
        );
        Ok(())
    }

    #[test]
    fn clean_close_removes_document_and_registry_record() -> Result<(), Box<dyn Error + Send + Sync>>
    {
        let mut application = EditorDemoApplication::new()?;
        application.new_document();
        let id = application.active_document().id;

        application.close_active();

        assert!(
            application
                .documents
                .iter()
                .all(|document| document.id != id)
        );
        assert!(application.document_registry.get(id).is_none());
        Ok(())
    }

    #[test]
    fn virtual_document_save_does_not_fake_a_successful_write()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut application = EditorDemoApplication::new()?;
        let result = application
            .active_document_mut()
            .editor
            .insert_text("changed");
        assert!(result.did_change);
        assert!(
            application
                .active_document()
                .is_dirty(&application.document_registry)
        );

        application.save_active();

        assert!(
            application
                .active_document()
                .is_dirty(&application.document_registry)
        );
        assert!(
            application
                .lifecycle_notice
                .as_deref()
                .is_some_and(|notice| { notice.contains("do not have a writable file target") })
        );
        Ok(())
    }

    #[test]
    fn clean_untitled_save_reports_save_as_requirement() -> Result<(), Box<dyn Error + Send + Sync>>
    {
        let mut application = EditorDemoApplication::new()?;
        application.new_document();

        let _ = application.execute_command("save");

        assert!(
            application
                .lifecycle_notice
                .as_deref()
                .is_some_and(|notice| { notice.contains("Save As is required") })
        );
        Ok(())
    }
}
