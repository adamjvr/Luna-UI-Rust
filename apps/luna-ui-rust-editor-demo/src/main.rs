// SPDX-License-Identifier: MPL-2.0

//! Native M3.3b editor integration harness for Luna UI Rust.
//!
//! This application mirrors the purpose of Swift LunaUITestApp's default editor mode: reusable
//! shell anatomy and editor text are exercised together without embedding Moth Text product
//! policy in Luna. It provides menus, tabs, a project sidebar, status chrome, editable text,
//! real dropdown menus, a separate command palette, find panel, document lifecycle tracking,
//! UTF-8 open/save services, native dialog boundaries, accessibility,
//! retained document layouts, recent-file projection, UI-thread external observation, real
//! workspace trees, controlled workspace mutations, persistent session restoration, overscanned
//! glyph rasters, stable-slot chrome-label caching, recursive split panes, pane-local tabs, and
//! shared document buffers with independent editor-view state, advanced tab overflow and drag
//! behavior, desktop submenus and context menus, completion popups, richer find/replace, and
//! interactive editor scrollbars.
//!
//! Shortcuts: Control-P command palette, Control-O open, Control-F find, Control-H replace,
//! Control-S save, Control-Shift-S Save As, Control-Shift-O Open Folder, Control-Shift-R refresh
//! workspace, Control-N new document, Control-B sidebar, Control-W close pane-local tab,
//! Control-\ split right, Control-Shift-\ split down, Control-Alt-Left/Right focus panes,
//! Control-Shift-W close pane, Control-Space completion popup, Control-A select all, and Escape
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
    CloseRequirement, DocumentId, DocumentRecord, DocumentRegistry, DocumentSource, DocumentViewId,
    DocumentViewRegistry, ExternalState, FileIdentity, OpenFileOutcome, RecentFileList,
    SaveRequirement,
};
use luna_host_winit::{
    AccessibilityActionKind, AccessibilityActionRequest, ApplicationError, HostControl,
    InvalidationClass, NativeApplication, WindowConfig, run_native,
};
use luna_input::{
    InputEvent, Key, Modifiers, NamedKey, PointerButton, PointerEvent, PointerEventKind,
};
use luna_panes::{PaneAxis, PaneId, PaneLayoutMetrics, PaneTree};
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
    CommandPalette, CommandPaletteState, CompletionItem, CompletionPopup, CompletionPopupState,
    DropdownMenu, DropdownMenuState, EditorPaneSurface, EditorPaneSurfaceHit,
    EditorPaneSurfaceState, EditorShell, EditorShellHit, EditorShellMetrics, EditorShellState,
    FindField, FindPanel, FindPanelState, MenuCommand, MenuDefinition, MenuItem, PaletteItem,
    PanePresentation, PaneTab, ShellMenu, SidebarItem, TabScrollDirection, TextAlignment,
    TextLabel, TextLabelCache, TextView, TextViewStyle, UiFrame, Widget,
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
const PANE_SURFACE_ID: &str = "m3-editor-panes";
const PALETTE_ID: &str = "m3-editor-palette";
const FIND_ID: &str = "m3-editor-find";
const COMPLETION_ID: &str = "m3-editor-completion";
const MENU_ID: &str = "m3-editor-dropdown-menu";
const CONTEXT_MENU_ID: &str = "m3-editor-context-menu";
const EXTERNAL_POLL_INTERVAL: Duration = Duration::from_millis(750);
const WORKSPACE_REFRESH_INTERVAL: Duration = Duration::from_millis(1_000);
const RECENT_FILE_LIMIT: usize = 8;

