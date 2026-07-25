// SPDX-License-Identifier: MPL-2.0

//! Native M3.2e editor integration harness for Luna UI Rust.
//!
//! This application mirrors the purpose of Swift LunaUITestApp's default editor mode: reusable
//! shell anatomy and editor text are exercised together without embedding Moth Text product
//! policy in Luna. It provides menus, tabs, a project sidebar, status chrome, editable text,
//! real dropdown menus, a separate command palette, find panel, document lifecycle tracking,
//! UTF-8 open/save services, native dialog boundaries, accessibility,
//! retained document layouts, recent-file projection, UI-thread external observation, real
//! workspace trees, controlled workspace mutations, persistent session restoration, overscanned
//! glyph rasters, and stable-slot chrome-label caching.
//!
//! Shortcuts: Control-P command palette, Control-O open, Control-F find, Control-H replace,
//! Control-S save, Control-Shift-S Save As, Control-Shift-O Open Folder, Control-Shift-R refresh
//! workspace, Control-N new document, Control-B sidebar, Control-W close tab, Control-A select all,
//! and Escape
//! closes the active menu/overlay or exits when no transient surface is open.

use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{InsetsI, NodeId, PointI, RectI, SizeI};
use luna_document_services::{
    DirtyCloseChoice, DocumentDialogService, FileObservation, FileServiceError,
    FileServiceErrorKind, SaveConflictChoice, StdTextFileService, SystemDialogService,
    TextFileService, WorkspaceCollisionChoice, WorkspaceDeleteChoice, WorkspaceDirtyDeleteChoice,
    WritePrecondition,
};
use luna_documents::{
    CloseRequirement, DocumentId, DocumentRecord, DocumentRegistry, DocumentSource, ExternalState,
    FileIdentity, OpenFileOutcome, RecentFileList, SaveRequirement,
};
use luna_host_winit::{
    AccessibilityActionKind, AccessibilityActionRequest, ApplicationError, HostControl,
    InvalidationClass, NativeApplication, WindowConfig, run_native,
};
use luna_input::{
    InputEvent, Key, Modifiers, NamedKey, PointerButton, PointerEvent, PointerEventKind,
};
use luna_render::DisplayList;
#[cfg(test)]
use luna_session::MemorySessionStore;
use luna_session::{
    SessionRecentFile, SessionState, SessionStore, SessionWorkspace, StdSessionStore,
};
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
use luna_workspaces::{
    StdWorkspaceService, WorkspaceCollisionPolicy, WorkspaceErrorKind, WorkspaceModel,
    WorkspaceNodeKind, WorkspaceNodeStatus, WorkspaceRuntimeService, WorkspaceScanOptions,
};
use std::collections::HashMap;
use std::error::Error;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::Duration;

const ROOT_ID: &str = "m3-editor-window";
const SHELL_ID: &str = "m3-editor-shell";
const TEXT_ID: &str = "m3-editor-text";
const PALETTE_ID: &str = "m3-editor-palette";
const FIND_ID: &str = "m3-editor-find";
const MENU_ID: &str = "m3-editor-dropdown-menu";
const EXTERNAL_POLL_INTERVAL: Duration = Duration::from_millis(750);
const WORKSPACE_REFRESH_INTERVAL: Duration = Duration::from_millis(1_000);
const RECENT_FILE_LIMIT: usize = 8;

const README_TEXT: &str = concat!(
    "# Luna UI Rust\n\n",
    "M3.2e adds workspace operations and persistent session runtime.\n\n",
    "- Deterministic shell geometry\n",
    "- Revision-keyed document shaping\n",
    "- Viewport-band glyph rasterization\n",
    "- Stable-slot editor chrome labels\n",
    "- Shared menu and palette command IDs\n",
    "- Stable document IDs and monotonic untitled names\n",
    "- Explicit save and close requirements\n",
    "- Real Open, Save, Save As, and dirty-close resolution\n",
    "- Atomic writes with optimistic conflict checks\n",
    "- Shared accessibility geometry\n",
    "- Open Folder and recursive workspace sidebar rows\n",
    "- Stable expansion and refresh state\n",
    "- Create, rename, and delete workspace entries\n",
    "- Persistent recent files and workspace tree restoration\n\n",
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

fn file_title(identity: &FileIdentity) -> String {
    identity.path().file_name().map_or_else(
        || identity.path().display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
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
    file_service: Box<dyn TextFileService>,
    dialog_service: Box<dyn DocumentDialogService>,
    workspace_service: Box<dyn WorkspaceRuntimeService>,
    session_store: Box<dyn SessionStore>,
    workspace: Option<WorkspaceModel>,
    workspace_options: WorkspaceScanOptions,
    recent_files: RecentFileList,
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
    observation_elapsed: Duration,
    workspace_refresh_elapsed: Duration,
    frame_build_count: u64,
}

impl EditorDemoApplication {
    fn new() -> Result<Self, ApplicationError> {
        Self::with_runtime_services(
            Box::new(StdTextFileService),
            Box::new(SystemDialogService::detect()),
            Box::new(StdWorkspaceService),
            Box::new(StdSessionStore::for_application("luna-ui-rust")?),
        )
    }

    #[cfg(test)]
    fn with_services(
        file_service: Box<dyn TextFileService>,
        dialog_service: Box<dyn DocumentDialogService>,
    ) -> Result<Self, ApplicationError> {
        Self::with_all_services(file_service, dialog_service, Box::new(StdWorkspaceService))
    }

    #[cfg(test)]
    fn with_all_services(
        file_service: Box<dyn TextFileService>,
        dialog_service: Box<dyn DocumentDialogService>,
        workspace_service: Box<dyn WorkspaceRuntimeService>,
    ) -> Result<Self, ApplicationError> {
        Self::with_runtime_services(
            file_service,
            dialog_service,
            workspace_service,
            Box::new(MemorySessionStore::default()),
        )
    }

    fn with_runtime_services(
        file_service: Box<dyn TextFileService>,
        dialog_service: Box<dyn DocumentDialogService>,
        workspace_service: Box<dyn WorkspaceRuntimeService>,
        session_store: Box<dyn SessionStore>,
    ) -> Result<Self, ApplicationError> {
        let mut document_registry = DocumentRegistry::new();
        let readme_id = document_registry.register_virtual("readme", "README.md", 0)?;
        let editor_id = document_registry.register_virtual("editor", "EditorSurface.rs", 0)?;
        let theme_id = document_registry.register_virtual("theme", "Theme.json", 0)?;
        let mut application = Self {
            root_id: NodeId::new(ROOT_ID)?,
            shell_id: NodeId::new(SHELL_ID)?,
            text_id: NodeId::new(TEXT_ID)?,
            palette_id: NodeId::new(PALETTE_ID)?,
            find_id: NodeId::new(FIND_ID)?,
            menu_id: NodeId::new(MENU_ID)?,
            document_registry,
            file_service,
            dialog_service,
            workspace_service,
            session_store,
            workspace: None,
            workspace_options: WorkspaceScanOptions::default(),
            recent_files: RecentFileList::new(RECENT_FILE_LIMIT),
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
            observation_elapsed: Duration::ZERO,
            workspace_refresh_elapsed: Duration::ZERO,
            frame_build_count: 0,
        };
        application.restore_session();
        Ok(application)
    }

    fn restore_session(&mut self) {
        let state = match self.session_store.load() {
            Ok(state) => state,
            Err(error) => {
                self.lifecycle_notice = Some(format!("Session restore failed: {error}"));
                return;
            }
        };
        self.recent_files
            .restore(state.recent_files.into_iter().filter_map(|recent| {
                FileIdentity::from_canonical_path(recent.path)
                    .ok()
                    .map(|identity| (identity, recent.title))
            }));
        if let Some(session_workspace) = state.workspace {
            match self
                .workspace_service
                .scan(&session_workspace.root, self.workspace_options)
            {
                Ok(snapshot) => {
                    let mut workspace = WorkspaceModel::new(snapshot);
                    let _ = workspace.restore_tree_state(
                        &session_workspace.expanded_paths,
                        session_workspace.selected_path.as_deref(),
                    );
                    self.workspace = Some(workspace);
                    self.sidebar_is_visible = true;
                    self.lifecycle_notice = Some(format!(
                        "Restored workspace {}",
                        session_workspace.root.display()
                    ));
                }
                Err(error) => {
                    self.lifecycle_notice =
                        Some(format!("Saved workspace could not be restored: {error}"));
                }
            }
        }
        if let Some(workspace) = &self.workspace {
            self.selected_sidebar_id = workspace
                .selected()
                .or_else(|| Some(workspace.snapshot().root_id()))
                .map(|id| id.stable_key().to_owned());
        } else {
            self.select_active_in_sidebar();
        }
    }

    fn session_state(&self) -> SessionState {
        let recent_files = self
            .recent_files
            .entries()
            .iter()
            .map(|entry| SessionRecentFile {
                path: entry.identity().path().to_path_buf(),
                title: entry.title().to_owned(),
            })
            .collect();
        let workspace = self.workspace.as_ref().map(|workspace| {
            let selected_path = workspace
                .selected()
                .and_then(|id| workspace.snapshot().node(id))
                .map(|node| node.path().to_path_buf());
            SessionWorkspace {
                root: workspace.snapshot().root().to_path_buf(),
                expanded_paths: workspace.expanded_paths(),
                selected_path,
            }
        });
        SessionState {
            recent_files,
            workspace,
        }
    }

    fn persist_session(&mut self) {
        let state = self.session_state();
        if let Err(error) = self.session_store.save(&state) {
            self.lifecycle_notice = Some(format!("Session save failed: {error}"));
        }
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
        let active_record = self.document_registry.get(active.id);
        let save_is_enabled = active_record
            .map(|record| record.save_requirement(active.editor.edit_revision()))
            .is_some_and(|requirement| requirement != SaveRequirement::None);
        let reload_is_enabled = active_record.is_some_and(|record| {
            matches!(
                record.external_state(),
                ExternalState::Modified { .. }
                    | ExternalState::Replaced { .. }
                    | ExternalState::Recreated { .. }
            )
        });
        let find_navigation_is_enabled = self.find.is_some() && !self.find_matches.is_empty();
        let selected_workspace = self.selected_workspace_entry();
        let workspace_mutation_is_enabled = self.workspace.is_some();
        let workspace_entry_mutation_is_enabled = selected_workspace
            .as_ref()
            .is_some_and(|(_, _, is_root)| !is_root);
        let mut file_items = vec![
            MenuItem::command(MenuCommand::new("new-file", "New File", "Ctrl+N")),
            MenuItem::command(MenuCommand::new("open", "Open…", "Ctrl+O")),
            MenuItem::command(MenuCommand::new(
                "open-folder",
                "Open Folder…",
                "Ctrl+Shift+O",
            )),
        ];
        if self.workspace.is_some() {
            file_items.extend([
                MenuItem::command(MenuCommand::new(
                    "refresh-workspace",
                    "Refresh Workspace",
                    "Ctrl+Shift+R",
                )),
                MenuItem::command(MenuCommand::new("close-workspace", "Close Workspace", "")),
                MenuItem::Separator,
                MenuItem::command(
                    MenuCommand::new("new-workspace-file", "New File in Workspace…", "")
                        .with_enabled(workspace_mutation_is_enabled),
                ),
                MenuItem::command(
                    MenuCommand::new("new-workspace-folder", "New Folder…", "")
                        .with_enabled(workspace_mutation_is_enabled),
                ),
                MenuItem::command(
                    MenuCommand::new("rename-workspace-entry", "Rename Workspace Entry…", "")
                        .with_enabled(workspace_entry_mutation_is_enabled),
                ),
                MenuItem::command(
                    MenuCommand::new("delete-workspace-entry", "Delete Workspace Entry…", "")
                        .with_enabled(workspace_entry_mutation_is_enabled),
                ),
            ]);
        }
        if !self.recent_files.entries().is_empty() {
            file_items.push(MenuItem::Separator);
            file_items.extend(self.recent_files.entries().iter().enumerate().map(
                |(index, entry)| {
                    MenuItem::command(MenuCommand::new(
                        format!("open-recent-{index}"),
                        format!("Open Recent: {}", entry.title()),
                        "",
                    ))
                },
            ));
            file_items.push(MenuItem::command(MenuCommand::new(
                "clear-recent-files",
                "Clear Recent Files",
                "",
            )));
        }
        file_items.extend([
            MenuItem::Separator,
            MenuItem::command(
                MenuCommand::new("save", "Save", "Ctrl+S").with_enabled(save_is_enabled),
            ),
            MenuItem::command(MenuCommand::new("save-as", "Save As…", "Ctrl+Shift+S")),
            MenuItem::command(
                MenuCommand::new("reload-from-disk", "Reload from Disk", "")
                    .with_enabled(reload_is_enabled),
            ),
            MenuItem::Separator,
            MenuItem::command(
                MenuCommand::new("close-tab", "Close Tab", "Ctrl+W")
                    .with_enabled(self.documents.len() > 1),
            ),
            MenuItem::Separator,
            MenuItem::command(MenuCommand::new("exit", "Exit", "")),
        ]);

        vec![
            MenuDefinition::new("file", "File", file_items),
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

    fn sidebar_items(&self) -> Vec<SidebarItem> {
        if let Some(workspace) = &self.workspace {
            return workspace
                .visible_rows()
                .into_iter()
                .map(|row| {
                    let title = match &row.status {
                        WorkspaceNodeStatus::Available => match row.kind {
                            WorkspaceNodeKind::Symlink => format!("{} ↗", row.title),
                            WorkspaceNodeKind::Directory | WorkspaceNodeKind::File => row.title,
                        },
                        WorkspaceNodeStatus::PermissionDenied => {
                            format!("{} (permission denied)", row.title)
                        }
                        WorkspaceNodeStatus::DepthLimit => {
                            format!("{} (depth limit)", row.title)
                        }
                        WorkspaceNodeStatus::Unreadable(_) => {
                            format!("{} (unreadable)", row.title)
                        }
                    };
                    match row.kind {
                        WorkspaceNodeKind::Directory => SidebarItem::folder(
                            row.id.stable_key(),
                            title,
                            row.depth,
                            row.is_expanded,
                        ),
                        WorkspaceNodeKind::File | WorkspaceNodeKind::Symlink => {
                            SidebarItem::file(row.id.stable_key(), title, row.depth)
                        }
                    }
                })
                .collect();
        }

        let mut items = vec![SidebarItem::folder(
            "open-documents",
            "Open Documents",
            0,
            true,
        )];
        items.extend(self.documents.iter().map(|document| {
            SidebarItem::file(
                document.stable_key(),
                document.title(&self.document_registry),
                1,
            )
        }));
        items
    }

    fn shell_state(&self) -> EditorShellState {
        let active = self.active_document();
        let active_title = active.title(&self.document_registry);
        let menu_definitions = self.menu_definitions();
        let sidebar_items = self.sidebar_items();
        let status_left = self.active_external_notice().map_or_else(
            || {
                self.lifecycle_notice.as_ref().map_or_else(
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
        if let Some(index) = command
            .strip_prefix("open-recent-")
            .and_then(|value| value.parse::<usize>().ok())
        {
            self.open_recent_file(index);
            return HostControl::Invalidate(InvalidationClass::WidgetLayout);
        }
        let invalidation = match command {
            "new-file" => {
                self.new_document();
                InvalidationClass::WidgetLayout
            }
            "open" => {
                self.open_file();
                InvalidationClass::WidgetLayout
            }
            "open-folder" => {
                self.open_workspace();
                InvalidationClass::WidgetLayout
            }
            "refresh-workspace" => {
                let _ = self.refresh_workspace(true);
                InvalidationClass::WidgetLayout
            }
            "close-workspace" => {
                self.close_workspace();
                InvalidationClass::WidgetLayout
            }
            "new-workspace-file" => {
                self.create_workspace_file();
                InvalidationClass::WidgetLayout
            }
            "new-workspace-folder" => {
                self.create_workspace_folder();
                InvalidationClass::WidgetLayout
            }
            "rename-workspace-entry" => {
                self.rename_workspace_entry();
                InvalidationClass::WidgetLayout
            }
            "delete-workspace-entry" => {
                self.delete_workspace_entry();
                InvalidationClass::WidgetLayout
            }
            "save" => {
                let _ = self.save_active();
                InvalidationClass::WidgetLayout
            }
            "save-as" => {
                let _ = self.save_active_as();
                InvalidationClass::WidgetLayout
            }
            "reload-from-disk" => {
                let _ = self.reload_active_file();
                InvalidationClass::TextLayout
            }
            "clear-recent-files" => {
                self.recent_files.clear();
                self.lifecycle_notice = Some("Recent files cleared".to_owned());
                self.persist_session();
                InvalidationClass::WidgetLayout
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
        self.select_active_in_sidebar();
        self.lifecycle_notice = Some(format!(
            "Created {}; Save will open a destination dialog",
            self.active_document().title(&self.document_registry)
        ));
        self.reveal_caret_on_next_frame = true;
    }

    fn workspace_selection_for_document(&self, id: DocumentId) -> Option<String> {
        let workspace = self.workspace.as_ref()?;
        let record = self.document_registry.get(id)?;
        let DocumentSource::File(identity) = record.source() else {
            return None;
        };
        workspace
            .snapshot()
            .node_for_path(identity.path())
            .map(|node| node.id().stable_key().to_owned())
    }

    fn select_active_in_sidebar(&mut self) {
        let active_id = self.active_document().id;
        self.selected_sidebar_id = self
            .workspace_selection_for_document(active_id)
            .or_else(|| self.workspace.is_none().then(|| active_id.stable_key()));
        if let Some(workspace) = self.workspace.as_mut() {
            let selected = self
                .selected_sidebar_id
                .as_deref()
                .and_then(|key| workspace.snapshot().node_for_stable_key(key))
                .map(|node| node.id().clone());
            if let Some(selected) = selected {
                let _ = workspace.reveal(&selected);
            } else {
                let _ = workspace.select(None);
            }
        }
    }

    fn selected_workspace_entry(&self) -> Option<(PathBuf, WorkspaceNodeKind, bool)> {
        let workspace = self.workspace.as_ref()?;
        let id = workspace
            .selected()
            .unwrap_or_else(|| workspace.snapshot().root_id());
        let node = workspace.snapshot().node(id)?;
        Some((
            node.path().to_path_buf(),
            node.kind(),
            node.id() == workspace.snapshot().root_id(),
        ))
    }

    fn workspace_creation_parent(&self) -> Option<PathBuf> {
        let (path, kind, _) = self.selected_workspace_entry()?;
        match kind {
            WorkspaceNodeKind::Directory => Some(path),
            WorkspaceNodeKind::File | WorkspaceNodeKind::Symlink => {
                path.parent().map(Path::to_path_buf)
            }
        }
    }

    fn select_workspace_path(&mut self, path: &Path) {
        let selected = self.workspace.as_ref().and_then(|workspace| {
            workspace
                .snapshot()
                .node_for_path(path)
                .map(|node| node.id().clone())
        });
        if let Some(selected) = selected
            && let Some(workspace) = self.workspace.as_mut()
        {
            let _ = workspace.reveal(&selected);
            self.selected_sidebar_id = Some(selected.stable_key().to_owned());
        }
    }

    fn create_workspace_file(&mut self) {
        let Some(parent) = self.workspace_creation_parent() else {
            self.lifecycle_notice = Some("No workspace is open".to_owned());
            return;
        };
        let name = match self.dialog_service.prompt_workspace_name(
            "New Workspace File",
            "Enter a file name:",
            "untitled.txt",
        ) {
            Ok(Some(name)) => name,
            Ok(None) => {
                self.lifecycle_notice = Some("New workspace file canceled".to_owned());
                return;
            }
            Err(error) => {
                self.lifecycle_notice = Some(format!("New file dialog unavailable: {error}"));
                return;
            }
        };
        let mut result = self.workspace_service.create_file(
            &parent,
            &name,
            WorkspaceCollisionPolicy::FailIfExists,
        );
        if result
            .as_ref()
            .is_err_and(|error| error.kind() == WorkspaceErrorKind::AlreadyExists)
        {
            let destination = parent.join(&name);
            if let Ok(identity) = FileIdentity::from_canonical_path(destination.clone())
                && let Some(existing) = self.document_registry.document_for_file(&identity)
            {
                self.activate_document_id(existing);
                self.lifecycle_notice = Some(
                    "The existing workspace file is already open; it was not replaced".to_owned(),
                );
                return;
            }
            match self.dialog_service.confirm_workspace_replace(&destination) {
                Ok(WorkspaceCollisionChoice::Replace) => {
                    result = self.workspace_service.create_file(
                        &parent,
                        &name,
                        WorkspaceCollisionPolicy::ReplaceFile,
                    );
                }
                Ok(WorkspaceCollisionChoice::Cancel) => {
                    self.lifecycle_notice = Some("New workspace file canceled".to_owned());
                    return;
                }
                Err(error) => {
                    self.lifecycle_notice =
                        Some(format!("Replacement confirmation unavailable: {error}"));
                    return;
                }
            }
        }
        match result {
            Ok(outcome) => {
                let path = outcome.path().to_path_buf();
                let _ = self.refresh_workspace(false);
                self.select_workspace_path(&path);
                self.lifecycle_notice = Some(format!("Created {}", path.display()));
                self.persist_session();
            }
            Err(error) => {
                self.lifecycle_notice = Some(format!("Create file failed: {error}"));
            }
        }
    }

    fn create_workspace_folder(&mut self) {
        let Some(parent) = self.workspace_creation_parent() else {
            self.lifecycle_notice = Some("No workspace is open".to_owned());
            return;
        };
        let name = match self.dialog_service.prompt_workspace_name(
            "New Workspace Folder",
            "Enter a folder name:",
            "new-folder",
        ) {
            Ok(Some(name)) => name,
            Ok(None) => {
                self.lifecycle_notice = Some("New workspace folder canceled".to_owned());
                return;
            }
            Err(error) => {
                self.lifecycle_notice = Some(format!("New folder dialog unavailable: {error}"));
                return;
            }
        };
        match self.workspace_service.create_directory(&parent, &name) {
            Ok(outcome) => {
                let path = outcome.path().to_path_buf();
                let _ = self.refresh_workspace(false);
                self.select_workspace_path(&path);
                self.lifecycle_notice = Some(format!("Created {}", path.display()));
                self.persist_session();
            }
            Err(error) => {
                self.lifecycle_notice = Some(format!("Create folder failed: {error}"));
            }
        }
    }

    fn affected_file_documents(&self, path: &Path) -> Vec<(DocumentId, FileIdentity)> {
        self.document_registry
            .records()
            .iter()
            .filter_map(|record| match record.source() {
                DocumentSource::File(identity) if identity.path().starts_with(path) => {
                    Some((record.id(), identity.clone()))
                }
                DocumentSource::File(_)
                | DocumentSource::Untitled { .. }
                | DocumentSource::Virtual { .. } => None,
            })
            .collect()
    }

    fn rename_workspace_entry(&mut self) {
        let Some((source, _kind, is_root)) = self.selected_workspace_entry() else {
            self.lifecycle_notice = Some("No workspace entry is selected".to_owned());
            return;
        };
        if is_root {
            self.lifecycle_notice = Some("The workspace root cannot be renamed".to_owned());
            return;
        }
        let initial_name = source
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let new_name = match self.dialog_service.prompt_workspace_name(
            "Rename Workspace Entry",
            "Enter the new name:",
            &initial_name,
        ) {
            Ok(Some(name)) => name,
            Ok(None) => {
                self.lifecycle_notice = Some("Rename canceled".to_owned());
                return;
            }
            Err(error) => {
                self.lifecycle_notice = Some(format!("Rename dialog unavailable: {error}"));
                return;
            }
        };
        let Some(parent) = source.parent() else {
            self.lifecycle_notice = Some("Rename source has no parent directory".to_owned());
            return;
        };
        let destination = parent.join(&new_name);
        let affected = self.affected_file_documents(&source);
        if let Ok(destination_identity) = FileIdentity::from_canonical_path(destination.clone())
            && let Some(existing) = self
                .document_registry
                .document_for_file(&destination_identity)
            && affected.iter().all(|(id, _)| *id != existing)
        {
            self.activate_document_id(existing);
            self.lifecycle_notice =
                Some("Rename destination is already open as another document".to_owned());
            return;
        }
        for (_, identity) in &affected {
            let relative = identity
                .path()
                .strip_prefix(&source)
                .unwrap_or_else(|_| Path::new(""));
            let candidate = destination.join(relative);
            if let Ok(candidate_identity) = FileIdentity::from_canonical_path(candidate)
                && let Some(existing) = self
                    .document_registry
                    .document_for_file(&candidate_identity)
                && affected.iter().all(|(id, _)| *id != existing)
            {
                self.activate_document_id(existing);
                self.lifecycle_notice =
                    Some("Rename destination is already open as another document".to_owned());
                return;
            }
        }
        let mut result = self.workspace_service.rename(
            &source,
            &new_name,
            WorkspaceCollisionPolicy::FailIfExists,
        );
        if result
            .as_ref()
            .is_err_and(|error| error.kind() == WorkspaceErrorKind::AlreadyExists)
        {
            match self.dialog_service.confirm_workspace_replace(&destination) {
                Ok(WorkspaceCollisionChoice::Replace) => {
                    result = self.workspace_service.rename(
                        &source,
                        &new_name,
                        WorkspaceCollisionPolicy::ReplaceFile,
                    );
                }
                Ok(WorkspaceCollisionChoice::Cancel) => {
                    self.lifecycle_notice = Some("Rename canceled".to_owned());
                    return;
                }
                Err(error) => {
                    self.lifecycle_notice =
                        Some(format!("Replacement confirmation unavailable: {error}"));
                    return;
                }
            }
        }
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => {
                self.lifecycle_notice = Some(format!("Rename failed: {error}"));
                return;
            }
        };
        let renamed_root = outcome.path().to_path_buf();
        let recent_relocations = self
            .recent_files
            .entries()
            .iter()
            .filter_map(|entry| {
                let relative = entry.identity().path().strip_prefix(&source).ok()?;
                let replacement =
                    FileIdentity::from_canonical_path(renamed_root.join(relative)).ok()?;
                Some((
                    entry.identity().clone(),
                    replacement.clone(),
                    file_title(&replacement),
                ))
            })
            .collect::<Vec<_>>();
        for (previous, replacement, title) in recent_relocations {
            self.recent_files.relocate(&previous, replacement, title);
        }
        let mut relocation_error = None;
        for (document_id, previous_identity) in affected {
            let relative = previous_identity
                .path()
                .strip_prefix(&source)
                .unwrap_or_else(|_| Path::new(""));
            let new_path = renamed_root.join(relative);
            match self.file_service.load_utf8(&new_path) {
                Ok(loaded) => {
                    let title = file_title(loaded.identity());
                    if let Err(error) = self.document_registry.relocate_file(
                        document_id,
                        loaded.identity().clone(),
                        title.clone(),
                        Some(loaded.snapshot()),
                    ) {
                        relocation_error = Some(format!(
                            "Renamed storage but document relocation failed: {error}"
                        ));
                        continue;
                    }
                }
                Err(error) => {
                    relocation_error = Some(format!(
                        "Renamed storage but could not observe {}: {error}",
                        new_path.display()
                    ));
                }
            }
        }
        let _ = self.refresh_workspace(false);
        self.select_workspace_path(&renamed_root);
        if relocation_error.is_none() {
            relocation_error = Some(format!(
                "Renamed {} to {}",
                source.display(),
                renamed_root.display()
            ));
        }
        self.lifecycle_notice = relocation_error;
        self.persist_session();
    }

    fn delete_workspace_entry(&mut self) {
        let Some((path, kind, is_root)) = self.selected_workspace_entry() else {
            self.lifecycle_notice = Some("No workspace entry is selected".to_owned());
            return;
        };
        if is_root {
            self.lifecycle_notice = Some("The workspace root cannot be deleted".to_owned());
            return;
        }
        match self
            .dialog_service
            .confirm_workspace_delete(&path, kind == WorkspaceNodeKind::Directory)
        {
            Ok(WorkspaceDeleteChoice::Delete) => {}
            Ok(WorkspaceDeleteChoice::Cancel) => {
                self.lifecycle_notice = Some("Delete canceled".to_owned());
                return;
            }
            Err(error) => {
                self.lifecycle_notice = Some(format!("Delete confirmation unavailable: {error}"));
                return;
            }
        }
        let affected = self.affected_file_documents(&path);
        let deleted_recent_identities = self
            .recent_files
            .entries()
            .iter()
            .filter(|entry| entry.identity().path().starts_with(&path))
            .map(|entry| entry.identity().clone())
            .collect::<Vec<_>>();
        let mut detach = Vec::new();
        let mut close = Vec::new();
        for (document_id, identity) in &affected {
            let Some(index) = self
                .documents
                .iter()
                .position(|document| document.id == *document_id)
            else {
                continue;
            };
            if self.documents[index].is_dirty(&self.document_registry) {
                let title = self.documents[index]
                    .title(&self.document_registry)
                    .to_owned();
                let choice = self
                    .dialog_service
                    .resolve_dirty_workspace_delete(&title, identity.path());
                match choice {
                    Ok(WorkspaceDirtyDeleteChoice::KeepOpen) => detach.push(*document_id),
                    Ok(WorkspaceDirtyDeleteChoice::DiscardAndClose) => close.push(*document_id),
                    Ok(WorkspaceDirtyDeleteChoice::Cancel) => {
                        self.lifecycle_notice = Some("Delete canceled".to_owned());
                        return;
                    }
                    Err(error) => {
                        self.lifecycle_notice =
                            Some(format!("Dirty-document resolution unavailable: {error}"));
                        return;
                    }
                }
            } else {
                close.push(*document_id);
            }
        }
        if let Err(error) = self.workspace_service.delete(&path) {
            self.lifecycle_notice = Some(format!("Delete failed: {error}"));
            return;
        }
        for identity in deleted_recent_identities {
            self.recent_files.remove(&identity);
        }
        let mut detach_error = None;
        for document_id in &detach {
            if let Err(error) = self.document_registry.detach_file_to_untitled(*document_id) {
                detach_error = Some(format!(
                    "Deleted storage but could not detach an open document: {error}"
                ));
            }
        }
        for document_id in close {
            self.remove_document_id(document_id);
        }
        if self.documents.is_empty() {
            self.new_document();
        }
        let _ = self.refresh_workspace(false);
        self.select_active_in_sidebar();
        if detach_error.is_none() {
            detach_error = Some(format!("Deleted {}", path.display()));
        }
        self.lifecycle_notice = detach_error;
        self.persist_session();
    }

    fn remove_document_id(&mut self, id: DocumentId) {
        let Some(index) = self.documents.iter().position(|document| document.id == id) else {
            return;
        };
        let removed_key = id.stable_key();
        self.documents.remove(index);
        let _ = self.document_registry.remove(id);
        let _ = self.text_layouts.remove(&removed_key);
        if index < self.active_index {
            self.active_index = self.active_index.saturating_sub(1);
        }
        self.active_index = self
            .active_index
            .min(self.documents.len().saturating_sub(1));
    }

    fn open_workspace(&mut self) {
        let selected = match self.dialog_service.choose_open_folder() {
            Ok(Some(path)) => path,
            Ok(None) => {
                self.lifecycle_notice = Some("Open Folder canceled".to_owned());
                return;
            }
            Err(error) => {
                self.lifecycle_notice = Some(format!("Open Folder dialog unavailable: {error}"));
                return;
            }
        };
        match self
            .workspace_service
            .scan(&selected, self.workspace_options)
        {
            Ok(snapshot) => {
                let root_title = snapshot.root().file_name().map_or_else(
                    || snapshot.root().display().to_string(),
                    |name| name.to_string_lossy().into_owned(),
                );
                let root_id = snapshot.root_id().clone();
                let root_key = root_id.stable_key().to_owned();
                let mut workspace = WorkspaceModel::new(snapshot);
                let _ = workspace.select(Some(root_id));
                self.workspace = Some(workspace);
                self.sidebar_is_visible = true;
                self.selected_sidebar_id = Some(root_key);
                self.lifecycle_notice = Some(format!("Opened workspace {root_title}"));
                self.workspace_refresh_elapsed = Duration::ZERO;
                self.persist_session();
            }
            Err(error) => {
                self.lifecycle_notice = Some(format!("Open Folder failed: {error}"));
            }
        }
    }

    fn close_workspace(&mut self) {
        if self.workspace.take().is_some() {
            self.select_active_in_sidebar();
            self.lifecycle_notice = Some("Workspace closed".to_owned());
            self.persist_session();
        }
    }

    fn refresh_workspace(&mut self, announce_unchanged: bool) -> bool {
        let Some(root) = self
            .workspace
            .as_ref()
            .map(|workspace| workspace.snapshot().root().to_path_buf())
        else {
            if announce_unchanged {
                self.lifecycle_notice = Some("No workspace is open".to_owned());
            }
            return false;
        };
        let snapshot = match self.workspace_service.scan(&root, self.workspace_options) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let notice = format!("Workspace refresh failed: {error}");
                let changed = self.lifecycle_notice.as_deref() != Some(notice.as_str());
                self.lifecycle_notice = Some(notice);
                return changed;
            }
        };
        let changed = self
            .workspace
            .as_mut()
            .is_some_and(|workspace| workspace.refresh(snapshot));
        if changed {
            self.selected_sidebar_id = self
                .workspace
                .as_ref()
                .and_then(|workspace| workspace.selected())
                .map(|id| id.stable_key().to_owned());
            if self.selected_sidebar_id.is_none() {
                self.select_active_in_sidebar();
            }
            if announce_unchanged {
                self.lifecycle_notice = Some("Workspace refreshed".to_owned());
            }
            self.persist_session();
        } else if announce_unchanged {
            self.lifecycle_notice = Some("Workspace is already up to date".to_owned());
        }
        changed || announce_unchanged
    }

    fn focus_sidebar_item(&mut self, id: &str) {
        let workspace_node = self.workspace.as_ref().and_then(|workspace| {
            workspace
                .snapshot()
                .node_for_stable_key(id)
                .map(|node| node.id().clone())
        });
        if let Some(node_id) = workspace_node {
            if let Some(workspace) = self.workspace.as_mut() {
                let _ = workspace.select(Some(node_id.clone()));
            }
            self.selected_sidebar_id = Some(node_id.stable_key().to_owned());
            self.persist_session();
        } else {
            self.activate_document(id);
        }
    }

    fn handle_sidebar_activation(&mut self, id: &str) {
        let workspace_target = self.workspace.as_ref().and_then(|workspace| {
            workspace.snapshot().node_for_stable_key(id).map(|node| {
                (
                    node.id().clone(),
                    node.kind(),
                    node.path().to_path_buf(),
                    node.status().clone(),
                )
            })
        });
        let Some((node_id, kind, path, status)) = workspace_target else {
            self.activate_document(id);
            return;
        };
        if let Some(workspace) = self.workspace.as_mut() {
            let _ = workspace.select(Some(node_id.clone()));
        }
        self.selected_sidebar_id = Some(node_id.stable_key().to_owned());
        match (kind, status) {
            (WorkspaceNodeKind::Directory, WorkspaceNodeStatus::Available) => {
                if let Some(workspace) = self.workspace.as_mut() {
                    let _ = workspace.toggle_expanded(&node_id);
                }
                self.lifecycle_notice = None;
                self.persist_session();
            }
            (WorkspaceNodeKind::File, WorkspaceNodeStatus::Available) => self.open_path(&path),
            (WorkspaceNodeKind::Symlink, _) => {
                self.lifecycle_notice = Some(format!(
                    "Symbolic links are shown but not followed: {}",
                    path.display()
                ));
            }
            (_, WorkspaceNodeStatus::PermissionDenied) => {
                self.lifecycle_notice = Some(format!("Permission denied: {}", path.display()));
            }
            (_, WorkspaceNodeStatus::DepthLimit) => {
                self.lifecycle_notice = Some(format!(
                    "Workspace depth limit reached at {}",
                    path.display()
                ));
            }
            (_, WorkspaceNodeStatus::Unreadable(message)) => {
                self.lifecycle_notice = Some(format!("Cannot read {}: {message}", path.display()));
            }
        }
    }

    fn open_file(&mut self) {
        let selected = match self.dialog_service.choose_open_file() {
            Ok(Some(path)) => path,
            Ok(None) => {
                self.lifecycle_notice = Some("Open canceled".to_owned());
                return;
            }
            Err(error) => {
                self.lifecycle_notice = Some(format!("Open dialog unavailable: {error}"));
                return;
            }
        };
        self.open_path(&selected);
    }

    fn open_recent_file(&mut self, index: usize) {
        let Some(identity) = self
            .recent_files
            .entries()
            .get(index)
            .map(|entry| entry.identity().clone())
        else {
            self.lifecycle_notice = Some("Recent file entry is no longer available".to_owned());
            return;
        };
        self.open_path(identity.path());
    }

    fn open_path(&mut self, path: &Path) {
        let loaded = match self.file_service.load_utf8(path) {
            Ok(loaded) => loaded,
            Err(error) => {
                if error.kind() == FileServiceErrorKind::NotFound
                    && let Ok(identity) = self.file_service.identity_for_save(path)
                {
                    self.recent_files.remove(&identity);
                    self.persist_session();
                }
                self.lifecycle_notice = Some(format!("Open failed: {error}"));
                return;
            }
        };
        let identity = loaded.identity().clone();
        let title = file_title(&identity);
        let storage_snapshot = loaded.snapshot();
        let editor = EditableText::new(loaded.into_text());
        self.recent_files.record(identity.clone(), title.clone());
        self.persist_session();
        match self.document_registry.register_file(
            identity,
            title.clone(),
            editor.edit_revision(),
            Some(storage_snapshot),
        ) {
            OpenFileOutcome::Opened(id) => {
                self.documents.push(DemoDocument {
                    id,
                    editor,
                    scroll: TextScroll::default(),
                });
                self.active_index = self.documents.len().saturating_sub(1);
                self.select_active_in_sidebar();
                self.lifecycle_notice = Some(format!("Opened {title}"));
                self.reveal_caret_on_next_frame = true;
            }
            OpenFileOutcome::AlreadyOpen(id) => {
                self.activate_document_id(id);
                self.lifecycle_notice = Some(format!("{title} is already open"));
            }
        }
    }

    fn save_active(&mut self) -> bool {
        let (document_id, edit_revision) = {
            let document = self.active_document();
            (document.id, document.editor.edit_revision())
        };
        let requirement = self
            .document_registry
            .get(document_id)
            .map(|record| record.save_requirement(edit_revision));
        match requirement {
            Some(SaveRequirement::None) => {
                self.lifecycle_notice = Some("Document is already saved".to_owned());
                true
            }
            Some(SaveRequirement::SaveAs | SaveRequirement::Unsupported) => self.save_active_as(),
            Some(SaveRequirement::WriteFile {
                identity,
                expected_storage_snapshot,
                external_state,
            }) => {
                if external_state != ExternalState::InSync {
                    return self.resolve_save_conflict(identity);
                }
                let precondition = expected_storage_snapshot
                    .map_or(WritePrecondition::Missing, WritePrecondition::Matches);
                match self.write_active_to_path(identity.path(), precondition) {
                    Ok(()) => true,
                    Err(error) if error.kind() == FileServiceErrorKind::Conflict => {
                        self.resolve_save_conflict(identity)
                    }
                    Err(error) => {
                        self.lifecycle_notice = Some(format!("Save failed: {error}"));
                        false
                    }
                }
            }
            None => {
                self.lifecycle_notice = Some("Document lifecycle record is unavailable".to_owned());
                false
            }
        }
    }

    fn save_active_as(&mut self) -> bool {
        let (document_id, suggested_name, current_path, text, edit_revision) = {
            let document = self.active_document();
            let record = self.document_registry.get(document.id);
            let current_path = record.and_then(|record| match record.source() {
                DocumentSource::File(identity) => Some(identity.path().to_path_buf()),
                DocumentSource::Untitled { .. } | DocumentSource::Virtual { .. } => None,
            });
            (
                document.id,
                document.title(&self.document_registry).to_owned(),
                current_path,
                document.editor.document().text().to_owned(),
                document.editor.edit_revision(),
            )
        };
        let selected = match self
            .dialog_service
            .choose_save_file(&suggested_name, current_path.as_deref())
        {
            Ok(Some(path)) => path,
            Ok(None) => {
                self.lifecycle_notice = Some("Save As canceled".to_owned());
                return false;
            }
            Err(error) => {
                self.lifecycle_notice = Some(format!("Save As dialog unavailable: {error}"));
                return false;
            }
        };
        let identity = match self.file_service.identity_for_save(&selected) {
            Ok(identity) => identity,
            Err(error) => {
                self.lifecycle_notice = Some(format!("Save As failed: {error}"));
                return false;
            }
        };
        if let Some(existing) = self.document_registry.document_for_file(&identity)
            && existing != document_id
        {
            self.activate_document_id(existing);
            self.lifecycle_notice = Some(format!(
                "{} is already open; no file was overwritten",
                file_title(&identity)
            ));
            return false;
        }
        let written =
            match self
                .file_service
                .write_utf8_atomic(&selected, &text, WritePrecondition::Any)
            {
                Ok(written) => written,
                Err(error) => {
                    self.lifecycle_notice = Some(format!("Save As failed: {error}"));
                    return false;
                }
            };
        let title = file_title(written.identity());
        if let Err(error) = self.document_registry.assign_file(
            document_id,
            written.identity().clone(),
            title.clone(),
            edit_revision,
            Some(written.snapshot()),
        ) {
            self.lifecycle_notice = Some(format!("Save As registration failed: {error}"));
            return false;
        }
        self.recent_files
            .record(written.identity().clone(), title.clone());
        self.persist_session();
        let _ = self.refresh_workspace(false);
        self.select_active_in_sidebar();
        self.lifecycle_notice = Some(format!("Saved {title}"));
        true
    }

    fn write_active_to_path(
        &mut self,
        path: &Path,
        precondition: WritePrecondition,
    ) -> Result<(), FileServiceError> {
        let (document_id, text, edit_revision) = {
            let document = self.active_document();
            (
                document.id,
                document.editor.document().text().to_owned(),
                document.editor.edit_revision(),
            )
        };
        let written = self
            .file_service
            .write_utf8_atomic(path, &text, precondition)?;
        if let Some(record) = self.document_registry.get_mut(document_id) {
            record.mark_saved(edit_revision, Some(written.snapshot()));
        }
        let title = file_title(written.identity());
        self.recent_files
            .record(written.identity().clone(), title.clone());
        self.persist_session();
        let _ = self.refresh_workspace(false);
        self.select_active_in_sidebar();
        self.lifecycle_notice = Some(format!("Saved {title}"));
        Ok(())
    }

    fn resolve_save_conflict(&mut self, identity: FileIdentity) -> bool {
        let title = self
            .active_document()
            .title(&self.document_registry)
            .to_owned();
        let choice = match self
            .dialog_service
            .resolve_save_conflict(&title, identity.path())
        {
            Ok(choice) => choice,
            Err(error) => {
                self.lifecycle_notice = Some(format!("Conflict dialog unavailable: {error}"));
                return false;
            }
        };
        match choice {
            SaveConflictChoice::Overwrite => {
                match self.write_active_to_path(identity.path(), WritePrecondition::Any) {
                    Ok(()) => true,
                    Err(error) => {
                        self.lifecycle_notice = Some(format!("Overwrite failed: {error}"));
                        false
                    }
                }
            }
            SaveConflictChoice::Reload => self.reload_active_from_path(identity.path()),
            SaveConflictChoice::Cancel => {
                self.lifecycle_notice = Some("Save canceled after conflict".to_owned());
                false
            }
        }
    }

    fn reload_active_file(&mut self) -> bool {
        let path = self
            .document_registry
            .get(self.active_document().id)
            .and_then(|record| match record.source() {
                DocumentSource::File(identity) => Some(identity.path().to_path_buf()),
                DocumentSource::Untitled { .. } | DocumentSource::Virtual { .. } => None,
            });
        path.is_some_and(|path| self.reload_active_from_path(&path))
    }

    fn active_external_notice(&self) -> Option<String> {
        let record = self.document_registry.get(self.active_document().id)?;
        let action = "Use File → Reload from Disk or save to resolve";
        match record.external_state() {
            ExternalState::InSync => None,
            ExternalState::Modified { .. } => Some(format!("Changed on disk. {action}")),
            ExternalState::Replaced { .. } => Some(format!("Replaced on disk. {action}")),
            ExternalState::Missing => Some(
                "Deleted on disk. Save can recreate it or Save As can choose another path"
                    .to_owned(),
            ),
            ExternalState::Recreated { .. } => Some(format!("Recreated on disk. {action}")),
        }
    }

    fn poll_external_changes(&mut self) -> bool {
        let active_id = self.active_document().id;
        let watched = self
            .document_registry
            .records()
            .iter()
            .filter_map(|record| match record.source() {
                DocumentSource::File(identity) => Some((record.id(), identity.clone())),
                DocumentSource::Untitled { .. } | DocumentSource::Virtual { .. } => None,
            })
            .collect::<Vec<_>>();
        let mut state_changed = false;
        let mut active_observation_error = None;
        for (document_id, identity) in watched {
            let previous = self
                .document_registry
                .get(document_id)
                .map_or(ExternalState::InSync, DocumentRecord::external_state);
            let observation = match self.file_service.observe_file(identity.path()) {
                Ok(observation) => observation,
                Err(error) => {
                    if document_id == active_id {
                        active_observation_error =
                            Some(format!("File observation failed: {error}"));
                    }
                    continue;
                }
            };
            if let Some(record) = self.document_registry.get_mut(document_id) {
                match observation {
                    FileObservation::Present(snapshot) => {
                        record.observe_storage_snapshot(snapshot);
                    }
                    FileObservation::Missing => record.observe_missing_file(),
                }
                state_changed |= record.external_state() != previous;
            }
        }

        let notice_changed = match active_observation_error {
            Some(notice) if self.lifecycle_notice.as_deref() != Some(notice.as_str()) => {
                self.lifecycle_notice = Some(notice);
                true
            }
            Some(_) => false,
            None if self
                .lifecycle_notice
                .as_deref()
                .is_some_and(|notice| notice.starts_with("File observation failed:")) =>
            {
                self.lifecycle_notice = None;
                true
            }
            None => {
                if state_changed {
                    self.lifecycle_notice = None;
                }
                false
            }
        };
        state_changed || notice_changed
    }

    fn reload_active_from_path(&mut self, path: &Path) -> bool {
        let loaded = match self.file_service.load_utf8(path) {
            Ok(loaded) => loaded,
            Err(error) => {
                self.lifecycle_notice = Some(format!("Reload failed: {error}"));
                return false;
            }
        };
        let document_id = self.active_document().id;
        let stable_key = document_id.stable_key();
        let storage_snapshot = loaded.snapshot();
        let editor = EditableText::new(loaded.into_text());
        let edit_revision = editor.edit_revision();
        {
            let document = self.active_document_mut();
            document.editor = editor;
            document.scroll = TextScroll::default();
        }
        if let Some(record) = self.document_registry.get_mut(document_id) {
            record.mark_saved(edit_revision, Some(storage_snapshot));
        }
        let _ = self.text_layouts.remove(&stable_key);
        self.refresh_find_matches();
        self.reveal_caret_on_next_frame = true;
        self.lifecycle_notice = Some(format!(
            "Reloaded {} and discarded editor changes",
            self.active_document().title(&self.document_registry)
        ));
        true
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
            let title = self
                .active_document()
                .title(&self.document_registry)
                .to_owned();
            let choice = match self.dialog_service.confirm_dirty_close(&title) {
                Ok(choice) => choice,
                Err(error) => {
                    self.lifecycle_notice = Some(format!("Close dialog unavailable: {error}"));
                    return;
                }
            };
            match choice {
                DirtyCloseChoice::Save if !self.save_active() => return,
                DirtyCloseChoice::Save | DirtyCloseChoice::Discard => {}
                DirtyCloseChoice::Cancel => {
                    self.lifecycle_notice = Some("Close canceled".to_owned());
                    return;
                }
            }
        }
        self.remove_active_document();
    }

    fn remove_active_document(&mut self) {
        let active_id = self.active_document().id;
        let removed_key = active_id.stable_key();
        self.documents.remove(self.active_index);
        let _ = self.document_registry.remove(active_id);
        let _ = self.text_layouts.remove(&removed_key);
        self.active_index = self
            .active_index
            .min(self.documents.len().saturating_sub(1));
        self.select_active_in_sidebar();
        self.lifecycle_notice = None;
        self.reveal_caret_on_next_frame = true;
    }

    fn activate_document_id(&mut self, id: DocumentId) {
        let stable_key = id.stable_key();
        self.activate_document(&stable_key);
    }

    fn activate_document(&mut self, id: &str) {
        if let Some(index) = self
            .documents
            .iter()
            .position(|document| document.stable_key() == id)
        {
            self.active_index = index;
            self.select_active_in_sidebar();
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
        let sidebar_header = if self.workspace.is_some() {
            "WORKSPACE"
        } else {
            "OPEN DOCUMENTS"
        };
        self.append_label(
            display_list,
            "m3-editor-sidebar-header-label",
            sidebar_header,
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

    fn frame_interval(&self) -> Option<Duration> {
        Some(Duration::from_millis(250))
    }

    fn update(&mut self, elapsed: Duration) -> HostControl {
        self.observation_elapsed = self.observation_elapsed.saturating_add(elapsed);
        self.workspace_refresh_elapsed = self.workspace_refresh_elapsed.saturating_add(elapsed);
        let mut invalidation = None;
        if self.observation_elapsed >= EXTERNAL_POLL_INTERVAL {
            self.observation_elapsed = Duration::ZERO;
            if self.poll_external_changes() {
                invalidation = Some(InvalidationClass::TextOverlay);
            }
        }
        if self.workspace_refresh_elapsed >= WORKSPACE_REFRESH_INTERVAL {
            self.workspace_refresh_elapsed = Duration::ZERO;
            if self.refresh_workspace(false) {
                invalidation = Some(InvalidationClass::WidgetLayout);
            }
        }
        invalidation.map_or(HostControl::Continue, HostControl::Invalidate)
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
                                && keyboard.modifiers.contains(Modifiers::SHIFT) =>
                        {
                            Some("save-as")
                        }
                        Key::Character(value)
                            if value.eq_ignore_ascii_case("o")
                                && keyboard.modifiers.contains(Modifiers::SHIFT) =>
                        {
                            Some("open-folder")
                        }
                        Key::Character(value)
                            if value.eq_ignore_ascii_case("r")
                                && keyboard.modifiers.contains(Modifiers::SHIFT) =>
                        {
                            Some("refresh-workspace")
                        }
                        Key::Character(value) if value.eq_ignore_ascii_case("s") => Some("save"),
                        Key::Character(value) if value.eq_ignore_ascii_case("o") => Some("open"),
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
                                self.handle_sidebar_activation(&id);
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
                Some(EditorShellHit::Tab(id)) => {
                    self.activate_document(&id);
                    return HostControl::Invalidate(InvalidationClass::Accessibility);
                }
                Some(EditorShellHit::SidebarItem(id)) => {
                    if request.kind == AccessibilityActionKind::Click {
                        self.handle_sidebar_activation(&id);
                    } else {
                        self.focus_sidebar_item(&id);
                    }
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
    use luna_document_services::{
        DirtyCloseChoice, MemoryTextFileService, SaveConflictChoice, ScriptedDialogService,
        TextFileService, WorkspaceDeleteChoice, WorkspaceDirtyDeleteChoice,
    };
    use luna_documents::{DocumentRecord, DocumentSource, ExternalState};
    use luna_host_winit::{AccessibilityActionKind, AccessibilityActionRequest, NativeApplication};
    use luna_input::{InputEvent, Modifiers, PointerButton, PointerEvent, PointerEventKind};
    use luna_session::{MemorySessionStore, SessionRecentFile, SessionState, SessionWorkspace};
    use luna_workspaces::{MemoryWorkspaceService, WorkspaceNodeKind};
    use std::error::Error;
    use std::path::{Path, PathBuf};

    type TestError = Box<dyn Error + Send + Sync + 'static>;
    type TestResult<T = ()> = Result<T, TestError>;

    fn test_services() -> TestResult<(MemoryTextFileService, ScriptedDialogService)> {
        Ok((
            MemoryTextFileService::new(PathBuf::from("/luna-editor-tests"))?,
            ScriptedDialogService::default(),
        ))
    }

    fn test_application(
        files: &MemoryTextFileService,
        dialogs: &ScriptedDialogService,
    ) -> TestResult<EditorDemoApplication> {
        EditorDemoApplication::with_services(Box::new(files.clone()), Box::new(dialogs.clone()))
    }

    fn test_workspace_service() -> TestResult<MemoryWorkspaceService> {
        Ok(MemoryWorkspaceService::new(PathBuf::from(
            "/luna-editor-tests",
        ))?)
    }

    fn test_application_with_workspace(
        files: &MemoryTextFileService,
        dialogs: &ScriptedDialogService,
        workspace: &MemoryWorkspaceService,
    ) -> TestResult<EditorDemoApplication> {
        EditorDemoApplication::with_all_services(
            Box::new(files.clone()),
            Box::new(dialogs.clone()),
            Box::new(workspace.clone()),
        )
    }

    #[test]
    fn dropdown_and_palette_states_remain_independent() -> TestResult {
        let (files, dialogs) = test_services()?;
        let mut application = test_application(&files, &dialogs)?;

        let _ = application.open_menu("file");
        assert_eq!(application.menu.active_menu_id.as_deref(), Some("file"));
        assert!(application.palette.is_none());

        let _ = application.open_palette();
        assert!(!application.menu.is_open());
        assert!(application.palette.is_some());
        Ok(())
    }

    #[test]
    fn menu_heading_pointer_replaces_palette_with_dropdown() -> TestResult {
        let (files, dialogs) = test_services()?;
        let mut application = test_application(&files, &dialogs)?;
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
    fn command_palette_is_not_projected_into_any_dropdown() -> TestResult {
        let (files, dialogs) = test_services()?;
        let application = test_application(&files, &dialogs)?;
        assert!(application.menu_definitions().iter().all(|menu| {
            menu.items.iter().all(|item| {
                item.as_command()
                    .is_none_or(|command| command.id != "command-palette")
            })
        }));
        Ok(())
    }

    #[test]
    fn palette_projection_includes_real_file_commands() -> TestResult {
        let (files, dialogs) = test_services()?;
        let application = test_application(&files, &dialogs)?;
        let items = application.palette_items();

        assert!(items.iter().any(|item| item.id == "new-file"));
        assert!(items.iter().any(|item| item.id == "open"));
        assert!(items.iter().any(|item| item.id == "save-as"));
        Ok(())
    }

    #[test]
    fn untitled_documents_use_monotonic_registry_titles() -> TestResult {
        let (files, dialogs) = test_services()?;
        let mut application = test_application(&files, &dialogs)?;
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
    fn open_loads_utf8_and_duplicate_open_activates_existing() -> TestResult {
        let (files, dialogs) = test_services()?;
        let path = PathBuf::from("/luna-editor-tests/open.txt");
        files.insert_utf8(&path, "opened text")?;
        dialogs.push_open_file(Some(path.clone()));
        dialogs.push_open_file(Some(path));
        let mut application = test_application(&files, &dialogs)?;

        application.open_file();
        let count = application.documents.len();
        assert_eq!(
            application.active_document().editor.document().text(),
            "opened text"
        );
        application.open_file();

        assert_eq!(application.documents.len(), count);
        assert!(
            application
                .lifecycle_notice
                .as_deref()
                .is_some_and(|notice| { notice.contains("already open") })
        );
        Ok(())
    }

    #[test]
    fn save_as_writes_and_assigns_file_identity() -> TestResult {
        let (files, dialogs) = test_services()?;
        let destination = PathBuf::from("/luna-editor-tests/saved.txt");
        dialogs.push_save_file(Some(destination.clone()));
        let mut application = test_application(&files, &dialogs)?;
        application.new_document();
        let result = application
            .active_document_mut()
            .editor
            .insert_text("saved text");
        assert!(result.did_change);

        assert!(application.save_active());

        assert_eq!(files.bytes(&destination)?, Some(b"saved text".to_vec()));
        let record = application
            .document_registry
            .get(application.active_document().id)
            .ok_or_else(|| std::io::Error::other("missing saved document record"))?;
        assert!(matches!(record.source(), DocumentSource::File(_)));
        assert!(
            !application
                .active_document()
                .is_dirty(&application.document_registry)
        );
        Ok(())
    }

    #[test]
    fn dirty_close_cancel_keeps_document_open() -> TestResult {
        let (files, dialogs) = test_services()?;
        dialogs.push_dirty_close(DirtyCloseChoice::Cancel);
        let mut application = test_application(&files, &dialogs)?;
        application.new_document();
        let count = application.documents.len();
        let result = application
            .active_document_mut()
            .editor
            .insert_text("changed");
        assert!(result.did_change);

        application.close_active();

        assert_eq!(application.documents.len(), count);
        assert_eq!(
            application.lifecycle_notice.as_deref(),
            Some("Close canceled")
        );
        Ok(())
    }

    #[test]
    fn dirty_close_discard_removes_document() -> TestResult {
        let (files, dialogs) = test_services()?;
        dialogs.push_dirty_close(DirtyCloseChoice::Discard);
        let mut application = test_application(&files, &dialogs)?;
        application.new_document();
        let id = application.active_document().id;
        let result = application
            .active_document_mut()
            .editor
            .insert_text("changed");
        assert!(result.did_change);

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
    fn dirty_close_save_writes_then_closes() -> TestResult {
        let (files, dialogs) = test_services()?;
        let destination = PathBuf::from("/luna-editor-tests/close-save.txt");
        dialogs.push_dirty_close(DirtyCloseChoice::Save);
        dialogs.push_save_file(Some(destination.clone()));
        let mut application = test_application(&files, &dialogs)?;
        application.new_document();
        let id = application.active_document().id;
        let result = application
            .active_document_mut()
            .editor
            .insert_text("save before close");
        assert!(result.did_change);

        application.close_active();

        assert_eq!(
            files.bytes(&destination)?,
            Some(b"save before close".to_vec())
        );
        assert!(
            application
                .documents
                .iter()
                .all(|document| document.id != id)
        );
        Ok(())
    }

    #[test]
    fn external_save_conflict_can_reload_storage_content() -> TestResult {
        let (files, dialogs) = test_services()?;
        let path = PathBuf::from("/luna-editor-tests/conflict.txt");
        files.insert_utf8(&path, "baseline")?;
        dialogs.push_open_file(Some(path.clone()));
        dialogs.push_save_conflict(SaveConflictChoice::Reload);
        let mut application = test_application(&files, &dialogs)?;
        application.open_file();
        let result = application
            .active_document_mut()
            .editor
            .insert_text(" editor change");
        assert!(result.did_change);
        files.insert_utf8(&path, "external change")?;

        assert!(application.save_active());

        assert_eq!(
            application.active_document().editor.document().text(),
            "external change"
        );
        assert!(
            !application
                .active_document()
                .is_dirty(&application.document_registry)
        );
        Ok(())
    }

    #[test]
    fn external_save_conflict_can_overwrite_after_confirmation() -> TestResult {
        let (files, dialogs) = test_services()?;
        let path = PathBuf::from("/luna-editor-tests/conflict-overwrite.txt");
        files.insert_utf8(&path, "baseline")?;
        dialogs.push_open_file(Some(path.clone()));
        dialogs.push_save_conflict(SaveConflictChoice::Overwrite);
        let mut application = test_application(&files, &dialogs)?;
        application.open_file();
        let result = application
            .active_document_mut()
            .editor
            .insert_text("editor ");
        assert!(result.did_change);
        files.insert_utf8(&path, "external change")?;

        assert!(application.save_active());

        assert_eq!(files.bytes(&path)?, Some(b"editor baseline".to_vec()));
        assert!(
            !application
                .active_document()
                .is_dirty(&application.document_registry)
        );
        Ok(())
    }

    #[test]
    fn external_save_conflict_cancel_preserves_both_versions() -> TestResult {
        let (files, dialogs) = test_services()?;
        let path = PathBuf::from("/luna-editor-tests/conflict-cancel.txt");
        files.insert_utf8(&path, "baseline")?;
        dialogs.push_open_file(Some(path.clone()));
        dialogs.push_save_conflict(SaveConflictChoice::Cancel);
        let mut application = test_application(&files, &dialogs)?;
        application.open_file();
        let result = application
            .active_document_mut()
            .editor
            .insert_text("editor ");
        assert!(result.did_change);
        files.insert_utf8(&path, "external change")?;

        assert!(!application.save_active());

        assert_eq!(files.bytes(&path)?, Some(b"external change".to_vec()));
        assert_eq!(
            application.active_document().editor.document().text(),
            "editor baseline"
        );
        assert!(
            application
                .active_document()
                .is_dirty(&application.document_registry)
        );
        Ok(())
    }

    #[test]
    fn recent_files_are_projected_and_reopen_existing_document() -> TestResult {
        let (files, dialogs) = test_services()?;
        let path = PathBuf::from("/luna-editor-tests/recent.txt");
        files.insert_utf8(&path, "recent")?;
        dialogs.push_open_file(Some(path));
        let mut application = test_application(&files, &dialogs)?;
        application.open_file();
        let count = application.documents.len();

        assert!(application.menu_definitions().iter().any(|menu| {
            menu.items.iter().any(|item| {
                item.as_command()
                    .is_some_and(|command| command.id == "open-recent-0")
            })
        }));
        application.open_recent_file(0);
        assert_eq!(application.documents.len(), count);
        assert!(
            application
                .lifecycle_notice
                .as_deref()
                .is_some_and(|notice| notice.contains("already open"))
        );
        Ok(())
    }

    #[test]
    fn unchanged_polling_requests_no_state_change() -> TestResult {
        let (files, dialogs) = test_services()?;
        let path = PathBuf::from("/luna-editor-tests/unchanged.txt");
        files.insert_utf8(&path, "baseline")?;
        dialogs.push_open_file(Some(path));
        let mut application = test_application(&files, &dialogs)?;
        application.open_file();

        assert!(!application.poll_external_changes());
        assert_eq!(
            application
                .document_registry
                .get(application.active_document().id)
                .map(DocumentRecord::external_state),
            Some(ExternalState::InSync)
        );
        Ok(())
    }

    #[test]
    fn polling_reports_modified_replaced_missing_and_recreated_files() -> TestResult {
        let (files, dialogs) = test_services()?;
        let path = PathBuf::from("/luna-editor-tests/observed.txt");
        files.insert_utf8(&path, "baseline")?;
        dialogs.push_open_file(Some(path.clone()));
        let mut application = test_application(&files, &dialogs)?;
        application.open_file();

        files.modify_utf8_in_place(&path, "modified")?;
        assert!(application.poll_external_changes());
        assert!(matches!(
            application
                .document_registry
                .get(application.active_document().id)
                .map(DocumentRecord::external_state),
            Some(ExternalState::Modified { .. })
        ));
        assert!(
            application
                .shell_state()
                .status_left
                .contains("Changed on disk")
        );

        files.insert_utf8(&path, "replacement")?;
        assert!(application.poll_external_changes());
        assert!(matches!(
            application
                .document_registry
                .get(application.active_document().id)
                .map(DocumentRecord::external_state),
            Some(ExternalState::Replaced { .. })
        ));
        assert!(
            application
                .shell_state()
                .status_left
                .contains("Replaced on disk")
        );

        assert!(files.remove_file(&path)?);
        assert!(application.poll_external_changes());
        assert_eq!(
            application
                .document_registry
                .get(application.active_document().id)
                .map(DocumentRecord::external_state),
            Some(ExternalState::Missing)
        );
        assert!(
            application
                .shell_state()
                .status_left
                .contains("Deleted on disk")
        );

        files.insert_utf8(&path, "recreated")?;
        assert!(application.poll_external_changes());
        assert!(matches!(
            application
                .document_registry
                .get(application.active_document().id)
                .map(DocumentRecord::external_state),
            Some(ExternalState::Recreated { .. })
        ));
        assert!(
            application
                .shell_state()
                .status_left
                .contains("Recreated on disk")
        );
        Ok(())
    }

    #[test]
    fn reload_command_refreshes_observed_storage_and_clears_notice() -> TestResult {
        let (files, dialogs) = test_services()?;
        let path = PathBuf::from("/luna-editor-tests/reload-observed.txt");
        files.insert_utf8(&path, "baseline")?;
        dialogs.push_open_file(Some(path.clone()));
        let mut application = test_application(&files, &dialogs)?;
        application.open_file();
        files.modify_utf8_in_place(&path, "external")?;
        assert!(application.poll_external_changes());

        assert!(application.reload_active_file());
        assert_eq!(
            application.active_document().editor.document().text(),
            "external"
        );
        assert_eq!(
            application
                .document_registry
                .get(application.active_document().id)
                .map(DocumentRecord::external_state),
            Some(ExternalState::InSync)
        );
        Ok(())
    }

    #[test]
    fn invalid_utf8_open_reports_failure_without_new_document() -> TestResult {
        let (files, dialogs) = test_services()?;
        let path = PathBuf::from("/luna-editor-tests/invalid.txt");
        files.insert_bytes(&path, vec![0xff, 0xfe])?;
        dialogs.push_open_file(Some(path));
        let mut application = test_application(&files, &dialogs)?;
        let count = application.documents.len();

        application.open_file();

        assert_eq!(application.documents.len(), count);
        assert!(
            application
                .lifecycle_notice
                .as_deref()
                .is_some_and(|notice| { notice.contains("UTF-8") })
        );
        Ok(())
    }

    #[test]
    fn clean_close_removes_document_and_registry_record() -> TestResult {
        let (files, dialogs) = test_services()?;
        let mut application = test_application(&files, &dialogs)?;
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
    fn save_as_cancellation_preserves_dirty_state() -> TestResult {
        let (files, dialogs) = test_services()?;
        dialogs.push_save_file(None);
        let mut application = test_application(&files, &dialogs)?;
        application.new_document();
        let result = application
            .active_document_mut()
            .editor
            .insert_text("changed");
        assert!(result.did_change);

        assert!(!application.save_active());

        assert!(
            application
                .active_document()
                .is_dirty(&application.document_registry)
        );
        assert_eq!(
            application.lifecycle_notice.as_deref(),
            Some("Save As canceled")
        );
        Ok(())
    }

    #[test]
    fn save_as_duplicate_identity_does_not_overwrite_open_file() -> TestResult {
        let (files, dialogs) = test_services()?;
        let path = PathBuf::from("/luna-editor-tests/owned.txt");
        files.insert_utf8(&path, "owned")?;
        dialogs.push_open_file(Some(path.clone()));
        dialogs.push_save_file(Some(path.clone()));
        let mut application = test_application(&files, &dialogs)?;
        application.open_file();
        let owner_id = application.active_document().id;
        application.new_document();
        let result = application
            .active_document_mut()
            .editor
            .insert_text("replacement");
        assert!(result.did_change);

        assert!(!application.save_active_as());

        assert_eq!(application.active_document().id, owner_id);
        assert_eq!(files.bytes(&path)?, Some(b"owned".to_vec()));
        Ok(())
    }

    #[test]
    fn save_existing_file_updates_storage_revision() -> TestResult {
        let (files, dialogs) = test_services()?;
        let path = PathBuf::from("/luna-editor-tests/existing.txt");
        files.insert_utf8(&path, "before")?;
        dialogs.push_open_file(Some(path.clone()));
        let mut application = test_application(&files, &dialogs)?;
        application.open_file();
        let result = application
            .active_document_mut()
            .editor
            .insert_text(" after");
        assert!(result.did_change);

        assert!(application.save_active());

        assert_eq!(files.bytes(&path)?, Some(b" afterbefore".to_vec()));
        assert!(
            !application
                .active_document()
                .is_dirty(&application.document_registry)
        );
        Ok(())
    }

    #[test]
    fn file_menu_commands_are_enabled() -> TestResult {
        let (files, dialogs) = test_services()?;
        let application = test_application(&files, &dialogs)?;
        let file_menu = application
            .menu_definitions()
            .into_iter()
            .find(|menu| menu.id == "file")
            .ok_or_else(|| std::io::Error::other("missing File menu"))?;
        let enabled_ids = file_menu
            .items
            .iter()
            .filter_map(|item| item.as_command())
            .filter(|command| command.is_enabled)
            .map(|command| command.id.as_str())
            .collect::<Vec<_>>();

        assert!(enabled_ids.contains(&"open"));
        assert!(enabled_ids.contains(&"open-folder"));
        assert!(enabled_ids.contains(&"save"));
        assert!(enabled_ids.contains(&"save-as"));
        Ok(())
    }

    #[test]
    fn open_cancel_does_not_change_document_count() -> TestResult {
        let (files, dialogs) = test_services()?;
        dialogs.push_open_file(None);
        let mut application = test_application(&files, &dialogs)?;
        let count = application.documents.len();

        application.open_file();

        assert_eq!(application.documents.len(), count);
        assert_eq!(
            application.lifecycle_notice.as_deref(),
            Some("Open canceled")
        );
        Ok(())
    }

    #[test]
    fn scripted_services_use_canonical_test_paths() -> TestResult {
        let (files, _dialogs) = test_services()?;
        files.insert_utf8(Path::new("relative.txt"), "text")?;
        let loaded = files.load_utf8(Path::new("relative.txt"))?;

        assert_eq!(
            loaded.identity().path(),
            Path::new("/luna-editor-tests/relative.txt")
        );
        Ok(())
    }
    #[test]
    fn open_folder_projects_real_workspace_rows() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        workspace.insert_file(Path::new("src/main.rs"))?;
        files.insert_utf8(Path::new("src/main.rs"), "fn main() {}\n")?;
        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;

        let _ = application.execute_command("open-folder");

        let model = application
            .workspace
            .as_ref()
            .ok_or_else(|| std::io::Error::other("workspace was not opened"))?;
        assert_eq!(model.snapshot().root(), Path::new("/luna-editor-tests"));
        let rows = application.sidebar_items();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].title, "luna-editor-tests");
        assert_eq!(rows[1].title, "src");
        assert!(application.sidebar_is_visible);
        Ok(())
    }

    #[test]
    fn folder_activation_expands_and_file_activation_opens_document() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        workspace.insert_file(Path::new("src/main.rs"))?;
        files.insert_utf8(Path::new("src/main.rs"), "fn main() {}\n")?;
        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;
        application.open_workspace();

        let source_id = application
            .workspace
            .as_ref()
            .and_then(|model| {
                model
                    .snapshot()
                    .node_for_path(Path::new("/luna-editor-tests/src"))
            })
            .ok_or_else(|| std::io::Error::other("source directory missing"))?
            .id()
            .stable_key()
            .to_owned();
        application.handle_sidebar_activation(&source_id);
        assert_eq!(application.sidebar_items().len(), 3);

        let file_id = application
            .workspace
            .as_ref()
            .and_then(|model| {
                model
                    .snapshot()
                    .node_for_path(Path::new("/luna-editor-tests/src/main.rs"))
            })
            .ok_or_else(|| std::io::Error::other("workspace file missing"))?
            .id()
            .stable_key()
            .to_owned();
        application.handle_sidebar_activation(&file_id);

        assert_eq!(
            application
                .active_document()
                .title(&application.document_registry),
            "main.rs"
        );
        assert_eq!(
            application.selected_sidebar_id.as_deref(),
            Some(file_id.as_str())
        );
        assert_eq!(application.documents.len(), 4);
        Ok(())
    }

    #[test]
    fn accessibility_focus_selects_workspace_row_without_activating_it() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        workspace.insert_file(Path::new("src/main.rs"))?;
        files.insert_utf8(Path::new("src/main.rs"), "fn main() {}\n")?;
        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;
        application.open_workspace();
        let source_node_id = application
            .workspace
            .as_ref()
            .and_then(|model| {
                model
                    .snapshot()
                    .node_for_path(Path::new("/luna-editor-tests/src"))
            })
            .ok_or_else(|| std::io::Error::other("source directory missing"))?
            .id()
            .clone();
        let source_key = source_node_id.stable_key().to_owned();
        let shell = application.create_shell()?;
        let source_frame = shell
            .layout()
            .sidebar_rows
            .iter()
            .find(|frame| frame.id == source_key)
            .ok_or_else(|| std::io::Error::other("source sidebar row missing"))?;
        let document_count = application.documents.len();

        let _ = application.handle_accessibility_action(AccessibilityActionRequest {
            target: Some(source_frame.node_id.clone()),
            kind: AccessibilityActionKind::Focus,
        });

        assert_eq!(application.documents.len(), document_count);
        assert_eq!(
            application.selected_sidebar_id.as_deref(),
            Some(source_key.as_str())
        );
        assert!(
            application
                .workspace
                .as_ref()
                .is_some_and(|model| !model.is_expanded(&source_node_id))
        );
        Ok(())
    }

    #[test]
    fn workspace_refresh_preserves_expansion_and_adds_rows() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        workspace.insert_file(Path::new("src/main.rs"))?;
        files.insert_utf8(Path::new("src/main.rs"), "fn main() {}\n")?;
        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;
        application.open_workspace();
        let source_id = application
            .workspace
            .as_ref()
            .and_then(|model| {
                model
                    .snapshot()
                    .node_for_path(Path::new("/luna-editor-tests/src"))
            })
            .ok_or_else(|| std::io::Error::other("source directory missing"))?
            .id()
            .stable_key()
            .to_owned();
        application.handle_sidebar_activation(&source_id);
        workspace.insert_file(Path::new("src/lib.rs"))?;
        files.insert_utf8(Path::new("src/lib.rs"), "pub fn demo() {}\n")?;

        assert!(application.refresh_workspace(false));

        let rows = application.sidebar_items();
        assert!(rows.iter().any(|row| row.title == "lib.rs"));
        assert!(rows.iter().any(|row| row.title == "main.rs"));
        assert!(
            rows.iter()
                .any(|row| row.id == source_id && row.is_expanded)
        );
        Ok(())
    }

    #[test]
    fn opening_workspace_file_twice_activates_existing_tab() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        workspace.insert_file(Path::new("README.md"))?;
        files.insert_utf8(Path::new("README.md"), "# Workspace\n")?;
        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;
        application.open_workspace();
        let file_key = application
            .workspace
            .as_ref()
            .and_then(|model| {
                model
                    .snapshot()
                    .node_for_path(Path::new("/luna-editor-tests/README.md"))
            })
            .ok_or_else(|| std::io::Error::other("README row missing"))?
            .id()
            .stable_key()
            .to_owned();

        application.handle_sidebar_activation(&file_key);
        let count = application.documents.len();
        application.handle_sidebar_activation(&file_key);

        assert_eq!(application.documents.len(), count);
        assert_eq!(
            application
                .active_document()
                .title(&application.document_registry),
            "README.md"
        );
        Ok(())
    }

    #[test]
    fn symlink_rows_are_visible_but_not_opened() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        workspace.insert_symlink(Path::new("linked"))?;
        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;
        application.open_workspace();
        let link = application
            .workspace
            .as_ref()
            .and_then(|model| {
                model
                    .snapshot()
                    .node_for_path(Path::new("/luna-editor-tests/linked"))
            })
            .ok_or_else(|| std::io::Error::other("symlink row missing"))?;
        assert_eq!(link.kind(), WorkspaceNodeKind::Symlink);
        let link_key = link.id().stable_key().to_owned();
        let count = application.documents.len();

        application.handle_sidebar_activation(&link_key);

        assert_eq!(application.documents.len(), count);
        assert!(
            application
                .lifecycle_notice
                .as_deref()
                .is_some_and(|notice| notice.contains("not followed"))
        );
        Ok(())
    }

    #[test]
    fn close_workspace_restores_open_document_sidebar() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;
        application.open_workspace();
        assert!(application.workspace.is_some());

        application.close_workspace();

        assert!(application.workspace.is_none());
        assert_eq!(application.sidebar_items()[0].title, "Open Documents");
        let active_key = application.active_document().stable_key();
        assert_eq!(
            application.selected_sidebar_id.as_deref(),
            Some(active_key.as_str())
        );
        Ok(())
    }

    #[test]
    fn file_menu_projects_workspace_commands_only_while_open() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;
        let closed_ids = application
            .menu_definitions()
            .into_iter()
            .find(|menu| menu.id == "file")
            .ok_or_else(|| std::io::Error::other("missing File menu"))?
            .items
            .into_iter()
            .filter_map(|item| item.as_command().map(|command| command.id.clone()))
            .collect::<Vec<_>>();
        assert!(closed_ids.iter().any(|id| id == "open-folder"));
        assert!(!closed_ids.iter().any(|id| id == "refresh-workspace"));

        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        application.open_workspace();
        let open_ids = application
            .menu_definitions()
            .into_iter()
            .find(|menu| menu.id == "file")
            .ok_or_else(|| std::io::Error::other("missing File menu"))?
            .items
            .into_iter()
            .filter_map(|item| item.as_command().map(|command| command.id.clone()))
            .collect::<Vec<_>>();
        assert!(open_ids.iter().any(|id| id == "refresh-workspace"));
        assert!(open_ids.iter().any(|id| id == "close-workspace"));
        let palette_ids = application
            .palette_items()
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        assert!(palette_ids.iter().any(|id| id == "refresh-workspace"));
        assert!(palette_ids.iter().any(|id| id == "close-workspace"));
        Ok(())
    }
    #[test]
    fn workspace_create_rename_and_delete_commands_mutate_tree() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;
        application.open_workspace();

        dialogs.push_workspace_name(Some("created.txt".to_owned()));
        application.create_workspace_file();
        assert!(
            application
                .workspace
                .as_ref()
                .and_then(|model| model
                    .snapshot()
                    .node_for_path(Path::new("/luna-editor-tests/created.txt")))
                .is_some()
        );

        application.select_workspace_path(Path::new("/luna-editor-tests/created.txt"));
        dialogs.push_workspace_name(Some("renamed.txt".to_owned()));
        application.rename_workspace_entry();
        assert!(
            application
                .workspace
                .as_ref()
                .and_then(|model| model
                    .snapshot()
                    .node_for_path(Path::new("/luna-editor-tests/renamed.txt")))
                .is_some()
        );

        dialogs.push_workspace_delete(WorkspaceDeleteChoice::Delete);
        application.delete_workspace_entry();
        assert!(
            application
                .workspace
                .as_ref()
                .and_then(|model| model
                    .snapshot()
                    .node_for_path(Path::new("/luna-editor-tests/renamed.txt")))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn deleting_dirty_workspace_file_can_keep_buffer_as_untitled() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        let path = Path::new("/luna-editor-tests/dirty.txt");
        files.insert_utf8(path, "original")?;
        workspace.insert_file(Path::new("dirty.txt"))?;
        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;
        application.open_workspace();
        application.open_path(path);
        let _ = application
            .active_document_mut()
            .editor
            .insert_text(" changed");
        application.select_workspace_path(path);
        dialogs.push_workspace_delete(WorkspaceDeleteChoice::Delete);
        dialogs.push_workspace_dirty_delete(WorkspaceDirtyDeleteChoice::KeepOpen);

        application.delete_workspace_entry();

        assert!(matches!(
            application
                .document_registry
                .get(application.active_document().id)
                .map(DocumentRecord::source),
            Some(DocumentSource::Untitled { .. })
        ));
        assert!(
            application
                .active_document()
                .is_dirty(&application.document_registry)
        );
        Ok(())
    }

    #[test]
    fn persisted_recent_files_and_workspace_tree_are_restored() -> TestResult {
        let files = MemoryTextFileService::new(PathBuf::from("/luna-editor-tests"))?;
        let dialogs = ScriptedDialogService::default();
        let workspace = test_workspace_service()?;
        workspace.insert_file(Path::new("src/main.rs"))?;
        let session = MemorySessionStore::default();
        session.set_state(SessionState {
            recent_files: vec![SessionRecentFile {
                path: PathBuf::from("/luna-editor-tests/recent.txt"),
                title: "recent.txt".to_owned(),
            }],
            workspace: Some(SessionWorkspace {
                root: PathBuf::from("/luna-editor-tests"),
                expanded_paths: vec![PathBuf::from("/luna-editor-tests/src")],
                selected_path: Some(PathBuf::from("/luna-editor-tests/src/main.rs")),
            }),
        });
        let application = EditorDemoApplication::with_runtime_services(
            Box::new(files),
            Box::new(dialogs),
            Box::new(workspace),
            Box::new(session.clone()),
        )?;

        assert_eq!(application.recent_files.entries().len(), 1);
        let restored_workspace = application
            .workspace
            .as_ref()
            .ok_or_else(|| std::io::Error::other("workspace was not restored"))?;
        assert_eq!(
            restored_workspace
                .selected()
                .and_then(|id| restored_workspace.snapshot().node(id))
                .map(|node| node.path()),
            Some(Path::new("/luna-editor-tests/src/main.rs")),
        );
        assert_eq!(
            session
                .state()
                .workspace
                .as_ref()
                .map(|state| state.root.as_path()),
            Some(Path::new("/luna-editor-tests"))
        );
        Ok(())
    }

    #[test]
    fn file_menu_projects_workspace_mutation_commands_only_with_workspace() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;
        let closed = application
            .menu_definitions()
            .into_iter()
            .find(|menu| menu.id == "file")
            .ok_or_else(|| std::io::Error::other("file menu missing"))?;
        assert!(!closed.items.iter().any(|item| {
            matches!(item, luna_ui::MenuItem::Command(command) if command.id == "new-workspace-file")
        }));

        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        application.open_workspace();
        let open = application
            .menu_definitions()
            .into_iter()
            .find(|menu| menu.id == "file")
            .ok_or_else(|| std::io::Error::other("file menu missing"))?;
        assert!(open.items.iter().any(|item| {
            matches!(item, luna_ui::MenuItem::Command(command) if command.id == "new-workspace-file")
        }));
        Ok(())
    }

    #[test]
    fn create_collision_never_replaces_an_open_workspace_file() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        let path = Path::new("/luna-editor-tests/existing.txt");
        files.insert_utf8(path, "existing")?;
        workspace.insert_file(Path::new("existing.txt"))?;
        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;
        application.open_workspace();
        application.open_path(path);
        application.select_workspace_path(Path::new("/luna-editor-tests"));
        dialogs.push_workspace_name(Some("existing.txt".to_owned()));

        application.create_workspace_file();

        assert_eq!(files.bytes(path)?, Some(b"existing".to_vec()));
        assert!(
            application
                .lifecycle_notice
                .as_deref()
                .is_some_and(|notice| notice.contains("already open"))
        );
        Ok(())
    }

    #[test]
    fn renaming_open_dirty_file_relocates_identity_without_clearing_dirty_state() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        let source = Path::new("/luna-editor-tests/source.txt");
        let destination = Path::new("/luna-editor-tests/destination.txt");
        files.insert_utf8(source, "original")?;
        files.insert_utf8(destination, "original")?;
        workspace.insert_file(Path::new("source.txt"))?;
        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;
        application.open_workspace();
        application.open_path(source);
        let _ = application
            .active_document_mut()
            .editor
            .insert_text(" changed");
        application.select_workspace_path(source);
        dialogs.push_workspace_name(Some("destination.txt".to_owned()));

        application.rename_workspace_entry();

        let active = application.active_document();
        let record = application
            .document_registry
            .get(active.id)
            .ok_or_else(|| std::io::Error::other("relocated document missing"))?;
        assert!(matches!(
            record.source(),
            DocumentSource::File(identity) if identity.path() == destination
        ));
        assert!(active.is_dirty(&application.document_registry));
        Ok(())
    }

    #[test]
    fn directory_rename_relocates_unopened_recent_files() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        workspace.insert_file(Path::new("folder/recent.txt"))?;
        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;
        application.open_workspace();
        let previous =
            files.identity_for_save(Path::new("/luna-editor-tests/folder/recent.txt"))?;
        application.recent_files.record(previous, "recent.txt");
        application.select_workspace_path(Path::new("/luna-editor-tests/folder"));
        dialogs.push_workspace_name(Some("renamed-folder".to_owned()));

        application.rename_workspace_entry();

        assert_eq!(
            application.recent_files.entries()[0].identity().path(),
            Path::new("/luna-editor-tests/renamed-folder/recent.txt")
        );
        Ok(())
    }

    #[test]
    fn directory_delete_removes_unopened_recent_files_below_it() -> TestResult {
        let (files, dialogs) = test_services()?;
        let workspace = test_workspace_service()?;
        workspace.insert_file(Path::new("folder/recent.txt"))?;
        dialogs.push_open_folder(Some(PathBuf::from("/luna-editor-tests")));
        let mut application = test_application_with_workspace(&files, &dialogs, &workspace)?;
        application.open_workspace();
        let recent = files.identity_for_save(Path::new("/luna-editor-tests/folder/recent.txt"))?;
        application.recent_files.record(recent, "recent.txt");
        application.select_workspace_path(Path::new("/luna-editor-tests/folder"));
        dialogs.push_workspace_delete(WorkspaceDeleteChoice::Delete);

        application.delete_workspace_entry();

        assert!(application.recent_files.entries().is_empty());
        Ok(())
    }
}