const README_TEXT: &str = concat!(
    "# Luna UI Rust\n\n",
    "M3.3b adds advanced desktop tabs and popup interaction.\n\n",
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
    "- Persistent recent files and workspace tree restoration\n",
    "- Recursive horizontal and vertical editor panes\n",
    "- Pane-local tabs and independent caret, selection, and scroll\n",
    "- Shared document buffers across multiple views\n",
    "- Pinned, preview, reordered, and cross-pane tabs\n",
    "- Tab overflow controls and active-tab visibility\n",
    "- Nested menus, tab context menus, and mnemonics\n",
    "- Completion popup and richer find/replace controls\n",
    "- Interactive vertical scrollbars\n\n",
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct PaneViewState {
    id: DocumentViewId,
    document_id: DocumentId,
    editor: EditableText,
    scroll: TextScroll,
    text_node_id: NodeId,
}

impl PaneViewState {
    fn new(id: DocumentViewId, document: &DemoDocument) -> Result<Self, ApplicationError> {
        Ok(Self {
            id,
            document_id: document.id,
            editor: document.editor.clone(),
            scroll: document.scroll,
            text_node_id: NodeId::new(format!("{TEXT_ID}-{}", id.value()))?,
        })
    }

    fn stable_key(&self) -> String {
        self.id.stable_key()
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DraggedPaneTab {
    pane_id: PaneId,
    view_id: DocumentViewId,
    origin: PointI,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TabContextMenuState {
    pane_id: PaneId,
    view_id: DocumentViewId,
    anchor: RectI,
    menu: DropdownMenuState,
}

struct EditorDemoApplication {
    root_id: NodeId,
    shell_id: NodeId,
    pane_surface_id: NodeId,
    palette_id: NodeId,
    find_id: NodeId,
    completion_id: NodeId,
    menu_id: NodeId,
    context_menu_id: NodeId,
    document_registry: DocumentRegistry,
    view_registry: DocumentViewRegistry,
    pane_tree: PaneTree,
    pane_views: Vec<PaneViewState>,
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
    completion: Option<CompletionPopupState>,
    menu: DropdownMenuState,
    tab_context_menu: Option<TabContextMenuState>,
    find: Option<FindPanelState>,
    find_matches: Vec<Range<usize>>,
    drag_anchor: Option<TextLocation>,
    dragged_splitter: Option<PaneId>,
    dragged_tab: Option<DraggedPaneTab>,
    dragged_scrollbar: Option<DocumentViewId>,
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
        let documents = vec![
            DemoDocument::new(readme_id, README_TEXT),
            DemoDocument::new(editor_id, EDITOR_TEXT),
            DemoDocument::new(theme_id, THEME_TEXT),
        ];
        let mut view_registry = DocumentViewRegistry::new();
        let readme_view = view_registry.create_view(readme_id);
        let editor_view = view_registry.create_view(editor_id);
        let theme_view = view_registry.create_view(theme_id);
        let mut pane_tree = PaneTree::new(readme_view);
        let root_pane = pane_tree.focused_pane();
        pane_tree
            .add_view(root_pane, editor_view)
            .map_err(|error| Box::new(error) as ApplicationError)?;
        pane_tree
            .add_view(root_pane, theme_view)
            .map_err(|error| Box::new(error) as ApplicationError)?;
        pane_tree
            .activate_view(root_pane, editor_view)
            .map_err(|error| Box::new(error) as ApplicationError)?;
        let mut pane_views = Vec::new();
        for (view_id, document) in [readme_view, editor_view, theme_view]
            .into_iter()
            .zip(documents.iter())
        {
            pane_views.push(PaneViewState::new(view_id, document)?);
        }
        let mut application = Self {
            root_id: NodeId::new(ROOT_ID)?,
            shell_id: NodeId::new(SHELL_ID)?,
            pane_surface_id: NodeId::new(PANE_SURFACE_ID)?,
            palette_id: NodeId::new(PALETTE_ID)?,
            find_id: NodeId::new(FIND_ID)?,
            completion_id: NodeId::new(COMPLETION_ID)?,
            menu_id: NodeId::new(MENU_ID)?,
            context_menu_id: NodeId::new(CONTEXT_MENU_ID)?,
            document_registry,
            view_registry,
            pane_tree,
            pane_views,
            file_service,
            dialog_service,
            workspace_service,
            session_store,
            workspace: None,
            workspace_options: WorkspaceScanOptions::default(),
            recent_files: RecentFileList::new(RECENT_FILE_LIMIT),
            documents,
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
            completion: None,
            menu: DropdownMenuState::default(),
            tab_context_menu: None,
            find: None,
            find_matches: Vec::new(),
            drag_anchor: None,
            dragged_splitter: None,
            dragged_tab: None,
            dragged_scrollbar: None,
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

    fn active_view_index(&self) -> usize {
        let view_id = self.pane_tree.focused_view();
        self.pane_views
            .iter()
            .position(|view| view.id == view_id)
            .unwrap_or(0)
            .min(self.pane_views.len().saturating_sub(1))
    }

    fn active_view(&self) -> &PaneViewState {
        &self.pane_views[self.active_view_index()]
    }

    fn active_view_mut(&mut self) -> &mut PaneViewState {
        let index = self.active_view_index();
        &mut self.pane_views[index]
    }

    fn view_state(&self, view_id: DocumentViewId) -> Option<&PaneViewState> {
        self.pane_views.iter().find(|view| view.id == view_id)
    }

    fn view_state_mut(&mut self, view_id: DocumentViewId) -> Option<&mut PaneViewState> {
        self.pane_views.iter_mut().find(|view| view.id == view_id)
    }

    fn sync_active_index_from_view(&mut self) {
        let document_id = self.active_view().document_id;
        if let Some(index) = self
            .documents
            .iter()
            .position(|document| document.id == document_id)
        {
            self.active_index = index;
        }
    }

    fn synchronize_document_views(&mut self, document_id: DocumentId) {
        let Some(document) = self
            .documents
            .iter()
            .find(|document| document.id == document_id)
        else {
            return;
        };
        let text = document.editor.document().text().to_owned();
        let revision = document.editor.edit_revision();
        for view in self
            .pane_views
            .iter_mut()
            .filter(|view| view.document_id == document_id)
        {
            if view.editor.edit_revision() != revision
                || view.editor.document().text() != text.as_str()
            {
                view.editor.synchronize_document(text.clone(), revision);
            }
        }
    }

    fn commit_active_view_to_buffer(&mut self) {
        let view = self.active_view().clone();
        if let Some(document) = self
            .documents
            .iter_mut()
            .find(|document| document.id == view.document_id)
        {
            document.editor = view.editor.clone();
            document.scroll = view.scroll;
        }
        let text = view.editor.document().text().to_owned();
        let revision = view.editor.edit_revision();
        let is_dirty = self
            .document_registry
            .get(view.document_id)
            .is_some_and(|record| record.is_dirty(revision));
        if is_dirty && let Some(pane_id) = self.pane_tree.pane_for_view(view.id) {
            let _ = self.pane_tree.promote_preview(pane_id, view.id);
        }
        for sibling in self
            .pane_views
            .iter_mut()
            .filter(|sibling| sibling.document_id == view.document_id && sibling.id != view.id)
        {
            if sibling.editor.edit_revision() != revision
                || sibling.editor.document().text() != text.as_str()
            {
                sibling.editor.synchronize_document(text.clone(), revision);
            }
        }
    }

    fn activate_pane_view(&mut self, pane_id: PaneId, view_id: DocumentViewId) {
        if self.pane_tree.focused_view() != view_id {
            self.commit_active_view_to_buffer();
        }
        if self.pane_tree.activate_view(pane_id, view_id).is_ok() {
            self.sync_active_index_from_view();
            self.text_is_focused = true;
            self.reveal_caret_on_next_frame = true;
            self.select_active_in_sidebar();
            self.refresh_find_matches();
        }
    }

    fn can_close_active_view(&self) -> bool {
        let view_count = self
            .pane_tree
            .leaf(self.pane_tree.focused_pane())
            .map_or(0, |leaf| leaf.views().len());
        view_count > 1 || self.pane_tree.leaves().len() > 1 || self.documents.len() > 1
    }

    fn create_view_for_document(
        &mut self,
        document_id: DocumentId,
        source_view: Option<DocumentViewId>,
    ) -> Result<DocumentViewId, ApplicationError> {
        let view_id = self.view_registry.create_view(document_id);
        let state = if let Some(source) = source_view.and_then(|id| self.view_state(id).cloned()) {
            PaneViewState {
                id: view_id,
                document_id,
                editor: source.editor,
                scroll: source.scroll,
                text_node_id: NodeId::new(format!("{TEXT_ID}-{}", view_id.value()))?,
            }
        } else {
            let Some(document) = self
                .documents
                .iter()
                .find(|document| document.id == document_id)
            else {
                return Err(std::io::Error::other("document missing for editor view").into());
            };
            PaneViewState::new(view_id, document)?
        };
        self.pane_views.push(state);
        Ok(view_id)
    }

    fn add_document_to_focused_pane(&mut self, document_id: DocumentId) {
        self.add_document_to_focused_pane_with_mode(document_id, false);
    }

    fn add_document_to_focused_pane_with_mode(
        &mut self,
        document_id: DocumentId,
        as_preview: bool,
    ) {
        let pane_id = self.pane_tree.focused_pane();
        let previous_preview = self
            .pane_tree
            .leaf(pane_id)
            .and_then(|leaf| leaf.preview_view());
        if let Some(previous) = previous_preview {
            let previous_document = self
                .view_registry
                .view(previous)
                .map(|record| record.document_id());
            let previous_is_dirty = previous_document.is_some_and(|id| {
                self.documents
                    .iter()
                    .find(|document| document.id == id)
                    .is_some_and(|document| document.is_dirty(&self.document_registry))
            });
            if previous_is_dirty {
                let _ = self.pane_tree.promote_preview(pane_id, previous);
            }
        }
        match self.create_view_for_document(document_id, None) {
            Ok(view_id) => {
                if self.pane_tree.add_view(pane_id, view_id).is_ok() {
                    let replaced_preview = if as_preview {
                        self.pane_tree
                            .set_preview_view(pane_id, view_id)
                            .ok()
                            .flatten()
                    } else {
                        None
                    };
                    self.activate_pane_view(pane_id, view_id);
                    if let Some(replaced_view) = replaced_preview {
                        let replaced_document = self
                            .view_registry
                            .view(replaced_view)
                            .map(|record| record.document_id());
                        if self.pane_tree.close_view(pane_id, replaced_view).is_ok() {
                            let _ = self.view_registry.remove_view(replaced_view);
                            self.pane_views.retain(|view| view.id != replaced_view);
                            let _ = self.text_layouts.remove(&replaced_view.stable_key());
                            if let Some(document_id) = replaced_document
                                && self
                                    .view_registry
                                    .views_for_document(document_id)
                                    .next()
                                    .is_none()
                            {
                                self.remove_document_id(document_id);
                            }
                        }
                    }
                }
            }
            Err(error) => {
                self.lifecycle_notice = Some(format!("Could not create editor view: {error}"));
            }
        }
    }

    fn toggle_active_tab_pin(&mut self, pin: bool) {
        let pane_id = self.pane_tree.focused_pane();
        let view_id = self.pane_tree.focused_view();
        let result = if pin {
            self.pane_tree.pin_view(pane_id, view_id)
        } else {
            self.pane_tree.unpin_view(pane_id, view_id)
        };
        match result {
            Ok(()) => {
                self.lifecycle_notice = Some(if pin {
                    "Pinned active tab".to_owned()
                } else {
                    "Unpinned active tab".to_owned()
                });
            }
            Err(error) => self.lifecycle_notice = Some(error.to_string()),
        }
    }

    fn promote_active_preview(&mut self) {
        let pane_id = self.pane_tree.focused_pane();
        let view_id = self.pane_tree.focused_view();
        match self.pane_tree.promote_preview(pane_id, view_id) {
            Ok(()) => self.lifecycle_notice = Some("Promoted preview tab".to_owned()),
            Err(error) => self.lifecycle_notice = Some(error.to_string()),
        }
    }

    fn move_active_tab_to_relative_pane(&mut self, delta: i32) {
        let leaves = self
            .pane_tree
            .leaves()
            .into_iter()
            .map(|leaf| leaf.id())
            .collect::<Vec<_>>();
        if leaves.len() < 2 {
            self.lifecycle_notice = Some("No other pane is available".to_owned());
            return;
        }
        let source = self.pane_tree.focused_pane();
        let current = leaves.iter().position(|pane| *pane == source).unwrap_or(0);
        let count = i32::try_from(leaves.len()).unwrap_or(i32::MAX).max(1);
        let target_index = (i32::try_from(current).unwrap_or(0) + delta).rem_euclid(count);
        let target = leaves[usize::try_from(target_index).unwrap_or(0)];
        self.move_active_tab_to_pane(target);
    }

    fn move_active_tab_to_pane(&mut self, target: PaneId) {
        self.commit_active_view_to_buffer();
        let source = self.pane_tree.focused_pane();
        let view_id = self.pane_tree.focused_view();
        let target_index = self
            .pane_tree
            .leaf(target)
            .map_or(0, |leaf| leaf.views().len());
        match self
            .pane_tree
            .move_view(source, target, view_id, target_index)
        {
            Ok(pane_id) => {
                self.activate_pane_view(pane_id, view_id);
                self.lifecycle_notice = Some(format!("Moved tab to pane {}", pane_id.value()));
            }
            Err(error) => self.lifecycle_notice = Some(error.to_string()),
        }
    }

    fn scroll_pane_tabs(
        &mut self,
        pane_id: PaneId,
        direction: TabScrollDirection,
        visible_count: usize,
        effective_offset: usize,
    ) {
        let Some(leaf) = self.pane_tree.leaf(pane_id) else {
            return;
        };
        let pinned_count = leaf.pinned_views().len();
        let regular = leaf.views()[pinned_count..].to_vec();
        if regular.is_empty() {
            return;
        }
        let next_offset = match direction {
            TabScrollDirection::Previous => effective_offset.saturating_sub(1),
            TabScrollDirection::Next => effective_offset.saturating_add(1),
        }
        .min(regular.len().saturating_sub(1));
        let _ = self.pane_tree.set_tab_scroll_offset(pane_id, next_offset);
        let activation_index = match direction {
            TabScrollDirection::Previous => next_offset,
            TabScrollDirection::Next => next_offset
                .saturating_add(visible_count.saturating_sub(1))
                .min(regular.len().saturating_sub(1)),
        };
        if let Some(view_id) = regular.get(activation_index).copied() {
            self.activate_pane_view(pane_id, view_id);
        }
    }

    fn split_focused_pane(&mut self, axis: PaneAxis) {
        self.commit_active_view_to_buffer();
        let source_view = self.pane_tree.focused_view();
        let document_id = self.active_document().id;
        match self.create_view_for_document(document_id, Some(source_view)) {
            Ok(view_id) => {
                let pane_id = self.pane_tree.split_focused(axis, view_id);
                self.activate_pane_view(pane_id, view_id);
                self.lifecycle_notice = Some(match axis {
                    PaneAxis::Horizontal => "Split editor right".to_owned(),
                    PaneAxis::Vertical => "Split editor down".to_owned(),
                });
            }
            Err(error) => {
                self.lifecycle_notice = Some(format!("Could not split editor: {error}"));
            }
        }
    }

    fn focus_relative_pane(&mut self, delta: i32) {
        self.commit_active_view_to_buffer();
        if delta < 0 {
            let _ = self.pane_tree.focus_previous();
        } else {
            let _ = self.pane_tree.focus_next();
        }
        self.sync_active_index_from_view();
        self.select_active_in_sidebar();
        self.refresh_find_matches();
        self.reveal_caret_on_next_frame = true;
        self.lifecycle_notice = Some(format!(
            "Focused pane {}",
            self.pane_tree.focused_pane().value()
        ));
    }

    fn close_focused_pane(&mut self) {
        if self.pane_tree.leaves().len() <= 1 {
            self.lifecycle_notice = Some("The final editor pane cannot be closed".to_owned());
            return;
        }
        self.commit_active_view_to_buffer();
        let result = match self.pane_tree.close_focused_pane() {
            Ok(result) => result,
            Err(error) => {
                self.lifecycle_notice = Some(error.to_string());
                return;
            }
        };
        let removed = result.removed_views;
        let destination = result.focused_pane;
        for view_id in removed {
            let document_id = self
                .view_registry
                .view(view_id)
                .map(|record| record.document_id());
            let _ = self.view_registry.remove_view(view_id);
            self.pane_views.retain(|view| view.id != view_id);
            if let Some(document_id) = document_id {
                let needs_replacement = self
                    .view_registry
                    .views_for_document(document_id)
                    .next()
                    .is_none();
                if needs_replacement
                    && let Ok(replacement) = self.create_view_for_document(document_id, None)
                {
                    let _ = self.pane_tree.add_view(destination, replacement);
                }
            }
        }
        self.sync_active_index_from_view();
        self.select_active_in_sidebar();
        self.refresh_find_matches();
        self.lifecycle_notice = Some("Closed editor pane".to_owned());
    }

    fn close_active_view(&mut self) {
        if !self.can_close_active_view() {
            self.lifecycle_notice = Some("At least one editor view must remain open".to_owned());
            return;
        }
        self.commit_active_view_to_buffer();
        let pane_id = self.pane_tree.focused_pane();
        let view_id = self.pane_tree.focused_view();
        let document_id = self.active_document().id;
        let shared_view_count = self.view_registry.views_for_document(document_id).count();
        if shared_view_count <= 1 {
            self.close_active();
            return;
        }
        match self.pane_tree.close_view(pane_id, view_id) {
            Ok(_) => {
                let _ = self.view_registry.remove_view(view_id);
                self.pane_views.retain(|view| view.id != view_id);
                let _ = self.text_layouts.remove(&view_id.stable_key());
                self.sync_active_index_from_view();
                self.select_active_in_sidebar();
                self.refresh_find_matches();
                self.lifecycle_notice = Some("Closed pane-local document view".to_owned());
            }
            Err(error) => {
                self.lifecycle_notice = Some(error.to_string());
            }
        }
    }

    fn menu_definitions(&self) -> Vec<MenuDefinition> {
        let active = self.active_document();
        let active_record = self.document_registry.get(active.id);
        let save_is_enabled = active_record
            .map(|record| record.save_requirement(self.active_view().editor.edit_revision()))
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
        let focused_pane = self.pane_tree.focused_pane();
        let focused_view = self.pane_tree.focused_view();
        let focused_leaf = self.pane_tree.leaf(focused_pane);
        let active_is_pinned = focused_leaf.is_some_and(|leaf| leaf.is_pinned(focused_view));
        let active_is_preview = focused_leaf.is_some_and(|leaf| leaf.is_preview(focused_view));

        let mut file_items = vec![
            MenuItem::command(
                MenuCommand::new("new-file", "New File", "Ctrl+N").with_mnemonic('n'),
            ),
            MenuItem::command(MenuCommand::new("open", "Open…", "Ctrl+O").with_mnemonic('o')),
            MenuItem::command(
                MenuCommand::new("open-folder", "Open Folder…", "Ctrl+Shift+O").with_mnemonic('f'),
            ),
        ];
        if self.workspace.is_some() {
            file_items.extend([
                MenuItem::command(
                    MenuCommand::new("refresh-workspace", "Refresh Workspace", "Ctrl+Shift+R")
                        .with_mnemonic('r'),
                ),
                MenuItem::command(
                    MenuCommand::new("close-workspace", "Close Workspace", "").with_mnemonic('w'),
                ),
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
            let mut recent_items = self
                .recent_files
                .entries()
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    MenuItem::command(MenuCommand::new(
                        format!("open-recent-{index}"),
                        entry.title(),
                        "",
                    ))
                })
                .collect::<Vec<_>>();
            recent_items.push(MenuItem::Separator);
            recent_items.push(MenuItem::command(
                MenuCommand::new("clear-recent-files", "Clear Recent Files", "").with_mnemonic('c'),
            ));
            file_items.push(MenuItem::submenu(
                MenuDefinition::new("recent-files", "Open Recent", recent_items).with_mnemonic('e'),
            ));
        }
        file_items.extend([
            MenuItem::Separator,
            MenuItem::command(
                MenuCommand::new("save", "Save", "Ctrl+S")
                    .with_enabled(save_is_enabled)
                    .with_mnemonic('s'),
            ),
            MenuItem::command(
                MenuCommand::new("save-as", "Save As…", "Ctrl+Shift+S").with_mnemonic('a'),
            ),
            MenuItem::command(
                MenuCommand::new("reload-from-disk", "Reload from Disk", "")
                    .with_enabled(reload_is_enabled)
                    .with_mnemonic('l'),
            ),
            MenuItem::Separator,
            MenuItem::command(
                MenuCommand::new("close-tab", "Close Tab", "Ctrl+W")
                    .with_enabled(self.can_close_active_view())
                    .with_mnemonic('t'),
            ),
            MenuItem::Separator,
            MenuItem::command(MenuCommand::new("exit", "Exit", "").with_mnemonic('x')),
        ]);

        let mut tab_items = vec![
            MenuItem::command(
                MenuCommand::new("pin-tab", "Pin Tab", "")
                    .with_enabled(!active_is_pinned)
                    .with_mnemonic('p'),
            ),
            MenuItem::command(
                MenuCommand::new("unpin-tab", "Unpin Tab", "")
                    .with_enabled(active_is_pinned)
                    .with_mnemonic('u'),
            ),
            MenuItem::command(
                MenuCommand::new("promote-preview", "Keep Preview Open", "")
                    .with_enabled(active_is_preview)
                    .with_mnemonic('k'),
            ),
        ];
        if self.pane_tree.leaves().len() > 1 {
            tab_items.extend([
                MenuItem::command(
                    MenuCommand::new("move-tab-next-pane", "Move to Next Pane", "")
                        .with_mnemonic('n'),
                ),
                MenuItem::command(
                    MenuCommand::new("move-tab-previous-pane", "Move to Previous Pane", "")
                        .with_mnemonic('v'),
                ),
            ]);
        }
        tab_items.extend([
            MenuItem::Separator,
            MenuItem::command(
                MenuCommand::new("close-tab", "Close Tab", "Ctrl+W")
                    .with_enabled(self.can_close_active_view())
                    .with_mnemonic('c'),
            ),
        ]);

        vec![
            MenuDefinition::new("file", "File", file_items).with_mnemonic('f'),
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
                        MenuCommand::new("show-completions", "Show Completions", "Ctrl+Space")
                            .with_mnemonic('m'),
                    ),
                    MenuItem::command(
                        MenuCommand::new("select-all", "Select All", "Ctrl+A")
                            .with_enabled(!self.active_view().editor.document().text().is_empty())
                            .with_mnemonic('a'),
                    ),
                ],
            )
            .with_mnemonic('e'),
            MenuDefinition::new(
                "find",
                "Find",
                vec![
                    MenuItem::command(
                        MenuCommand::new("find", "Find…", "Ctrl+F").with_mnemonic('f'),
                    ),
                    MenuItem::command(
                        MenuCommand::new("replace", "Replace…", "Ctrl+H").with_mnemonic('r'),
                    ),
                    MenuItem::Separator,
                    MenuItem::command(
                        MenuCommand::new("find-next", "Find Next", "F3")
                            .with_enabled(find_navigation_is_enabled),
                    ),
                    MenuItem::command(
                        MenuCommand::new("find-previous", "Find Previous", "Shift+F3")
                            .with_enabled(find_navigation_is_enabled),
                    ),
                    MenuItem::Separator,
                    MenuItem::command(
                        MenuCommand::new("replace-current", "Replace Current", "")
                            .with_enabled(find_navigation_is_enabled),
                    ),
                    MenuItem::command(
                        MenuCommand::new("replace-all", "Replace All", "")
                            .with_enabled(find_navigation_is_enabled),
                    ),
                ],
            )
            .with_mnemonic('n'),
            MenuDefinition::new(
                "view",
                "View",
                vec![
                    MenuItem::command(
                        MenuCommand::new("toggle-sidebar", "Show Sidebar", "Ctrl+B")
                            .with_checked(self.sidebar_is_visible)
                            .with_mnemonic('s'),
                    ),
                    MenuItem::submenu(
                        MenuDefinition::new("tabs", "Tabs", tab_items).with_mnemonic('t'),
                    ),
                    MenuItem::Separator,
                    MenuItem::command(MenuCommand::new("split-right", "Split Right", "Ctrl+\\")),
                    MenuItem::command(MenuCommand::new(
                        "split-down",
                        "Split Down",
                        "Ctrl+Shift+\\",
                    )),
                    MenuItem::command(
                        MenuCommand::new("close-pane", "Close Pane", "Ctrl+Shift+W")
                            .with_enabled(self.pane_tree.leaves().len() > 1),
                    ),
                    MenuItem::command(
                        MenuCommand::new("focus-next-pane", "Focus Next Pane", "Ctrl+Alt+Right")
                            .with_enabled(self.pane_tree.leaves().len() > 1),
                    ),
                    MenuItem::command(
                        MenuCommand::new(
                            "focus-previous-pane",
                            "Focus Previous Pane",
                            "Ctrl+Alt+Left",
                        )
                        .with_enabled(self.pane_tree.leaves().len() > 1),
                    ),
                    MenuItem::Separator,
                    MenuItem::command(
                        MenuCommand::new("theme", "Light Theme", "")
                            .with_checked(self.theme == Theme::luna_light())
                            .with_mnemonic('l'),
                    ),
                ],
            )
            .with_mnemonic('v'),
            MenuDefinition::new(
                "help",
                "Help",
                vec![MenuItem::command(
                    MenuCommand::new("about", "About Luna UI Rust", "").with_enabled(false),
                )],
            )
            .with_mnemonic('h'),
        ]
    }

    fn palette_items(&self) -> Vec<PaletteItem> {
        let mut items = Vec::new();
        for menu in self.menu_definitions() {
            Self::append_palette_items(&menu.title, &menu.items, &mut items);
        }
        items
    }

    fn append_palette_items(prefix: &str, menu_items: &[MenuItem], items: &mut Vec<PaletteItem>) {
        for item in menu_items {
            match item {
                MenuItem::Command(command) if command.is_enabled => items.push(PaletteItem::new(
                    command.id.clone(),
                    format!("{prefix}: {}", command.title),
                    command.shortcut.clone(),
                )),
                MenuItem::Submenu(submenu) => {
                    let nested_prefix = format!("{prefix}: {}", submenu.title);
                    Self::append_palette_items(&nested_prefix, &submenu.items, items);
                }
                MenuItem::Command(_) | MenuItem::Separator => {}
            }
        }
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
        let active_view = self.active_view();
        EditorShellState {
            menus: menu_definitions
                .iter()
                .map(|menu| ShellMenu::new(menu.id.clone(), menu.title.clone()))
                .collect(),
            tabs: Vec::new(),
            tab_strip_is_visible: false,
            active_menu_id: self.menu.active_menu_id.clone(),
            active_tab_id: None,
            sidebar_items,
            selected_sidebar_id: self.selected_sidebar_id.clone(),
            sidebar_is_visible: self.sidebar_is_visible,
            sidebar_width: 236,
            status_left,
            status_right: format!(
                "Pane {}  Ln {}, Col {}  UTF-8  {source_label}",
                self.pane_tree.focused_pane().value(),
                active_view.editor.caret().line_index.saturating_add(1),
                active_view.editor.caret().utf8_column.saturating_add(1)
            ),
            editor_children: vec![self.pane_surface_id.clone()],
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
            .saturating_add(usize::from(self.tab_context_menu.is_some()))
            .saturating_add(usize::from(self.palette.is_some()))
            .saturating_add(usize::from(self.completion.is_some()))
            .saturating_add(usize::from(self.find.is_some()))
    }

    fn text_viewport_size(&self, bounds: RectI) -> SizeI {
        let style = TextViewStyle::from_theme(self.theme);
        let inner = bounds.inset(style.content_insets);
        SizeI::new(
            inner
                .width
                .saturating_sub(style.gutter_width)
                .saturating_sub(style.scrollbar_width)
                .max(1),
            inner.height.max(1),
        )
    }

    fn pane_surface_state(&self) -> EditorPaneSurfaceState {
        let pane_count = self.pane_tree.leaves().len();
        let document_count = self.documents.len();
        let panes = self
            .pane_tree
            .leaves()
            .into_iter()
            .filter_map(|leaf| {
                let active_state = self.view_state(leaf.active_view())?;
                let tabs = leaf
                    .views()
                    .iter()
                    .filter_map(|view_id| {
                        let view = self.view_state(*view_id)?;
                        let document = self
                            .documents
                            .iter()
                            .find(|document| document.id == view.document_id)?;
                        Some(PaneTab {
                            view_id: *view_id,
                            title: document.title(&self.document_registry).to_owned(),
                            is_dirty: document.is_dirty(&self.document_registry),
                            is_closable: leaf.views().len() > 1
                                || pane_count > 1
                                || document_count > 1,
                            is_pinned: leaf.is_pinned(*view_id),
                            is_preview: leaf.is_preview(*view_id),
                        })
                    })
                    .collect();
                Some(PanePresentation {
                    pane_id: leaf.id(),
                    tabs,
                    active_view: leaf.active_view(),
                    tab_scroll_offset: leaf.tab_scroll_offset(),
                    editor_child: active_state.text_node_id.clone(),
                })
            })
            .collect();
        EditorPaneSurfaceState {
            tree: self.pane_tree.clone(),
            panes,
        }
    }

    fn create_pane_surface(
        &self,
        shell: &EditorShell,
    ) -> Result<EditorPaneSurface, ApplicationError> {
        Ok(EditorPaneSurface::new(
            self.pane_surface_id.clone(),
            shell.layout().editor,
            self.theme,
            self.pane_surface_state(),
            PaneLayoutMetrics::default(),
        )?)
    }

    fn text_view_from_layout(
        &self,
        view_id: DocumentViewId,
        bounds: RectI,
        layout: TextLayoutSnapshot,
    ) -> Option<TextView> {
        let view = self.view_state(view_id)?;
        let document = self
            .documents
            .iter()
            .find(|document| document.id == view.document_id)?;
        Some(TextView::new(
            view.text_node_id.clone(),
            bounds,
            view.editor.document().clone(),
            layout,
            view.editor.caret(),
            view.editor.selection(),
            view.scroll,
            TextViewStyle::from_theme(self.theme),
            format!("Editor for {}", document.title(&self.document_registry)),
            self.text_is_focused
                && self.pane_tree.focused_view() == view_id
                && self.palette.is_none()
                && self.completion.is_none()
                && self.find.is_none()
                && self.tab_context_menu.is_none()
                && !self.menu.is_open(),
            true,
        ))
    }

    fn current_text_view(&self) -> Option<TextView> {
        let view = self.active_view();
        self.text_layouts
            .get(&view.stable_key())
            .and_then(TextLayoutCache::snapshot)
            .cloned()
            .and_then(|layout| self.text_view_from_layout(view.id, self.last_editor_bounds, layout))
    }

    fn create_completion_popup(&self) -> Result<Option<CompletionPopup>, ApplicationError> {
        let Some(state) = self.completion.clone() else {
            return Ok(None);
        };
        let anchor = self
            .current_text_view()
            .and_then(|view| view.caret_bounds())
            .unwrap_or_else(|| {
                RectI::new(
                    self.last_editor_bounds.x.saturating_add(48),
                    self.last_editor_bounds.y.saturating_add(24),
                    2,
                    20,
                )
            });
        Ok(Some(CompletionPopup::new(
            self.completion_id.clone(),
            self.viewport,
            anchor,
            self.theme,
            state,
        )?))
    }

    fn update_text_layout(
        &mut self,
        view_id: DocumentViewId,
        request: TextLayoutRequest,
        scroll_y: i32,
        viewport_height: u32,
    ) -> Result<TextLayoutSnapshot, ApplicationError> {
        let Some(view) = self.view_state(view_id) else {
            return Err(std::io::Error::other("editor view missing during layout").into());
        };
        let key = view.stable_key();
        let revision = view.editor.edit_revision();
        let document = view.editor.document().clone();
        let cache = self.text_layouts.entry(key).or_default();
        Ok(cache
            .update(
                &mut self.engine,
                &document,
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
        Ok(Some(DropdownMenu::new_with_state(
            self.menu_id.clone(),
            self.viewport,
            anchor,
            self.theme,
            definition,
            &self.menu,
        )?))
    }

    fn tab_context_definition(&self, pane_id: PaneId, view_id: DocumentViewId) -> MenuDefinition {
        let leaf = self.pane_tree.leaf(pane_id);
        let is_pinned = leaf.is_some_and(|leaf| leaf.is_pinned(view_id));
        let is_preview = leaf.is_some_and(|leaf| leaf.is_preview(view_id));
        let move_items = self
            .pane_tree
            .leaves()
            .into_iter()
            .filter(|leaf| leaf.id() != pane_id)
            .map(|leaf| {
                MenuItem::command(MenuCommand::new(
                    format!("move-tab-to-pane-{}", leaf.id().value()),
                    format!("Pane {}", leaf.id().value()),
                    "",
                ))
            })
            .collect::<Vec<_>>();
        let mut items = vec![
            MenuItem::command(
                MenuCommand::new("pin-tab", "Pin Tab", "")
                    .with_enabled(!is_pinned)
                    .with_mnemonic('p'),
            ),
            MenuItem::command(
                MenuCommand::new("unpin-tab", "Unpin Tab", "")
                    .with_enabled(is_pinned)
                    .with_mnemonic('u'),
            ),
            MenuItem::command(
                MenuCommand::new("promote-preview", "Keep Preview Open", "")
                    .with_enabled(is_preview)
                    .with_mnemonic('k'),
            ),
        ];
        if !move_items.is_empty() {
            items.push(MenuItem::submenu(
                MenuDefinition::new("context-move-tab", "Move to Pane", move_items)
                    .with_mnemonic('m'),
            ));
        }
        items.extend([
            MenuItem::Separator,
            MenuItem::command(
                MenuCommand::new("close-tab", "Close", "Ctrl+W")
                    .with_enabled(self.can_close_active_view())
                    .with_mnemonic('c'),
            ),
        ]);
        MenuDefinition::new("tab-context", "Tab", items)
    }

    fn create_tab_context_menu(&self) -> Result<Option<DropdownMenu>, ApplicationError> {
        let Some(context) = self.tab_context_menu.as_ref() else {
            return Ok(None);
        };
        let definition = self.tab_context_definition(context.pane_id, context.view_id);
        Ok(Some(DropdownMenu::new_with_state(
            self.context_menu_id.clone(),
            self.viewport,
            context.anchor,
            self.theme,
            definition,
            &context.menu,
        )?))
    }

    fn open_tab_context_menu(
        &mut self,
        pane_id: PaneId,
        view_id: DocumentViewId,
        position: PointI,
    ) -> HostControl {
        self.commit_active_view_to_buffer();
        self.activate_pane_view(pane_id, view_id);
        let definition = self.tab_context_definition(pane_id, view_id);
        let mut menu = DropdownMenuState::default();
        menu.open(&definition);
        self.menu.close();
        self.palette = None;
        self.completion = None;
        self.find = None;
        self.tab_context_menu = Some(TabContextMenuState {
            pane_id,
            view_id,
            anchor: RectI::new(position.x, position.y, 1, 1),
            menu,
        });
        self.text_is_focused = false;
        HostControl::Invalidate(InvalidationClass::TextOverlay)
    }

    fn close_tab_context_menu(&mut self) -> HostControl {
        if self.tab_context_menu.take().is_none() {
            return HostControl::Continue;
        }
        self.text_is_focused = true;
        HostControl::Invalidate(InvalidationClass::TextOverlay)
    }

    fn open_menu(&mut self, menu_id: &str) -> HostControl {
        if self.menu.active_menu_id.as_deref() == Some(menu_id)
            && self.palette.is_none()
            && self.completion.is_none()
            && self.find.is_none()
            && self.tab_context_menu.is_none()
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
        self.completion = None;
        self.find = None;
        self.tab_context_menu = None;
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
        self.text_is_focused = self.palette.is_none()
            && self.completion.is_none()
            && self.find.is_none()
            && self.tab_context_menu.is_none();
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
            NamedKey::Escape => {
                if self.menu.close_submenu() {
                    HostControl::Invalidate(InvalidationClass::PaintOverlay)
                } else {
                    self.close_menu()
                }
            }
            NamedKey::ArrowDown => {
                self.menu.select_next(&definition);
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::ArrowUp => {
                self.menu.select_previous(&definition);
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::ArrowLeft => {
                if self.menu.close_submenu() {
                    HostControl::Invalidate(InvalidationClass::PaintOverlay)
                } else {
                    self.switch_menu(-1)
                }
            }
            NamedKey::ArrowRight => {
                if self.menu.open_selected_submenu(&definition) {
                    HostControl::Invalidate(InvalidationClass::PaintOverlay)
                } else {
                    self.switch_menu(1)
                }
            }
            NamedKey::Home => {
                self.menu.select_first(&definition);
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::End => {
                self.menu.select_last(&definition);
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::Enter => {
                if let Some(command) = self.menu.selected_command(&definition).map(str::to_owned) {
                    self.execute_command(&command)
                } else if self.menu.open_selected_submenu(&definition) {
                    HostControl::Invalidate(InvalidationClass::PaintOverlay)
                } else {
                    HostControl::Continue
                }
            }
            NamedKey::Tab
            | NamedKey::Backspace
            | NamedKey::Delete
            | NamedKey::PageUp
            | NamedKey::PageDown => HostControl::Continue,
        }
    }

    fn handle_tab_context_key(&mut self, key: NamedKey) -> HostControl {
        let Some(context) = self.tab_context_menu.as_ref() else {
            return HostControl::Continue;
        };
        let definition = self.tab_context_definition(context.pane_id, context.view_id);
        match key {
            NamedKey::Escape => {
                let submenu_closed = self
                    .tab_context_menu
                    .as_mut()
                    .is_some_and(|context| context.menu.close_submenu());
                if submenu_closed {
                    HostControl::Invalidate(InvalidationClass::PaintOverlay)
                } else {
                    self.close_tab_context_menu()
                }
            }
            NamedKey::ArrowDown => {
                if let Some(context) = self.tab_context_menu.as_mut() {
                    context.menu.select_next(&definition);
                }
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::ArrowUp => {
                if let Some(context) = self.tab_context_menu.as_mut() {
                    context.menu.select_previous(&definition);
                }
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::ArrowRight => {
                if let Some(context) = self.tab_context_menu.as_mut() {
                    let _ = context.menu.open_selected_submenu(&definition);
                }
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::ArrowLeft => {
                if let Some(context) = self.tab_context_menu.as_mut() {
                    let _ = context.menu.close_submenu();
                }
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::Home => {
                if let Some(context) = self.tab_context_menu.as_mut() {
                    context.menu.select_first(&definition);
                }
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::End => {
                if let Some(context) = self.tab_context_menu.as_mut() {
                    context.menu.select_last(&definition);
                }
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::Enter => {
                let command = self
                    .tab_context_menu
                    .as_ref()
                    .and_then(|context| context.menu.selected_command(&definition))
                    .map(str::to_owned);
                if let Some(command) = command {
                    self.execute_command(&command)
                } else {
                    if let Some(context) = self.tab_context_menu.as_mut() {
                        let _ = context.menu.open_selected_submenu(&definition);
                    }
                    HostControl::Invalidate(InvalidationClass::PaintOverlay)
                }
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
        self.tab_context_menu = None;
        self.completion = None;
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
        self.completion = None;
        self.tab_context_menu = None;
        self.menu.close();
        self.text_is_focused = false;
        self.refresh_find_matches();
        debug_assert_eq!(self.transient_surface_count(), 1);
        HostControl::Invalidate(InvalidationClass::TextOverlay)
    }

    fn open_completion(&mut self) -> HostControl {
        let prefix = self.active_word_prefix();
        let candidates = [
            ("struct", "struct", "Rust keyword"),
            ("enum", "enum", "Rust keyword"),
            ("impl", "impl", "Rust keyword"),
            ("match", "match", "Rust keyword"),
            ("DocumentViewId", "DocumentViewId", "Luna document view"),
            ("PaneTree", "PaneTree", "Luna pane topology"),
            ("EditorPaneSurface", "EditorPaneSurface", "Luna pane widget"),
            ("CompletionPopup", "CompletionPopup", "Luna popup widget"),
        ];
        let mut items = candidates
            .into_iter()
            .filter(|(label, _, _)| {
                prefix.is_empty()
                    || label
                        .to_ascii_lowercase()
                        .starts_with(&prefix.to_ascii_lowercase())
            })
            .map(|(label, insert_text, detail)| {
                CompletionItem::new(label, label, detail, insert_text)
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            items.push(CompletionItem::new(
                "no-suggestions",
                "No suggestions",
                "Type a different prefix",
                "",
            ));
        }
        self.menu.close();
        self.tab_context_menu = None;
        self.palette = None;
        self.find = None;
        self.completion = Some(CompletionPopupState {
            items,
            selected_index: 0,
        });
        self.text_is_focused = true;
        HostControl::Invalidate(InvalidationClass::TextOverlay)
    }

    fn active_word_prefix(&self) -> String {
        let view = self.active_view();
        let document = view.editor.document();
        let offset = document.absolute_offset(view.editor.caret(), SnapBias::Backward);
        let before = document.text().get(..offset).unwrap_or_default();
        let start = before
            .char_indices()
            .rev()
            .find(|(_, character)| !character.is_alphanumeric() && *character != '_')
            .map_or(0, |(index, character)| {
                index.saturating_add(character.len_utf8())
            });
        before.get(start..).unwrap_or_default().to_owned()
    }

    fn accept_completion(&mut self) -> HostControl {
        let item = self
            .completion
            .as_ref()
            .and_then(CompletionPopupState::selected_item)
            .cloned();
        self.completion = None;
        let Some(item) = item else {
            return HostControl::Invalidate(InvalidationClass::TextOverlay);
        };
        if item.insert_text.is_empty() {
            return HostControl::Invalidate(InvalidationClass::TextOverlay);
        }
        let prefix = self.active_word_prefix();
        if !prefix.is_empty() {
            let caret = self.active_view().editor.caret();
            let document = self.active_view().editor.document().clone();
            let caret_offset = document.absolute_offset(caret, SnapBias::Backward);
            let start_offset = caret_offset.saturating_sub(prefix.len());
            let anchor = document.location_for_offset(start_offset, SnapBias::Backward);
            self.active_view_mut()
                .editor
                .set_selection(TextRange::new(anchor, caret));
        }
        let result = self.active_view_mut().editor.insert_text(&item.insert_text);
        if result.did_change {
            self.commit_active_view_to_buffer();
            self.reveal_caret_on_next_frame = true;
            self.lifecycle_notice = Some(format!("Inserted completion {}", item.label));
            HostControl::Invalidate(InvalidationClass::TextLayout)
        } else {
            HostControl::Invalidate(InvalidationClass::TextOverlay)
        }
    }

    fn handle_completion_key(&mut self, key: NamedKey) -> HostControl {
        match key {
            NamedKey::Escape => {
                self.completion = None;
                HostControl::Invalidate(InvalidationClass::TextOverlay)
            }
            NamedKey::ArrowDown => {
                if let Some(state) = self.completion.as_mut() {
                    state.select_next();
                }
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::ArrowUp => {
                if let Some(state) = self.completion.as_mut() {
                    state.select_previous();
                }
                HostControl::Invalidate(InvalidationClass::PaintOverlay)
            }
            NamedKey::Enter | NamedKey::Tab => self.accept_completion(),
            NamedKey::Backspace
            | NamedKey::Delete
            | NamedKey::ArrowLeft
            | NamedKey::ArrowRight
            | NamedKey::Home
            | NamedKey::End
            | NamedKey::PageUp
            | NamedKey::PageDown => {
                self.completion = None;
                HostControl::Invalidate(InvalidationClass::TextOverlay)
            }
        }
    }

    fn replace_current_match(&mut self) {
        self.refresh_find_matches();
        if self.find_matches.is_empty() {
            self.lifecycle_notice = Some("No find match to replace".to_owned());
            return;
        }
        let selected = self
            .find
            .as_ref()
            .map_or(0, |find| find.selected_match.saturating_sub(1))
            .min(self.find_matches.len().saturating_sub(1));
        let range = self.find_matches[selected].clone();
        let replacement = self
            .find
            .as_ref()
            .map_or_else(String::new, |find| find.replacement.clone());
        let document = self.active_view().editor.document().clone();
        let anchor = document.location_for_offset(range.start, SnapBias::Backward);
        let focus = document.location_for_offset(range.end, SnapBias::Forward);
        self.active_view_mut()
            .editor
            .set_selection(TextRange::new(anchor, focus));
        let result = self.active_view_mut().editor.insert_text(&replacement);
        if result.did_change {
            self.commit_active_view_to_buffer();
            self.refresh_find_matches();
            self.reveal_caret_on_next_frame = true;
            self.lifecycle_notice = Some("Replaced current match".to_owned());
        }
    }

    fn replace_all_matches(&mut self) {
        self.refresh_find_matches();
        if self.find_matches.is_empty() {
            self.lifecycle_notice = Some("No find matches to replace".to_owned());
            return;
        }
        let replacement = self
            .find
            .as_ref()
            .map_or_else(String::new, |find| find.replacement.clone());
        let mut text = self.active_view().editor.document().text().to_owned();
        let count = self.find_matches.len();
        for range in self.find_matches.iter().rev() {
            text.replace_range(range.clone(), &replacement);
        }
        let end = self.active_view().editor.document().end_location();
        self.active_view_mut()
            .editor
            .set_selection(TextRange::new(TextLocation::default(), end));
        let result = self.active_view_mut().editor.insert_text(&text);
        if result.did_change {
            self.commit_active_view_to_buffer();
            self.refresh_find_matches();
            self.reveal_caret_on_next_frame = true;
            self.lifecycle_notice = Some(format!("Replaced {count} matches"));
        }
    }

    fn execute_command(&mut self, command: &str) -> HostControl {
        self.palette = None;
        self.completion = None;
        self.tab_context_menu = None;
        self.menu.close();
        if let Some(index) = command
            .strip_prefix("open-recent-")
            .and_then(|value| value.parse::<usize>().ok())
        {
            self.open_recent_file(index);
            return HostControl::Invalidate(InvalidationClass::WidgetLayout);
        }
        if let Some(value) = command
            .strip_prefix("move-tab-to-pane-")
            .and_then(|value| value.parse::<u64>().ok())
        {
            let target = self
                .pane_tree
                .leaves()
                .into_iter()
                .map(|leaf| leaf.id())
                .find(|pane_id| pane_id.value() == value);
            if let Some(target) = target {
                self.move_active_tab_to_pane(target);
                return HostControl::Invalidate(InvalidationClass::WidgetLayout);
            }
            self.lifecycle_notice = Some(format!("Pane {value} is no longer available"));
            return HostControl::Invalidate(InvalidationClass::TextOverlay);
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
            "close-tab" if self.can_close_active_view() => {
                self.close_active_view();
                InvalidationClass::WidgetLayout
            }
            "close-tab" => return HostControl::Continue,
            "pin-tab" => {
                self.toggle_active_tab_pin(true);
                InvalidationClass::WidgetLayout
            }
            "unpin-tab" => {
                self.toggle_active_tab_pin(false);
                InvalidationClass::WidgetLayout
            }
            "promote-preview" => {
                self.promote_active_preview();
                InvalidationClass::WidgetLayout
            }
            "move-tab-next-pane" => {
                self.move_active_tab_to_relative_pane(1);
                InvalidationClass::WidgetLayout
            }
            "move-tab-previous-pane" => {
                self.move_active_tab_to_relative_pane(-1);
                InvalidationClass::WidgetLayout
            }
            "split-right" => {
                self.split_focused_pane(PaneAxis::Horizontal);
                InvalidationClass::WidgetLayout
            }
            "split-down" => {
                self.split_focused_pane(PaneAxis::Vertical);
                InvalidationClass::WidgetLayout
            }
            "close-pane" if self.pane_tree.leaves().len() > 1 => {
                self.close_focused_pane();
                InvalidationClass::WidgetLayout
            }
            "close-pane" => return HostControl::Continue,
            "focus-next-pane" => {
                self.focus_relative_pane(1);
                InvalidationClass::WidgetLayout
            }
            "focus-previous-pane" => {
                self.focus_relative_pane(-1);
                InvalidationClass::WidgetLayout
            }
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
            "replace-current" => {
                self.replace_current_match();
                InvalidationClass::TextLayout
            }
            "replace-all" => {
                self.replace_all_matches();
                InvalidationClass::TextLayout
            }
            "show-completions" => return self.open_completion(),
            "select-all" => {
                let end = self.active_view().editor.document().end_location();
                self.active_view_mut()
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
        self.text_is_focused = self.palette.is_none()
            && self.completion.is_none()
            && self.find.is_none()
            && self.tab_context_menu.is_none()
            && !self.menu.is_open();
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
        self.add_document_to_focused_pane(id);
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
        self.commit_active_view_to_buffer();
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
        self.commit_active_view_to_buffer();
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
        let view_ids = self
            .view_registry
            .views_for_document(id)
            .map(|record| record.id())
            .collect::<Vec<_>>();
        for view_id in view_ids {
            let Some(pane_id) = self.pane_tree.pane_for_view(view_id) else {
                let _ = self.view_registry.remove_view(view_id);
                self.pane_views.retain(|view| view.id != view_id);
                continue;
            };
            let needs_replacement = self.pane_tree.leaves().len() == 1
                && self
                    .pane_tree
                    .leaf(pane_id)
                    .is_some_and(|leaf| leaf.views().len() == 1);
            if needs_replacement {
                let replacement_id = if let Some(document) =
                    self.documents.iter().find(|document| document.id != id)
                {
                    document.id
                } else {
                    let editor = EditableText::new(String::new());
                    let replacement_id = self
                        .document_registry
                        .create_untitled(editor.edit_revision());
                    self.documents.push(DemoDocument {
                        id: replacement_id,
                        editor,
                        scroll: TextScroll::default(),
                    });
                    replacement_id
                };
                if let Ok(replacement_view) = self.create_view_for_document(replacement_id, None) {
                    let _ = self.pane_tree.add_view(pane_id, replacement_view);
                }
            }
            let _ = self.pane_tree.close_view(pane_id, view_id);
            let _ = self.view_registry.remove_view(view_id);
            self.pane_views.retain(|view| view.id != view_id);
            let _ = self.text_layouts.remove(&view_id.stable_key());
        }
        self.documents.remove(index);
        let _ = self.document_registry.remove(id);
        self.sync_active_index_from_view();
        self.select_active_in_sidebar();
        self.refresh_find_matches();
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
            (WorkspaceNodeKind::File, WorkspaceNodeStatus::Available) => {
                self.open_path_with_mode(&path, true);
            }
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
        self.open_path_with_mode(path, false);
    }

    fn open_path_with_mode(&mut self, path: &Path, as_preview: bool) {
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
                self.add_document_to_focused_pane_with_mode(id, as_preview);
                self.lifecycle_notice = Some(if as_preview {
                    format!("Previewing {title}")
                } else {
                    format!("Opened {title}")
                });
                self.reveal_caret_on_next_frame = true;
            }
            OpenFileOutcome::AlreadyOpen(id) => {
                self.activate_document_id(id);
                self.lifecycle_notice = Some(format!("{title} is already open"));
            }
        }
    }

    fn save_active(&mut self) -> bool {
        self.commit_active_view_to_buffer();
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
        self.commit_active_view_to_buffer();
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
        self.commit_active_view_to_buffer();
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
        let storage_snapshot = loaded.snapshot();
        let editor = EditableText::new(loaded.into_text());
        let edit_revision = editor.edit_revision();
        {
            let document = self.active_document_mut();
            document.editor = editor;
            document.scroll = TextScroll::default();
        }
        self.synchronize_document_views(document_id);
        for view in self
            .pane_views
            .iter_mut()
            .filter(|view| view.document_id == document_id)
        {
            view.scroll = TextScroll::default();
        }
        let view_keys = self
            .pane_views
            .iter()
            .filter(|view| view.document_id == document_id)
            .map(PaneViewState::stable_key)
            .collect::<Vec<_>>();
        for key in view_keys {
            let _ = self.text_layouts.remove(&key);
        }
        if let Some(record) = self.document_registry.get_mut(document_id) {
            record.mark_saved(edit_revision, Some(storage_snapshot));
        }
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
        let revision = self.active_view().editor.edit_revision();
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
        self.remove_document_id(active_id);
        self.lifecycle_notice = None;
        self.reveal_caret_on_next_frame = true;
    }

    fn activate_document_id(&mut self, id: DocumentId) {
        let stable_key = id.stable_key();
        self.activate_document(&stable_key);
    }

    fn activate_document(&mut self, id: &str) {
        let Some(index) = self
            .documents
            .iter()
            .position(|document| document.stable_key() == id)
        else {
            return;
        };
        self.commit_active_view_to_buffer();
        let document_id = self.documents[index].id;
        let focused_pane = self.pane_tree.focused_pane();
        let focused_view = self.pane_tree.leaf(focused_pane).and_then(|leaf| {
            leaf.views().iter().copied().find(|view_id| {
                self.view_registry
                    .view(*view_id)
                    .is_some_and(|record| record.document_id() == document_id)
            })
        });
        if let Some(view_id) = focused_view {
            self.activate_pane_view(focused_pane, view_id);
        } else {
            let existing = self
                .view_registry
                .views_for_document(document_id)
                .find_map(|record| {
                    self.pane_tree
                        .pane_for_view(record.id())
                        .map(|pane_id| (pane_id, record.id()))
                });
            if let Some((pane_id, view_id)) = existing {
                self.activate_pane_view(pane_id, view_id);
            } else {
                self.active_index = index;
                self.add_document_to_focused_pane(document_id);
            }
        }
        self.lifecycle_notice = None;
        self.text_is_focused = self.palette.is_none()
            && self.completion.is_none()
            && self.find.is_none()
            && self.tab_context_menu.is_none()
            && !self.menu.is_open();
    }

    fn find_match_ranges(
        text: &str,
        query: &str,
        case_sensitive: bool,
        whole_word: bool,
    ) -> Vec<Range<usize>> {
        if query.is_empty() {
            return Vec::new();
        }
        let mut ranges = Vec::new();
        let mut cursor = 0;
        while cursor < text.len() {
            let Some(suffix) = text.get(cursor..) else {
                break;
            };
            let Some(character) = suffix.chars().next() else {
                break;
            };
            let start = cursor;
            let end = start.saturating_add(query.len());
            let is_match = text.get(start..end).is_some_and(|candidate| {
                if case_sensitive {
                    candidate == query
                } else {
                    candidate.eq_ignore_ascii_case(query)
                }
            });
            let has_word_boundary = if is_match && whole_word {
                let before_is_word = text
                    .get(..start)
                    .and_then(|prefix| prefix.chars().next_back())
                    .is_some_and(Self::is_find_word_character);
                let after_is_word = text
                    .get(end..)
                    .and_then(|suffix| suffix.chars().next())
                    .is_some_and(Self::is_find_word_character);
                !before_is_word && !after_is_word
            } else {
                is_match
            };
            if has_word_boundary {
                ranges.push(start..end);
                cursor = end;
            } else {
                cursor = cursor.saturating_add(character.len_utf8());
            }
        }
        ranges
    }

    fn is_find_word_character(character: char) -> bool {
        character.is_alphanumeric() || character == '_'
    }

    fn refresh_find_matches(&mut self) {
        let (query, case_sensitive, whole_word) = self.find.as_ref().map_or_else(
            || (String::new(), false, false),
            |state| (state.query.clone(), state.case_sensitive, state.whole_word),
        );
        self.find_matches = Self::find_match_ranges(
            self.active_view().editor.document().text(),
            &query,
            case_sensitive,
            whole_word,
        );
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
            .map_or(0, |state| state.selected_match.saturating_sub(1))
            .min(count.saturating_sub(1));
        let next = if delta < 0 {
            if current == 0 {
                count.saturating_sub(1)
            } else {
                current.saturating_sub(1)
            }
        } else if delta > 0 {
            current.saturating_add(1) % count
        } else {
            current
        };
        if let Some(find) = self.find.as_mut() {
            find.selected_match = next.saturating_add(1);
        }
        let range = self.find_matches[next].clone();
        let document = self.active_view().editor.document().clone();
        let anchor = document.location_for_offset(range.start, SnapBias::Backward);
        let focus = document.location_for_offset(range.end, SnapBias::Forward);
        self.active_view_mut()
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
            NamedKey::Enter => {
                if self
                    .find
                    .as_ref()
                    .is_some_and(|find| find.active_field == FindField::Replacement)
                {
                    self.replace_current_match();
                    InvalidationClass::TextLayout
                } else {
                    self.select_find_match(1);
                    InvalidationClass::TextOverlay
                }
            }
            NamedKey::ArrowDown => {
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
        let previous_caret = self.active_view().editor.caret();
        let previous_selection = self.active_view().editor.selection();
        let previous_revision = self.active_view().editor.edit_revision();
        let previous_scroll = self.active_view().scroll;
        let mut reveal = true;
        let invalidation = match key {
            NamedKey::ArrowLeft => {
                self.active_view_mut().editor.move_backward(extending);
                InvalidationClass::TextOverlay
            }
            NamedKey::ArrowRight => {
                self.active_view_mut().editor.move_forward(extending);
                InvalidationClass::TextOverlay
            }
            NamedKey::ArrowUp => {
                self.active_view_mut().editor.move_up(extending);
                InvalidationClass::TextOverlay
            }
            NamedKey::ArrowDown => {
                self.active_view_mut().editor.move_down(extending);
                InvalidationClass::TextOverlay
            }
            NamedKey::Home => {
                self.active_view_mut().editor.move_to_line_start(extending);
                InvalidationClass::TextOverlay
            }
            NamedKey::End => {
                self.active_view_mut().editor.move_to_line_end(extending);
                InvalidationClass::TextOverlay
            }
            NamedKey::Backspace => {
                let _ = self.active_view_mut().editor.delete_backward();
                InvalidationClass::TextLayout
            }
            NamedKey::Delete => {
                let _ = self.active_view_mut().editor.delete_forward();
                InvalidationClass::TextLayout
            }
            NamedKey::Enter => {
                let _ = self.active_view_mut().editor.insert_newline();
                InvalidationClass::TextLayout
            }
            NamedKey::PageUp => {
                let amount = i32::try_from(viewport_height).unwrap_or(i32::MAX);
                let current_y = self.active_view().scroll.y;
                self.active_view_mut().scroll.y = current_y.saturating_sub(amount).max(0);
                reveal = false;
                InvalidationClass::TextRaster
            }
            NamedKey::PageDown => {
                let amount = i32::try_from(viewport_height).unwrap_or(i32::MAX);
                let current_y = self.active_view().scroll.y;
                self.active_view_mut().scroll.y = current_y
                    .saturating_add(amount)
                    .min(maximum_scroll_y.max(0));
                reveal = false;
                InvalidationClass::TextRaster
            }
            NamedKey::Escape | NamedKey::Tab => return HostControl::Continue,
        };
        let changed = match invalidation {
            InvalidationClass::TextLayout => {
                self.active_view().editor.edit_revision() != previous_revision
            }
            InvalidationClass::TextRaster => self.active_view().scroll != previous_scroll,
            InvalidationClass::TextOverlay => {
                self.active_view().editor.caret() != previous_caret
                    || self.active_view().editor.selection() != previous_selection
            }
            _ => true,
        };
        if !changed {
            return HostControl::Continue;
        }
        if invalidation == InvalidationClass::TextLayout {
            self.commit_active_view_to_buffer();
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
                let Some((menu_depth, item_index)) = dropdown.menu_item_at(pointer.position) else {
                    return HostControl::Continue;
                };
                let definition = dropdown.definition().clone();
                if self
                    .menu
                    .select_hovered_path(&definition, menu_depth, item_index)
                {
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
                if let Some((menu_depth, item_index)) = dropdown.menu_item_at(pointer.position) {
                    let definition = dropdown.definition().clone();
                    let _ = self
                        .menu
                        .select_hovered_path(&definition, menu_depth, item_index);
                    return HostControl::Invalidate(InvalidationClass::PaintOverlay);
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

    fn handle_tab_context_pointer(&mut self, pointer: &PointerEvent) -> HostControl {
        let Some(menu) = self.create_tab_context_menu().ok().flatten() else {
            return HostControl::Continue;
        };
        match pointer.kind {
            PointerEventKind::Moved => {
                let Some((menu_depth, item_index)) = menu.menu_item_at(pointer.position) else {
                    return HostControl::Continue;
                };
                let definition = menu.definition().clone();
                let changed = self.tab_context_menu.as_mut().is_some_and(|context| {
                    context
                        .menu
                        .select_hovered_path(&definition, menu_depth, item_index)
                });
                if changed {
                    HostControl::Invalidate(InvalidationClass::PaintOverlay)
                } else {
                    HostControl::Continue
                }
            }
            PointerEventKind::Pressed(PointerButton::Primary) => {
                if let Some(command) = menu.command_at(pointer.position).map(str::to_owned) {
                    return self.execute_command(&command);
                }
                if let Some((menu_depth, item_index)) = menu.menu_item_at(pointer.position) {
                    let definition = menu.definition().clone();
                    if let Some(context) = self.tab_context_menu.as_mut() {
                        let _ =
                            context
                                .menu
                                .select_hovered_path(&definition, menu_depth, item_index);
                    }
                    return HostControl::Invalidate(InvalidationClass::PaintOverlay);
                }
                if menu.contains(pointer.position) {
                    HostControl::Continue
                } else {
                    self.close_tab_context_menu()
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
        let previous_caret = self.active_view().editor.caret();
        let previous_selection = self.active_view().editor.selection();
        let was_focused = self.text_is_focused;
        if extending {
            let anchor = self.drag_anchor.unwrap_or(previous_caret);
            self.active_view_mut()
                .editor
                .set_selection(TextRange::new(anchor, location));
        } else {
            self.active_view_mut().editor.set_caret(location);
            self.drag_anchor = Some(location);
        }
        self.text_is_focused = true;
        self.reveal_caret_on_next_frame = true;
        previous_caret != self.active_view().editor.caret()
            || previous_selection != self.active_view().editor.selection()
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

    fn append_pane_labels(
        &mut self,
        surface: &EditorPaneSurface,
        display_list: &mut DisplayList,
    ) -> Result<(), ApplicationError> {
        for (index, frame) in surface.layout().tabs.iter().enumerate() {
            let title = if frame.is_pinned {
                let initial = frame.title.chars().next().unwrap_or('•');
                format!("{initial}{}", if frame.is_dirty { "•" } else { "" })
            } else {
                format!(
                    "{}{}{}",
                    frame.title,
                    if frame.is_preview { " ◇" } else { "" },
                    if frame.is_dirty { " •" } else { "" }
                )
            };
            let horizontal_padding: u32 = if frame.is_pinned { 4 } else { 10 };
            let close_width: u32 = if frame.close_bounds.is_some() { 24 } else { 6 };
            self.append_label(
                display_list,
                &format!("m3-editor-pane-tab-label-{index}"),
                &title,
                RectI::new(
                    frame
                        .bounds
                        .x
                        .saturating_add(i32::try_from(horizontal_padding).unwrap_or(0)),
                    frame.bounds.y,
                    frame
                        .bounds
                        .width
                        .saturating_sub(horizontal_padding.saturating_add(close_width)),
                    frame.bounds.height,
                ),
                if frame.is_pinned {
                    TextAlignment::Center
                } else {
                    TextAlignment::Leading
                },
                13.0,
            )?;
            if let Some(close_bounds) = frame.close_bounds {
                self.append_label(
                    display_list,
                    &format!("m3-editor-pane-tab-close-{index}"),
                    "×",
                    close_bounds,
                    TextAlignment::Center,
                    12.0,
                )?;
            }
        }
        for (index, strip) in surface.layout().tab_strips.iter().enumerate() {
            if let Some(bounds) = strip.previous_bounds {
                self.append_colored_label(
                    display_list,
                    &format!("m3-editor-pane-tab-scroll-previous-{index}"),
                    "‹",
                    bounds,
                    TextAlignment::Center,
                    15.0,
                    if strip.can_scroll_previous {
                        self.theme.foreground
                    } else {
                        self.theme.muted_foreground()
                    },
                )?;
            }
            if let Some(bounds) = strip.next_bounds {
                self.append_colored_label(
                    display_list,
                    &format!("m3-editor-pane-tab-scroll-next-{index}"),
                    "›",
                    bounds,
                    TextAlignment::Center,
                    15.0,
                    if strip.can_scroll_next {
                        self.theme.foreground
                    } else {
                        self.theme.muted_foreground()
                    },
                )?;
            }
        }
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
                    "m3-editor-dropdown-{}-{}-title-{}",
                    menu.definition().id,
                    row.menu_depth,
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
            if row.has_submenu {
                self.append_colored_label(
                    display_list,
                    &format!(
                        "m3-editor-dropdown-{}-{}-submenu-{}",
                        menu.definition().id,
                        row.menu_depth,
                        row.item_index
                    ),
                    "›",
                    RectI::new(
                        i32::try_from(row.bounds.right().saturating_sub(22))
                            .unwrap_or(row.bounds.x),
                        row.bounds.y,
                        16,
                        row.bounds.height,
                    ),
                    TextAlignment::Center,
                    14.0,
                    color,
                )?;
            } else if !row.shortcut.is_empty() {
                self.append_colored_label(
                    display_list,
                    &format!(
                        "m3-editor-dropdown-{}-{}-shortcut-{}",
                        menu.definition().id,
                        row.menu_depth,
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

    fn append_completion_labels(
        &mut self,
        popup: &CompletionPopup,
        display_list: &mut DisplayList,
    ) -> Result<(), ApplicationError> {
        for (index, row) in popup.layout().rows.iter().enumerate() {
            self.append_label(
                display_list,
                &format!("m3-editor-completion-label-{index}"),
                &row.label,
                RectI::new(
                    row.bounds.x.saturating_add(10),
                    row.bounds.y,
                    row.bounds.width.saturating_sub(150),
                    row.bounds.height,
                ),
                TextAlignment::Leading,
                13.0,
            )?;
            self.append_colored_label(
                display_list,
                &format!("m3-editor-completion-detail-{index}"),
                &row.detail,
                RectI::new(
                    i32::try_from(row.bounds.right().saturating_sub(136)).unwrap_or(row.bounds.x),
                    row.bounds.y,
                    126,
                    row.bounds.height,
                ),
                TextAlignment::Trailing,
                11.0,
                self.theme.muted_foreground(),
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
        self.append_label(
            display_list,
            "m3-editor-find-case-label",
            "Aa",
            panel.layout().match_case,
            TextAlignment::Center,
            11.0,
        )?;
        self.append_label(
            display_list,
            "m3-editor-find-word-label",
            "W",
            panel.layout().whole_word,
            TextAlignment::Center,
            11.0,
        )?;
        if state.replacement_is_visible {
            self.append_label(
                display_list,
                "m3-editor-find-replace-one-label",
                "Replace",
                panel.layout().replace_one,
                TextAlignment::Center,
                11.0,
            )?;
            self.append_label(
                display_list,
                "m3-editor-find-replace-all-label",
                "All",
                panel.layout().replace_all,
                TextAlignment::Center,
                11.0,
            )?;
        }
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
        let pane_surface = self.create_pane_surface(&shell)?;
        let leaf_frames = pane_surface.layout().panes.leaves.clone();
        let reveal_focused_caret = self.reveal_caret_on_next_frame;
        let mut text_views = Vec::new();

        for leaf in leaf_frames {
            let Some(view_id) = self
                .pane_tree
                .leaf(leaf.pane_id)
                .map(|pane| pane.active_view())
            else {
                continue;
            };
            let Some(mut view_state) = self.view_state(view_id).cloned() else {
                continue;
            };
            let text_viewport = self.text_viewport_size(leaf.editor);
            let request =
                TextLayoutRequest::new(text_viewport.width, 15.0, 22.0, self.theme.foreground);
            let mut layout = self.update_text_layout(
                view_id,
                request,
                view_state.scroll.y,
                text_viewport.height,
            )?;
            if leaf.is_focused && reveal_focused_caret {
                let provisional = TextView::new(
                    view_state.text_node_id.clone(),
                    leaf.editor,
                    view_state.editor.document().clone(),
                    layout.clone(),
                    view_state.editor.caret(),
                    view_state.editor.selection(),
                    view_state.scroll,
                    TextViewStyle::from_theme(self.theme),
                    "Editor",
                    true,
                    true,
                );
                let revealed_scroll = provisional.scroll_revealing_caret();
                if revealed_scroll != view_state.scroll {
                    if let Some(view) = self.view_state_mut(view_id) {
                        view.scroll = revealed_scroll;
                    }
                    view_state.scroll = revealed_scroll;
                    layout = self.update_text_layout(
                        view_id,
                        request,
                        view_state.scroll.y,
                        text_viewport.height,
                    )?;
                }
            }
            if leaf.is_focused {
                self.last_editor_bounds = leaf.editor;
            }
            if let Some(text_view) = self.text_view_from_layout(view_id, leaf.editor, layout) {
                text_views.push(text_view);
            }
        }
        self.reveal_caret_on_next_frame = false;

        let mut display_list = DisplayList::new();
        display_list.clear(self.theme.background);
        shell.build_display_list(&mut display_list);
        pane_surface.build_display_list(&mut display_list);
        for text_view in &text_views {
            text_view.build_display_list(&mut display_list);
        }
        let mut root_children = vec![self.shell_id.clone()];
        let mut nodes = vec![
            AccessibilityNode::new(self.root_id.clone(), AccessibilityRole::Window, viewport)
                .with_label("Luna UI Rust editor demo window")
                .with_children(root_children.clone()),
        ];
        nodes.extend(shell.accessibility_nodes());
        nodes.extend(pane_surface.accessibility_nodes());
        for text_view in &text_views {
            nodes.extend(text_view.accessibility_nodes());
        }
        self.append_shell_labels(&shell, &mut display_list)?;
        self.append_pane_labels(&pane_surface, &mut display_list)?;
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
        if let Some(popup) = self.create_completion_popup()? {
            popup.build_display_list(&mut display_list);
            nodes.extend(popup.accessibility_nodes());
            root_children.push(self.completion_id.clone());
            self.append_completion_labels(&popup, &mut display_list)?;
        }
        if let Some(menu) = self.create_dropdown_menu(&shell)? {
            menu.build_display_list(&mut display_list);
            nodes.extend(menu.accessibility_nodes());
            root_children.push(self.menu_id.clone());
            self.append_dropdown_labels(&menu, &mut display_list)?;
        }
        if let Some(menu) = self.create_tab_context_menu()? {
            menu.build_display_list(&mut display_list);
            nodes.extend(menu.accessibility_nodes());
            root_children.push(self.context_menu_id.clone());
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
                if self.tab_context_menu.is_some() {
                    if let Key::Named(key) = &keyboard.key {
                        return self.handle_tab_context_key(*key);
                    }
                    if let Key::Character(value) = &keyboard.key
                        && let Some(mnemonic) = value.chars().next()
                    {
                        let context_target = self
                            .tab_context_menu
                            .as_ref()
                            .map(|context| (context.pane_id, context.view_id));
                        if let Some((pane_id, view_id)) = context_target {
                            let definition = self.tab_context_definition(pane_id, view_id);
                            let command = self.tab_context_menu.as_mut().and_then(|context| {
                                context.menu.activate_mnemonic(&definition, mnemonic)
                            });
                            if let Some(command) = command {
                                return self.execute_command(&command);
                            }
                            return HostControl::Invalidate(InvalidationClass::PaintOverlay);
                        }
                    }
                    return HostControl::Continue;
                }
                if self.menu.is_open() {
                    if let Key::Named(key) = &keyboard.key {
                        return self.handle_menu_key(*key);
                    }
                    if let Key::Character(value) = &keyboard.key
                        && let Some(mnemonic) = value.chars().next()
                    {
                        let command = self.active_menu_definition().and_then(|definition| {
                            self.menu.activate_mnemonic(&definition, mnemonic)
                        });
                        if let Some(command) = command {
                            return self.execute_command(&command);
                        }
                        return HostControl::Invalidate(InvalidationClass::PaintOverlay);
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
                if self.completion.is_some() {
                    if let Key::Named(key) = &keyboard.key {
                        match key {
                            NamedKey::Escape
                            | NamedKey::ArrowDown
                            | NamedKey::ArrowUp
                            | NamedKey::Enter
                            | NamedKey::Tab => return self.handle_completion_key(*key),
                            NamedKey::Backspace
                            | NamedKey::Delete
                            | NamedKey::ArrowLeft
                            | NamedKey::ArrowRight
                            | NamedKey::Home
                            | NamedKey::End
                            | NamedKey::PageUp
                            | NamedKey::PageDown => {
                                self.completion = None;
                            }
                        }
                    } else {
                        self.completion = None;
                    }
                }

                if keyboard.modifiers.contains(Modifiers::ALT)
                    && !keyboard.modifiers.contains(Modifiers::CONTROL)
                    && !keyboard.modifiers.contains(Modifiers::SUPER)
                    && let Key::Character(value) = &keyboard.key
                    && let Some(mnemonic) = value.chars().next()
                    && let Some(menu_id) = self
                        .menu_definitions()
                        .into_iter()
                        .find(|definition| {
                            definition.mnemonic == Some(mnemonic.to_ascii_lowercase())
                        })
                        .map(|definition| definition.id)
                {
                    return self.open_menu(&menu_id);
                }

                if keyboard.modifiers.contains(Modifiers::CONTROL)
                    && keyboard.modifiers.contains(Modifiers::ALT)
                    && let Key::Named(key) = &keyboard.key
                {
                    let command = match key {
                        NamedKey::ArrowRight | NamedKey::ArrowDown => Some("focus-next-pane"),
                        NamedKey::ArrowLeft | NamedKey::ArrowUp => Some("focus-previous-pane"),
                        _ => None,
                    };
                    if let Some(command) = command {
                        return self.execute_command(command);
                    }
                }

                let command_modified = keyboard.modifiers.contains(Modifiers::SUPER)
                    || (keyboard.modifiers.contains(Modifiers::CONTROL)
                        && !keyboard.modifiers.contains(Modifiers::ALT));
                if command_modified {
                    let command = match &keyboard.key {
                        Key::Character(value) if value == " " => Some("show-completions"),
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
                        Key::Character(value)
                            if value.eq_ignore_ascii_case("w")
                                && keyboard.modifiers.contains(Modifiers::SHIFT) =>
                        {
                            Some("close-pane")
                        }
                        Key::Character(value) if value.eq_ignore_ascii_case("w") => {
                            Some("close-tab")
                        }
                        Key::Character(value)
                            if (value == "\\" || value == "|")
                                && keyboard.modifiers.contains(Modifiers::SHIFT) =>
                        {
                            Some("split-down")
                        }
                        Key::Character(value) if value == "\\" => Some("split-right"),
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
                    let result = self.active_view_mut().editor.insert_text(text);
                    if result.did_change {
                        self.commit_active_view_to_buffer();
                        self.lifecycle_notice = None;
                        self.reveal_caret_on_next_frame = true;
                        return HostControl::Invalidate(InvalidationClass::TextLayout);
                    }
                }
            }
            InputEvent::Text(text) => {
                if self.menu.is_open() || self.tab_context_menu.is_some() {
                    return HostControl::Continue;
                }
                if self.completion.is_some() {
                    self.completion = None;
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
                    let result = self.active_view_mut().editor.insert_text(&text);
                    if !result.did_change {
                        return HostControl::Continue;
                    }
                    self.commit_active_view_to_buffer();
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
                if self.tab_context_menu.is_some() {
                    return self.handle_tab_context_pointer(&pointer);
                }

                if pointer.kind == PointerEventKind::Pressed(PointerButton::Secondary) {
                    if let Ok(shell) = self.create_shell()
                        && matches!(
                            shell.semantic_hit_test(pointer.position),
                            Some(EditorShellHit::Editor)
                        )
                        && let Ok(surface) = self.create_pane_surface(&shell)
                    {
                        match surface.semantic_hit_test(pointer.position) {
                            Some(EditorPaneSurfaceHit::Tab { pane_id, view_id })
                            | Some(EditorPaneSurfaceHit::CloseTab { pane_id, view_id }) => {
                                return self.open_tab_context_menu(
                                    pane_id,
                                    view_id,
                                    pointer.position,
                                );
                            }
                            Some(
                                EditorPaneSurfaceHit::ScrollTabs { .. }
                                | EditorPaneSurfaceHit::Editor(_)
                                | EditorPaneSurfaceHit::Splitter(_),
                            )
                            | None => {}
                        }
                    }
                    self.completion = None;
                    return HostControl::Invalidate(InvalidationClass::TextOverlay);
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
                    if let Some(popup) = self.create_completion_popup().ok().flatten() {
                        if let Some(item_id) = popup.item_at(pointer.position).map(str::to_owned) {
                            if let Some(state) = self.completion.as_mut()
                                && let Some(index) =
                                    state.items.iter().position(|item| item.id == item_id)
                            {
                                state.selected_index = index;
                            }
                            return self.accept_completion();
                        }
                        if popup.layout().panel.contains(pointer.position) {
                            return HostControl::Continue;
                        }
                        self.completion = None;
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
                            if layout.match_case.contains(pointer.position) {
                                if let Some(find) = self.find.as_mut() {
                                    find.case_sensitive = !find.case_sensitive;
                                }
                                self.refresh_find_matches();
                                return HostControl::Invalidate(InvalidationClass::TextOverlay);
                            }
                            if layout.whole_word.contains(pointer.position) {
                                if let Some(find) = self.find.as_mut() {
                                    find.whole_word = !find.whole_word;
                                }
                                self.refresh_find_matches();
                                return HostControl::Invalidate(InvalidationClass::TextOverlay);
                            }
                            if layout.replace_one.contains(pointer.position) {
                                self.replace_current_match();
                                return HostControl::Invalidate(InvalidationClass::TextLayout);
                            }
                            if layout.replace_all.contains(pointer.position) {
                                self.replace_all_matches();
                                return HostControl::Invalidate(InvalidationClass::TextLayout);
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
                            Some(EditorShellHit::SidebarItem(id)) => {
                                self.handle_sidebar_activation(&id);
                                return HostControl::Invalidate(InvalidationClass::WidgetLayout);
                            }
                            Some(EditorShellHit::Menu(_))
                            | Some(EditorShellHit::Tab(_))
                            | Some(EditorShellHit::CloseTab(_)) => {}
                            Some(EditorShellHit::Editor) => {
                                if let Ok(surface) = self.create_pane_surface(&shell) {
                                    match surface.semantic_hit_test(pointer.position) {
                                        Some(EditorPaneSurfaceHit::Tab { pane_id, view_id }) => {
                                            self.commit_active_view_to_buffer();
                                            self.activate_pane_view(pane_id, view_id);
                                            self.dragged_tab = Some(DraggedPaneTab {
                                                pane_id,
                                                view_id,
                                                origin: pointer.position,
                                            });
                                            self.drag_anchor = None;
                                            return HostControl::Invalidate(
                                                InvalidationClass::WidgetLayout,
                                            );
                                        }
                                        Some(EditorPaneSurfaceHit::CloseTab {
                                            pane_id,
                                            view_id,
                                        }) => {
                                            self.commit_active_view_to_buffer();
                                            self.activate_pane_view(pane_id, view_id);
                                            self.close_active_view();
                                            return HostControl::Invalidate(
                                                InvalidationClass::WidgetLayout,
                                            );
                                        }
                                        Some(EditorPaneSurfaceHit::ScrollTabs {
                                            pane_id,
                                            direction,
                                        }) => {
                                            if let Some(strip) = surface
                                                .layout()
                                                .tab_strips
                                                .iter()
                                                .find(|strip| strip.pane_id == pane_id)
                                            {
                                                self.scroll_pane_tabs(
                                                    pane_id,
                                                    direction,
                                                    strip.visible_regular_count,
                                                    strip.effective_offset,
                                                );
                                                return HostControl::Invalidate(
                                                    InvalidationClass::WidgetLayout,
                                                );
                                            }
                                        }
                                        Some(EditorPaneSurfaceHit::Editor(pane_id)) => {
                                            self.commit_active_view_to_buffer();
                                            if self.pane_tree.focus(pane_id).is_ok() {
                                                self.sync_active_index_from_view();
                                                if let Some(frame) = surface
                                                    .layout()
                                                    .panes
                                                    .leaves
                                                    .iter()
                                                    .find(|frame| frame.pane_id == pane_id)
                                                {
                                                    self.last_editor_bounds = frame.editor;
                                                }
                                                if let Some(view) = self.current_text_view()
                                                    && view.vertical_scrollbar_contains(
                                                        pointer.position,
                                                    )
                                                {
                                                    let scroll_y = view
                                                        .scroll_y_for_scrollbar_point(
                                                            pointer.position,
                                                        );
                                                    let active_view = self.pane_tree.focused_view();
                                                    if let Some(state) =
                                                        self.view_state_mut(active_view)
                                                    {
                                                        state.scroll.y = scroll_y;
                                                    }
                                                    self.dragged_scrollbar = Some(active_view);
                                                    self.drag_anchor = None;
                                                    self.reveal_caret_on_next_frame = false;
                                                    return HostControl::Invalidate(
                                                        InvalidationClass::TextRaster,
                                                    );
                                                }
                                                let extending =
                                                    pointer.modifiers.contains(Modifiers::SHIFT);
                                                if self.apply_pointer_to_text(
                                                    pointer.position,
                                                    extending,
                                                ) {
                                                    return HostControl::Invalidate(
                                                        InvalidationClass::TextOverlay,
                                                    );
                                                }
                                                return HostControl::Invalidate(
                                                    InvalidationClass::WidgetLayout,
                                                );
                                            }
                                        }
                                        Some(EditorPaneSurfaceHit::Splitter(split_id)) => {
                                            self.dragged_splitter = Some(split_id);
                                            self.drag_anchor = None;
                                            self.text_is_focused = false;
                                            return HostControl::Invalidate(
                                                InvalidationClass::PaintOverlay,
                                            );
                                        }
                                        None => {}
                                    }
                                }
                            }
                            None => {}
                        }
                    }
                    return HostControl::Continue;
                }

                if pointer.kind == PointerEventKind::Moved
                    && let Some(view_id) = self.dragged_scrollbar
                    && self.pane_tree.focused_view() == view_id
                    && let Some(view) = self.current_text_view()
                {
                    let scroll_y = view.scroll_y_for_scrollbar_point(pointer.position);
                    if let Some(state) = self.view_state_mut(view_id) {
                        state.scroll.y = scroll_y;
                    }
                    self.reveal_caret_on_next_frame = false;
                    return HostControl::Invalidate(InvalidationClass::TextRaster);
                }
                if pointer.kind == PointerEventKind::Moved
                    && let Some(split_id) = self.dragged_splitter
                    && let Ok(shell) = self.create_shell()
                    && let Ok(surface) = self.create_pane_surface(&shell)
                    && let Some(splitter) = surface
                        .layout()
                        .panes
                        .splitters
                        .iter()
                        .find(|splitter| splitter.split_id == split_id)
                        .copied()
                {
                    let _ = self.pane_tree.set_split_ratio_from_point(
                        split_id,
                        splitter.container,
                        pointer.position,
                    );
                    return HostControl::Invalidate(InvalidationClass::WidgetLayout);
                }
                if pointer.kind == PointerEventKind::Moved
                    && self.drag_anchor.is_some()
                    && self.apply_pointer_to_text(pointer.position, true)
                {
                    return HostControl::Invalidate(InvalidationClass::TextOverlay);
                }
                if pointer.kind == PointerEventKind::Released(PointerButton::Primary) {
                    if let Some(dragged) = self.dragged_tab.take()
                        && let Ok(shell) = self.create_shell()
                        && let Ok(surface) = self.create_pane_surface(&shell)
                        && let Some((target_pane, target_index)) =
                            surface.tab_drop_target(pointer.position)
                    {
                        let moved_far_enough = pointer.position.x.abs_diff(dragged.origin.x) >= 4
                            || pointer.position.y.abs_diff(dragged.origin.y) >= 4;
                        if moved_far_enough {
                            self.commit_active_view_to_buffer();
                            match self.pane_tree.move_view(
                                dragged.pane_id,
                                target_pane,
                                dragged.view_id,
                                target_index,
                            ) {
                                Ok(pane_id) => {
                                    self.activate_pane_view(pane_id, dragged.view_id);
                                    self.lifecycle_notice = Some(if pane_id == dragged.pane_id {
                                        "Reordered tab".to_owned()
                                    } else {
                                        format!("Moved tab to pane {}", pane_id.value())
                                    });
                                }
                                Err(error) => {
                                    self.lifecycle_notice = Some(error.to_string());
                                }
                            }
                        }
                    }
                    self.drag_anchor = None;
                    self.dragged_splitter = None;
                    self.dragged_scrollbar = None;
                    self.text_is_focused = true;
                    return HostControl::Invalidate(InvalidationClass::WidgetLayout);
                }
                if pointer.kind == PointerEventKind::Left {
                    self.drag_anchor = None;
                    self.dragged_splitter = None;
                    self.dragged_tab = None;
                    self.dragged_scrollbar = None;
                    self.text_is_focused = true;
                }
            }
            InputEvent::Scroll(scroll)
                if self.palette.is_none()
                    && self.completion.is_none()
                    && self.find.is_none()
                    && self.tab_context_menu.is_none()
                    && !self.menu.is_open() =>
            {
                if let Some(view) = self.current_text_view() {
                    let maximum = view.maximum_scroll();
                    let (delta_x, delta_y) =
                        if scroll.modifiers.contains(Modifiers::SHIFT) && scroll.delta_x == 0 {
                            (scroll.delta_y, 0)
                        } else {
                            (scroll.delta_x, scroll.delta_y)
                        };
                    let previous = self.active_view().scroll;
                    let changed = {
                        let view = self.active_view_mut();
                        view.scroll
                            .scroll_by(delta_x, delta_y, maximum.x, maximum.y);
                        view.scroll != previous
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
                    && self.completion.is_none()
                    && self.find.is_none()
                    && self.tab_context_menu.is_none()
                    && !self.menu.is_open() =>
            {
                self.text_is_focused = true;
                return HostControl::Invalidate(InvalidationClass::PaintOverlay);
            }
            InputEvent::FocusLost
                if self.text_is_focused
                    || self.drag_anchor.is_some()
                    || self.dragged_splitter.is_some()
                    || self.dragged_tab.is_some()
                    || self.dragged_scrollbar.is_some()
                    || self.completion.is_some()
                    || self.tab_context_menu.is_some()
                    || self.menu.is_open() =>
            {
                self.text_is_focused = false;
                self.drag_anchor = None;
                self.dragged_splitter = None;
                self.dragged_tab = None;
                self.dragged_scrollbar = None;
                self.completion = None;
                self.tab_context_menu = None;
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
            && let Some(command) = menu.command_for_node(&target).map(str::to_owned)
        {
            return self.execute_command(&command);
        }

        if self.tab_context_menu.is_some()
            && let Ok(Some(menu)) = self.create_tab_context_menu()
            && request.kind == AccessibilityActionKind::Click
            && let Some(command) = menu.command_for_node(&target).map(str::to_owned)
        {
            return self.execute_command(&command);
        }

        if let Some(state) = self.palette.clone()
            && let Ok(palette) =
                CommandPalette::new(self.palette_id.clone(), self.viewport, self.theme, state)
        {
            if &target == palette.input_node_id() {
                self.text_is_focused = false;
                return HostControl::Invalidate(InvalidationClass::Accessibility);
            }
            if request.kind == AccessibilityActionKind::Click
                && let Some(command) = palette.command_for_node(&target).map(str::to_owned)
            {
                return self.execute_command(&command);
            }
        }

        if let Ok(Some(popup)) = self.create_completion_popup()
            && request.kind == AccessibilityActionKind::Click
            && let Some(item_id) = popup.item_for_node(&target).map(str::to_owned)
        {
            if let Some(state) = self.completion.as_mut()
                && let Some(index) = state.items.iter().position(|item| item.id == item_id)
            {
                state.selected_index = index;
            }
            return self.accept_completion();
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
                if &target == panel.match_case_node_id() {
                    if let Some(find) = self.find.as_mut() {
                        find.case_sensitive = !find.case_sensitive;
                    }
                    self.refresh_find_matches();
                    return HostControl::Invalidate(InvalidationClass::TextOverlay);
                }
                if &target == panel.whole_word_node_id() {
                    if let Some(find) = self.find.as_mut() {
                        find.whole_word = !find.whole_word;
                    }
                    self.refresh_find_matches();
                    return HostControl::Invalidate(InvalidationClass::TextOverlay);
                }
                if &target == panel.replace_one_node_id() {
                    self.replace_current_match();
                    return HostControl::Invalidate(InvalidationClass::TextLayout);
                }
                if &target == panel.replace_all_node_id() {
                    self.replace_all_matches();
                    return HostControl::Invalidate(InvalidationClass::TextLayout);
                }
                if &target == panel.close_node_id() {
                    self.find = None;
                    self.text_is_focused = true;
                    return HostControl::Invalidate(InvalidationClass::Accessibility);
                }
            }
        }

        if let Ok(shell) = self.create_shell()
            && let Ok(surface) = self.create_pane_surface(&shell)
        {
            match surface.semantic_target(&target) {
                Some(EditorPaneSurfaceHit::Tab { pane_id, view_id }) => {
                    self.activate_pane_view(pane_id, view_id);
                    return HostControl::Invalidate(InvalidationClass::Accessibility);
                }
                Some(EditorPaneSurfaceHit::Editor(pane_id)) => {
                    self.commit_active_view_to_buffer();
                    if self.pane_tree.focus(pane_id).is_ok() {
                        self.sync_active_index_from_view();
                        self.menu.close();
                        self.tab_context_menu = None;
                        self.palette = None;
                        self.completion = None;
                        self.find = None;
                        self.text_is_focused = true;
                        self.select_active_in_sidebar();
                        self.refresh_find_matches();
                        return HostControl::Invalidate(InvalidationClass::Accessibility);
                    }
                }
                Some(EditorPaneSurfaceHit::Splitter(split_id)) => {
                    self.lifecycle_notice = Some(format!(
                        "Pane splitter {}. Drag with the pointer to resize.",
                        split_id.value()
                    ));
                    return HostControl::Invalidate(InvalidationClass::Accessibility);
                }
                Some(EditorPaneSurfaceHit::ScrollTabs { pane_id, direction })
                    if request.kind == AccessibilityActionKind::Click =>
                {
                    if let Some(strip) = surface
                        .layout()
                        .tab_strips
                        .iter()
                        .find(|strip| strip.pane_id == pane_id)
                    {
                        self.scroll_pane_tabs(
                            pane_id,
                            direction,
                            strip.visible_regular_count,
                            strip.effective_offset,
                        );
                        return HostControl::Invalidate(InvalidationClass::Accessibility);
                    }
                }
                Some(EditorPaneSurfaceHit::CloseTab { pane_id, view_id })
                    if request.kind == AccessibilityActionKind::Click =>
                {
                    self.activate_pane_view(pane_id, view_id);
                    self.close_active_view();
                    return HostControl::Invalidate(InvalidationClass::Accessibility);
                }
                Some(
                    EditorPaneSurfaceHit::CloseTab { .. } | EditorPaneSurfaceHit::ScrollTabs { .. },
                )
                | None => {}
            }
        }

        if let Some(view) = self
            .pane_views
            .iter()
            .find(|view| view.text_node_id == target)
            .cloned()
            && let Some(pane_id) = self.pane_tree.pane_for_view(view.id)
        {
            self.activate_pane_view(pane_id, view.id);
            self.menu.close();
            self.tab_context_menu = None;
            self.palette = None;
            self.completion = None;
            self.find = None;
            self.text_is_focused = true;
            return HostControl::Invalidate(InvalidationClass::Accessibility);
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
                    self.close_active_view();
                    return HostControl::Invalidate(InvalidationClass::Accessibility);
                }
                Some(EditorShellHit::Editor) => {
                    self.menu.close();
                    self.tab_context_menu = None;
                    self.palette = None;
                    self.completion = None;
                    self.find = None;
                    self.text_is_focused = true;
                    return HostControl::Invalidate(InvalidationClass::Accessibility);
                }
                Some(EditorShellHit::CloseTab(_) | EditorShellHit::Menu(_)) | None => {}
            }
        }

        HostControl::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::EditorDemoApplication;
    use luna_core::{PointI, RectI};
    use luna_document_services::{
        DirtyCloseChoice, MemoryTextFileService, SaveConflictChoice, ScriptedDialogService,
        TextFileService, WorkspaceDeleteChoice, WorkspaceDirtyDeleteChoice,
    };
    use luna_documents::{DocumentRecord, DocumentSource, ExternalState};
    use luna_host_winit::{AccessibilityActionKind, AccessibilityActionRequest, NativeApplication};
    use luna_input::{InputEvent, Modifiers, PointerButton, PointerEvent, PointerEventKind};
    use luna_panes::PaneAxis;
    use luna_session::{MemorySessionStore, SessionRecentFile, SessionState, SessionWorkspace};
    use luna_text::{TextLocation, TextScroll};
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
            application.active_view().editor.document().text(),
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
            .active_view_mut()
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
        let result = application.active_view_mut().editor.insert_text("changed");
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
        let result = application.active_view_mut().editor.insert_text("changed");
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
            .active_view_mut()
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
            .active_view_mut()
            .editor
            .insert_text(" editor change");
        assert!(result.did_change);
        files.insert_utf8(&path, "external change")?;

        assert!(application.save_active());

        assert_eq!(
            application.active_view().editor.document().text(),
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
        let result = application.active_view_mut().editor.insert_text("editor ");
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
        let result = application.active_view_mut().editor.insert_text("editor ");
        assert!(result.did_change);
        files.insert_utf8(&path, "external change")?;

        assert!(!application.save_active());

        assert_eq!(files.bytes(&path)?, Some(b"external change".to_vec()));
        assert_eq!(
            application.active_view().editor.document().text(),
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

        let menus = application.menu_definitions();
        let file_menu = menus
            .iter()
            .find(|menu| menu.id == "file")
            .ok_or_else(|| std::io::Error::other("File menu missing"))?;
        let recent_menu = file_menu
            .items
            .iter()
            .find_map(|item| {
                item.as_submenu()
                    .filter(|submenu| submenu.id == "recent-files")
            })
            .ok_or_else(|| std::io::Error::other("Open Recent submenu missing"))?;
        assert!(recent_menu.items.iter().any(|item| {
            item.as_command()
                .is_some_and(|command| command.id == "open-recent-0")
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
            application.active_view().editor.document().text(),
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
        let result = application.active_view_mut().editor.insert_text("changed");
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
            .active_view_mut()
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
        let result = application.active_view_mut().editor.insert_text(" after");
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
        let _ = application.active_view_mut().editor.insert_text(" changed");
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
        let _ = application.active_view_mut().editor.insert_text(" changed");
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

    #[test]
    fn split_views_share_text_but_preserve_local_caret_and_scroll() -> TestResult {
        let (files, dialogs) = test_services()?;
        let mut application = test_application(&files, &dialogs)?;
        let source_view = application.pane_tree.focused_view();
        application
            .active_view_mut()
            .editor
            .set_caret(TextLocation::new(0, 4));
        application.active_view_mut().scroll = TextScroll::new(0, 11);

        application.split_focused_pane(PaneAxis::Horizontal);

        let sibling_view = application.pane_tree.focused_view();
        assert_ne!(source_view, sibling_view);
        assert_eq!(application.pane_tree.leaves().len(), 2);
        assert_eq!(
            application
                .view_state(source_view)
                .map(|view| view.document_id),
            application
                .view_state(sibling_view)
                .map(|view| view.document_id)
        );
        application
            .active_view_mut()
            .editor
            .set_caret(TextLocation::new(1, 3));
        application.active_view_mut().scroll = TextScroll::new(0, 44);
        let result = application.active_view_mut().editor.insert_text("shared ");
        assert!(result.did_change);
        application.commit_active_view_to_buffer();

        let source = application
            .view_state(source_view)
            .ok_or_else(|| std::io::Error::other("source pane view missing"))?;
        let sibling = application
            .view_state(sibling_view)
            .ok_or_else(|| std::io::Error::other("sibling pane view missing"))?;
        assert_eq!(
            source.editor.document().text(),
            sibling.editor.document().text()
        );
        assert_eq!(
            source.editor.edit_revision(),
            sibling.editor.edit_revision()
        );
        assert_eq!(source.editor.caret(), TextLocation::new(0, 4));
        assert_eq!(source.scroll, TextScroll::new(0, 11));
        assert_eq!(sibling.scroll, TextScroll::new(0, 44));
        Ok(())
    }

    #[test]
    fn recursive_splits_and_focus_traversal_are_pane_local() -> TestResult {
        let (files, dialogs) = test_services()?;
        let mut application = test_application(&files, &dialogs)?;
        application.split_focused_pane(PaneAxis::Horizontal);
        application.split_focused_pane(PaneAxis::Vertical);

        assert_eq!(application.pane_tree.leaves().len(), 3);
        assert_eq!(application.pane_tree.splits().len(), 2);
        let focused = application.pane_tree.focused_pane();
        application.focus_relative_pane(1);
        assert_ne!(application.pane_tree.focused_pane(), focused);
        application.focus_relative_pane(-1);
        assert_eq!(application.pane_tree.focused_pane(), focused);
        Ok(())
    }

    #[test]
    fn closing_shared_view_keeps_document_and_collapses_empty_pane() -> TestResult {
        let (files, dialogs) = test_services()?;
        let mut application = test_application(&files, &dialogs)?;
        let document_id = application.active_document().id;
        let document_count = application.documents.len();
        application.split_focused_pane(PaneAxis::Horizontal);

        application.close_active_view();

        assert_eq!(application.documents.len(), document_count);
        assert!(application.document_registry.get(document_id).is_some());
        assert_eq!(application.pane_tree.leaves().len(), 1);
        assert_eq!(
            application
                .view_registry
                .views_for_document(document_id)
                .count(),
            1
        );
        Ok(())
    }

    #[test]
    fn closing_pane_rehomes_documents_that_only_existed_there() -> TestResult {
        let (files, dialogs) = test_services()?;
        let mut application = test_application(&files, &dialogs)?;
        application.split_focused_pane(PaneAxis::Horizontal);
        application.new_document();
        let untitled_id = application.active_document().id;
        let document_count = application.documents.len();

        application.close_focused_pane();

        assert_eq!(application.pane_tree.leaves().len(), 1);
        assert_eq!(application.documents.len(), document_count);
        assert!(application.document_registry.get(untitled_id).is_some());
        assert_eq!(
            application
                .view_registry
                .views_for_document(untitled_id)
                .count(),
            1
        );
        Ok(())
    }

    #[test]
    fn pane_tabs_and_close_buttons_route_accessibility_actions() -> TestResult {
        let (files, dialogs) = test_services()?;
        let mut application = test_application(&files, &dialogs)?;
        application.viewport = RectI::new(0, 0, 1_180, 760);
        let first_pane = application.pane_tree.focused_pane();
        application.split_focused_pane(PaneAxis::Horizontal);
        let second_view = application.pane_tree.focused_view();
        let shell = application.create_shell()?;
        let surface = application.create_pane_surface(&shell)?;
        let tab = surface
            .layout()
            .tabs
            .iter()
            .find(|tab| tab.view_id == second_view)
            .ok_or_else(|| std::io::Error::other("second pane tab missing"))?;
        let tab_node = tab.node_id.clone();
        let close_node = tab
            .close_node_id
            .clone()
            .ok_or_else(|| std::io::Error::other("pane close node missing"))?;
        application.pane_tree.focus(first_pane)?;

        let _ = application.handle_accessibility_action(AccessibilityActionRequest {
            target: Some(tab_node),
            kind: AccessibilityActionKind::Click,
        });
        assert_eq!(application.pane_tree.focused_view(), second_view);

        let _ = application.handle_accessibility_action(AccessibilityActionRequest {
            target: Some(close_node),
            kind: AccessibilityActionKind::Click,
        });
        assert_eq!(application.pane_tree.leaves().len(), 1);
        Ok(())
    }

    #[test]
    fn clean_workspace_previews_replace_and_dirty_previews_promote() -> TestResult {
        let (files, dialogs) = test_services()?;
        let first_path = Path::new("/luna-editor-tests/preview-one.rs");
        let second_path = Path::new("/luna-editor-tests/preview-two.rs");
        files.insert_utf8(first_path, "fn one() {}\n")?;
        files.insert_utf8(second_path, "fn two() {}\n")?;
        let mut application = test_application(&files, &dialogs)?;

        application.open_path_with_mode(first_path, true);
        let first_view = application.pane_tree.focused_view();
        let first_document = application.active_document().id;
        assert!(
            application
                .pane_tree
                .leaf(application.pane_tree.focused_pane())
                .is_some_and(|leaf| leaf.is_preview(first_view))
        );

        application.open_path_with_mode(second_path, true);
        let second_view = application.pane_tree.focused_view();
        assert_ne!(first_view, second_view);
        assert!(application.document_registry.get(first_document).is_none());
        assert!(
            application
                .pane_tree
                .leaf(application.pane_tree.focused_pane())
                .is_some_and(|leaf| leaf.is_preview(second_view))
        );

        let edit = application
            .active_view_mut()
            .editor
            .insert_text("// retained\n");
        assert!(edit.did_change);
        application.commit_active_view_to_buffer();
        assert!(
            application
                .pane_tree
                .leaf(application.pane_tree.focused_pane())
                .is_some_and(|leaf| !leaf.is_preview(second_view))
        );
        Ok(())
    }

    #[test]
    fn pinning_and_cross_pane_moves_update_pane_local_tab_state() -> TestResult {
        let (files, dialogs) = test_services()?;
        let mut application = test_application(&files, &dialogs)?;
        application.new_document();
        let pinned_view = application.pane_tree.focused_view();
        let first_pane = application.pane_tree.focused_pane();

        application.toggle_active_tab_pin(true);
        assert!(application.pane_tree.leaf(first_pane).is_some_and(|leaf| {
            leaf.is_pinned(pinned_view) && leaf.views().first() == Some(&pinned_view)
        }));

        application.split_focused_pane(PaneAxis::Horizontal);
        let second_pane = application.pane_tree.focused_pane();
        application.new_document();
        let moved_view = application.pane_tree.focused_view();
        application.move_active_tab_to_pane(first_pane);

        assert_eq!(
            application.pane_tree.pane_for_view(moved_view),
            Some(first_pane)
        );
        assert_eq!(application.pane_tree.focused_pane(), first_pane);
        assert!(application.pane_tree.leaf(second_pane).is_some());

        application.activate_pane_view(first_pane, pinned_view);
        application.toggle_active_tab_pin(false);
        assert!(
            application
                .pane_tree
                .leaf(first_pane)
                .is_some_and(|leaf| !leaf.is_pinned(pinned_view))
        );
        Ok(())
    }

    #[test]
    fn menus_project_recent_tabs_and_context_move_submenus() -> TestResult {
        let (files, dialogs) = test_services()?;
        let recent_path = Path::new("/luna-editor-tests/recent.rs");
        files.insert_utf8(recent_path, "fn recent() {}\n")?;
        let identity = files.identity_for_save(recent_path)?;
        let mut application = test_application(&files, &dialogs)?;
        application.recent_files.record(identity, "recent.rs");
        application.split_focused_pane(PaneAxis::Horizontal);

        let menus = application.menu_definitions();
        let file = menus
            .iter()
            .find(|menu| menu.id == "file")
            .ok_or_else(|| std::io::Error::other("File menu missing"))?;
        assert!(file.items.iter().any(|item| {
            item.as_submenu()
                .is_some_and(|submenu| submenu.id == "recent-files")
        }));
        let view = menus
            .iter()
            .find(|menu| menu.id == "view")
            .ok_or_else(|| std::io::Error::other("View menu missing"))?;
        let tabs = view
            .items
            .iter()
            .find_map(|item| item.as_submenu().filter(|submenu| submenu.id == "tabs"))
            .ok_or_else(|| std::io::Error::other("Tabs submenu missing"))?;
        assert!(tabs.items.iter().any(|item| {
            item.as_command()
                .is_some_and(|command| command.id == "move-tab-next-pane")
        }));

        let context = application.tab_context_definition(
            application.pane_tree.focused_pane(),
            application.pane_tree.focused_view(),
        );
        assert!(context.items.iter().any(|item| {
            item.as_submenu()
                .is_some_and(|submenu| submenu.id == "context-move-tab")
        }));
        assert!(
            application
                .palette_items()
                .iter()
                .any(|item| item.id == "open-recent-0")
        );
        Ok(())
    }

    #[test]
    fn completion_replaces_the_active_prefix() -> TestResult {
        let (files, dialogs) = test_services()?;
        let mut application = test_application(&files, &dialogs)?;
        application.new_document();
        let edit = application.active_view_mut().editor.insert_text("Pan");
        assert!(edit.did_change);
        application.commit_active_view_to_buffer();

        let _ = application.open_completion();
        assert_eq!(
            application
                .completion
                .as_ref()
                .and_then(|state| state.selected_item())
                .map(|item| item.id.as_str()),
            Some("PaneTree")
        );
        let _ = application.accept_completion();

        assert_eq!(
            application.active_view().editor.document().text(),
            "PaneTree"
        );
        assert!(application.completion.is_none());
        Ok(())
    }

    #[test]
    fn literal_find_ranges_do_not_overlap() {
        assert_eq!(
            EditorDemoApplication::find_match_ranges("aaaa", "aa", true, false),
            vec![0..2, 2..4]
        );
    }

    #[test]
    fn whole_word_case_insensitive_replace_all_skips_identifier_substrings() -> TestResult {
        let ranges = EditorDemoApplication::find_match_ranges(
            "Alpha alpha alphabet alpha_ alpha",
            "alpha",
            false,
            true,
        );
        assert_eq!(ranges.len(), 3);
        assert_eq!(
            EditorDemoApplication::find_match_ranges(
                "Alpha alpha alphabet alpha_ alpha",
                "alpha",
                true,
                true,
            )
            .len(),
            2
        );

        let (files, dialogs) = test_services()?;
        let mut application = test_application(&files, &dialogs)?;
        application.new_document();
        let edit = application
            .active_view_mut()
            .editor
            .insert_text("cat Cat concatenate cat");
        assert!(edit.did_change);
        application.commit_active_view_to_buffer();
        application.find = Some(luna_ui::FindPanelState {
            query: "cat".to_owned(),
            replacement: "dog".to_owned(),
            replacement_is_visible: true,
            case_sensitive: false,
            whole_word: true,
            ..luna_ui::FindPanelState::default()
        });

        application.replace_all_matches();

        assert_eq!(
            application.active_view().editor.document().text(),
            "dog dog concatenate dog"
        );
        assert_eq!(
            application.lifecycle_notice.as_deref(),
            Some("Replaced 3 matches")
        );
        Ok(())
    }
}
