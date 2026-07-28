// SPDX-License-Identifier: MPL-2.0

//! Product-neutral external downstream consumer proof for Luna UI Rust.
//!
//! This crate intentionally declares its own empty Cargo workspace. It therefore compiles outside
//! Luna's primary workspace dependency graph while consuming the repository crates exclusively
//! through their public package APIs.

use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{InsetsI, NodeId, PointI, RectI, SizeI};
use luna_document_services::{StdTextFileService, SystemDialogService};
use luna_editor::KeywordSyntaxProvider;
use luna_host_wgpu::run_native_wgpu;
use luna_host_winit::{
    AccessibilityActionData, AccessibilityActionKind, AccessibilityActionRequest, ApplicationError,
    HostControl, NativeApplication, NativeLifecycleEvent, WindowConfig, run_native,
};
use luna_input::{ImeEvent, InputEvent, Key, Modifiers, NamedKey, PointerButton, PointerEventKind};
use luna_integration::{DownstreamServices, IntegrationDescriptor, ResourceLocator};
use luna_render::DisplayList;
use luna_session::{
    SessionDocument, SessionDocumentSource, SessionDocumentView, SessionPaneNode, SessionPaneTab,
    SessionPaneTree, SessionState, SessionStore, SessionWorkspace, StdSessionStore,
};
use luna_text::{EditableText, SnapBias, TextLocation, TextRange, TextScroll};
use luna_text_cosmic::{TextEngine, TextLayoutRequest, TextLayoutSnapshot};
use luna_theme::{Theme, ThemePreset};
use luna_ui::{
    Button, ControlState, ProgressBar, ScriptedCompletionProvider, TextAlignment, TextLabel,
    TextLabelCache, TextView, TextViewStyle, Toggle, UiFrame, Widget,
};
use luna_workspaces::{
    PlatformWorkspaceWatchService, StdWorkspaceService, WorkspaceModel, WorkspaceScanOptions,
    WorkspaceService,
};
use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Stable application identifier used for sessions and packaged resources.
pub const APPLICATION_ID: &str = "org.lunaui.ReferenceConsumer";

/// Human-readable application name used by the downstream integration descriptor.
pub const DISPLAY_NAME: &str = "Luna Reference Consumer";

const VIRTUAL_DOCUMENT_ID: &str = "org.lunaui.ReferenceConsumer.note";
const WELCOME_RESOURCE: &str = "welcome.txt";
const DOCUMENT_KEY: u64 = 1;
const VIEW_KEY: u64 = 1;
const PANE_KEY: u64 = 1;

/// Error-aware result used by the downstream consumer entry points.
pub type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

fn invalid_input(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.into())
}

fn qualification_error(message: impl Into<String>) -> std::io::Error {
    std::io::Error::other(message.into())
}

type ConsumerServices = DownstreamServices<
    StdTextFileService,
    SystemDialogService,
    StdWorkspaceService,
    PlatformWorkspaceWatchService,
    StdSessionStore,
    KeywordSyntaxProvider,
    ScriptedCompletionProvider,
>;

/// Parses command-line options and runs either the native consumer or its headless self-test.
///
/// # Errors
///
/// Returns an error when arguments are invalid, public Luna services fail to initialize, resource
/// discovery fails, or the selected native host cannot start.
pub fn run_from_environment() -> AppResult<()> {
    let options = Options::parse(env::args_os().skip(1))?;
    if options.show_help {
        print_help();
        return Ok(());
    }
    if options.self_test {
        return run_self_test(options.workspace_root);
    }

    let application = ReferenceConsumer::new(options.workspace_root)?;
    match options.backend {
        RenderBackend::Cpu => run_native(application)?,
        RenderBackend::Wgpu => run_native_wgpu(application)?,
    }
    Ok(())
}

/// Runs the deterministic, window-free downstream qualification exercise.
///
/// The caller should provide `LUNA_RESOURCE_ROOT` while running from the source tree. An
/// extracted package does not need that variable because [`ResourceLocator`] discovers the
/// executable-relative `share/org.lunaui.ReferenceConsumer` directory.
///
/// # Errors
///
/// Returns an error when resource resolution, workspace scanning, session round-tripping, text
/// shaping, reusable-control composition, or frame construction fails.
pub fn run_self_test(workspace_override: Option<PathBuf>) -> AppResult<()> {
    let sequence = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let temporary_root = env::temp_dir().join(format!(
        "luna-reference-consumer-self-test-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&temporary_root)?;

    let owns_workspace = workspace_override.is_none();
    let workspace_root = workspace_override.unwrap_or_else(|| temporary_root.join("workspace"));
    if owns_workspace {
        fs::create_dir_all(workspace_root.join("src"))?;
        fs::write(
            workspace_root.join("src/main.rs"),
            "fn main() { println!(\"downstream\"); }\n",
        )?;
        fs::write(workspace_root.join("README.md"), "# Downstream workspace\n")?;
    }

    let resource_locator = ResourceLocator::discover(APPLICATION_ID)?;
    let welcome = resource_locator.read_utf8(Path::new(WELCOME_RESOURCE))?;
    let session_path = temporary_root.join("session/editor-session-v2.txt");
    let services = compose_services(StdSessionStore::new(&session_path))?;
    let snapshot = services
        .workspace
        .scan(&workspace_root, WorkspaceScanOptions::default())?;
    let workspace_model = WorkspaceModel::new(snapshot);

    let mut editor = EditableText::new(welcome);
    let result = editor.insert_text("\nM8.2 self-test edit completed.\n");
    if !result.did_change {
        return Err(
            qualification_error("editable-text self-test did not change the document").into(),
        );
    }

    let state = session_state_for(
        &editor,
        TextScroll::default(),
        workspace_model.snapshot().root(),
        &workspace_model,
    );
    services.sessions.save(&state)?;
    let restored = services.sessions.load()?;
    if restored
        .documents
        .first()
        .map(|document| document.text.as_str())
        != Some(editor.document().text())
    {
        return Err(qualification_error(
            "session round-trip did not preserve the editable document",
        )
        .into());
    }

    let viewport = RectI::new(0, 0, 1_080, 720);
    let layout = ConsumerLayout::calculate(viewport);
    let theme = ThemePreset::LunaDark.theme();
    let mut engine = TextEngine::new();
    let mut labels = TextLabelCache::new();
    let editor_layout = engine.shape(
        editor.document(),
        TextLayoutRequest::new(layout.editor.width.max(1), 16.0, 23.0, theme.foreground),
    )?;
    let surface = build_surface(
        &mut engine,
        &mut labels,
        SurfaceInputs {
            ids: SurfaceIds::new()?,
            layout,
            theme,
            theme_preset: ThemePreset::LunaDark,
            editor_view: TextView::new(
                NodeId::new("reference-consumer.editor")?,
                layout.editor,
                editor.document().clone(),
                editor_layout,
                editor.caret(),
                editor.selection(),
                TextScroll::default(),
                text_view_style(theme),
                "Reference consumer editable text",
                true,
                true,
            ),
            workspace_root: workspace_model.snapshot().root(),
            workspace_rows: workspace_model.visible_rows().len(),
            status: "Headless public-API composition passed",
            reload_is_focused: false,
            theme_is_focused: false,
        },
    )?;
    let frame = UiFrame::build(&surface, theme.background)?;
    if frame.display_list.commands().is_empty() {
        return Err(
            qualification_error("headless consumer frame contained no display commands").into(),
        );
    }
    if frame.accessibility_tree.nodes().count() < 6 {
        return Err(qualification_error(
            "headless consumer frame exposed too few accessibility nodes",
        )
        .into());
    }

    println!("application_id={APPLICATION_ID}");
    println!("resource_roots={}", resource_locator.roots().len());
    println!("workspace={}", workspace_model.snapshot().root().display());
    println!("workspace_rows={}", workspace_model.visible_rows().len());
    println!("session={}", session_path.display());
    println!("display_commands={}", frame.display_list.commands().len());
    println!(
        "accessibility_nodes={}",
        frame.accessibility_tree.nodes().count()
    );
    println!("m8_2_self_test=passed");

    fs::remove_dir_all(&temporary_root)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenderBackend {
    Cpu,
    Wgpu,
}

#[derive(Debug)]
struct Options {
    backend: RenderBackend,
    self_test: bool,
    show_help: bool,
    workspace_root: Option<PathBuf>,
}

impl Options {
    fn parse(arguments: impl IntoIterator<Item = OsString>) -> AppResult<Self> {
        let mut backend = env::var("LUNA_RENDER_BACKEND")
            .ok()
            .map(|value| parse_backend(&value))
            .transpose()?
            .unwrap_or(RenderBackend::Cpu);
        let mut self_test = false;
        let mut show_help = false;
        let mut workspace_root = None;
        let mut arguments = arguments.into_iter();

        while let Some(argument) = arguments.next() {
            match argument.to_string_lossy().as_ref() {
                "--backend" => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| invalid_input("--backend requires cpu or wgpu"))?;
                    backend = parse_backend(&value.to_string_lossy())?;
                }
                "--workspace" => {
                    workspace_root =
                        Some(PathBuf::from(arguments.next().ok_or_else(|| {
                            invalid_input("--workspace requires a path")
                        })?));
                }
                "--self-test" => self_test = true,
                "-h" | "--help" => show_help = true,
                other => return Err(invalid_input(format!("unknown argument: {other}")).into()),
            }
        }

        Ok(Self {
            backend,
            self_test,
            show_help,
            workspace_root,
        })
    }
}

fn parse_backend(value: &str) -> AppResult<RenderBackend> {
    match value.to_ascii_lowercase().as_str() {
        "cpu" | "softbuffer" => Ok(RenderBackend::Cpu),
        "wgpu" | "gpu" => Ok(RenderBackend::Wgpu),
        other => Err(invalid_input(format!(
            "unsupported backend {other:?}; expected cpu or wgpu"
        ))
        .into()),
    }
}

fn print_help() {
    println!("{DISPLAY_NAME}");
    println!();
    println!("Usage: luna-reference-consumer [options]");
    println!();
    println!("  --backend cpu|wgpu   Select the native presentation backend");
    println!("  --workspace PATH     Open PATH as the consumer workspace");
    println!("  --self-test          Run the window-free public API and package test");
    println!("  -h, --help           Show this help");
}

fn compose_services(session_store: StdSessionStore) -> AppResult<ConsumerServices> {
    Ok(DownstreamServices::new(
        IntegrationDescriptor::new(APPLICATION_ID, DISPLAY_NAME)?,
        StdTextFileService,
        SystemDialogService::detect(),
        StdWorkspaceService,
        PlatformWorkspaceWatchService::default(),
        session_store,
        KeywordSyntaxProvider::rust_demo(),
        ScriptedCompletionProvider::default(),
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SurfaceIds {
    root: NodeId,
    title: NodeId,
    reload: NodeId,
    theme: NodeId,
    workspace: NodeId,
    progress: NodeId,
    editor: NodeId,
    status: NodeId,
}

impl SurfaceIds {
    fn new() -> Result<Self, luna_core::NodeIdError> {
        let root = NodeId::new("reference-consumer")?;
        Ok(Self {
            title: root.child("title")?,
            reload: root.child("reload-workspace")?,
            theme: root.child("cycle-theme")?,
            workspace: root.child("workspace-summary")?,
            progress: root.child("workspace-progress")?,
            editor: root.child("editor")?,
            status: root.child("status")?,
            root,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConsumerLayout {
    viewport: RectI,
    header: RectI,
    sidebar: RectI,
    reload_button: RectI,
    theme_toggle: RectI,
    workspace_label: RectI,
    progress: RectI,
    editor: RectI,
    footer: RectI,
}

impl ConsumerLayout {
    fn calculate(viewport: RectI) -> Self {
        let inner = viewport.inset(InsetsI::symmetric(16, 16));
        let header_height = 48_u32.min(inner.height);
        let footer_height = 30_u32.min(inner.height.saturating_sub(header_height));
        let body_height = inner
            .height
            .saturating_sub(header_height)
            .saturating_sub(footer_height);
        let body_y = inner
            .y
            .saturating_add(i32::try_from(header_height).unwrap_or(i32::MAX));
        let footer_y = body_y.saturating_add(i32::try_from(body_height).unwrap_or(i32::MAX));
        let sidebar_width = if inner.width >= 760 {
            280
        } else {
            inner.width.saturating_mul(2) / 5
        }
        .min(inner.width);
        let editor_x = inner
            .x
            .saturating_add(i32::try_from(sidebar_width).unwrap_or(i32::MAX))
            .saturating_add(12);
        let editor_width = inner.width.saturating_sub(sidebar_width).saturating_sub(12);
        let sidebar = RectI::new(inner.x, body_y, sidebar_width, body_height);
        let sidebar_inner = sidebar.inset(InsetsI::symmetric(14, 14));
        let reload_button = RectI::new(sidebar_inner.x, sidebar_inner.y, sidebar_inner.width, 38);
        let theme_toggle = RectI::new(
            sidebar_inner.x,
            sidebar_inner.y.saturating_add(52),
            sidebar_inner.width,
            34,
        );
        let progress = RectI::new(
            sidebar_inner.x,
            sidebar_inner.y.saturating_add(100),
            sidebar_inner.width,
            8,
        );
        let workspace_label = RectI::new(
            sidebar_inner.x,
            sidebar_inner.y.saturating_add(122),
            sidebar_inner.width,
            sidebar_inner.height.saturating_sub(122),
        );

        Self {
            viewport,
            header: RectI::new(inner.x, inner.y, inner.width, header_height),
            sidebar,
            reload_button,
            theme_toggle,
            workspace_label,
            progress,
            editor: RectI::new(editor_x, body_y, editor_width, body_height),
            footer: RectI::new(inner.x, footer_y, inner.width, footer_height),
        }
    }
}

struct SurfaceInputs<'a> {
    ids: SurfaceIds,
    layout: ConsumerLayout,
    theme: Theme,
    theme_preset: ThemePreset,
    editor_view: TextView,
    workspace_root: &'a Path,
    workspace_rows: usize,
    status: &'a str,
    reload_is_focused: bool,
    theme_is_focused: bool,
}

struct ConsumerSurface {
    ids: SurfaceIds,
    layout: ConsumerLayout,
    theme: Theme,
    title: TextLabel,
    reload: Button,
    theme_toggle: Toggle,
    workspace: TextLabel,
    progress: ProgressBar,
    editor: TextView,
    status: TextLabel,
}

fn build_surface(
    engine: &mut TextEngine,
    labels: &mut TextLabelCache,
    inputs: SurfaceInputs<'_>,
) -> Result<ConsumerSurface, ApplicationError> {
    let title_text = format!("{} — external public API consumer", DISPLAY_NAME);
    let workspace_text = format!(
        "Workspace\n{}\n\nVisible rows: {}\n\n\
         Session and resource loading are owned by this downstream crate.",
        inputs.workspace_root.display(),
        inputs.workspace_rows
    );
    let theme_text = format!("Cycle Theme — {}", inputs.theme_preset.label());

    let title_layout = labels.layout(
        engine,
        "reference-consumer.title",
        &title_text,
        inputs.layout.header.width.max(1),
        19.0,
        27.0,
        inputs.theme.foreground,
    )?;
    let reload_layout = labels.layout(
        engine,
        "reference-consumer.reload",
        "Reload Workspace",
        inputs.layout.reload_button.width.max(1),
        14.0,
        20.0,
        inputs.theme.foreground,
    )?;
    let theme_layout = labels.layout(
        engine,
        "reference-consumer.theme",
        &theme_text,
        inputs.layout.theme_toggle.width.saturating_sub(52).max(1),
        13.0,
        19.0,
        inputs.theme.foreground,
    )?;
    let workspace_layout = labels.layout(
        engine,
        "reference-consumer.workspace",
        &workspace_text,
        inputs.layout.workspace_label.width.max(1),
        13.0,
        19.0,
        inputs.theme.foreground,
    )?;
    let status_layout = labels.layout(
        engine,
        "reference-consumer.status",
        inputs.status,
        inputs.layout.footer.width.max(1),
        12.0,
        18.0,
        inputs.theme.muted_foreground(),
    )?;

    Ok(ConsumerSurface {
        title: TextLabel::new(
            inputs.ids.title.clone(),
            inputs.layout.header.inset(InsetsI::symmetric(12, 6)),
            title_text,
            title_layout,
            TextAlignment::Leading,
        ),
        reload: Button::new(
            inputs.ids.reload.clone(),
            inputs.layout.reload_button,
            "Reload Workspace",
            reload_layout,
            inputs.theme,
            ControlState {
                is_focused: inputs.reload_is_focused,
                ..ControlState::default()
            },
        ),
        theme_toggle: Toggle::new(
            inputs.ids.theme.clone(),
            inputs.layout.theme_toggle,
            theme_text,
            theme_layout,
            inputs.theme,
            ControlState {
                is_focused: inputs.theme_is_focused,
                ..ControlState::default()
            },
            true,
        ),
        workspace: TextLabel::new(
            inputs.ids.workspace.clone(),
            inputs.layout.workspace_label,
            workspace_text,
            workspace_layout,
            TextAlignment::Leading,
        ),
        progress: ProgressBar::new(
            inputs.ids.progress.clone(),
            inputs.layout.progress,
            inputs.theme,
            u16::try_from(inputs.workspace_rows.min(100)).unwrap_or(100),
            100,
            "Workspace entry count capped at 100",
        ),
        editor: inputs.editor_view,
        status: TextLabel::new(
            inputs.ids.status.clone(),
            inputs.layout.footer.inset(InsetsI::symmetric(10, 4)),
            inputs.status,
            status_layout,
            TextAlignment::Leading,
        ),
        ids: inputs.ids,
        layout: inputs.layout,
        theme: inputs.theme,
    })
}

impl Widget for ConsumerSurface {
    fn id(&self) -> &NodeId {
        &self.ids.root
    }

    fn bounds(&self) -> RectI {
        self.layout.viewport
    }

    fn build_display_list(&self, display_list: &mut DisplayList) {
        display_list.fill_rect(self.layout.header, self.theme.panel_header);
        display_list.fill_rect(self.layout.sidebar, self.theme.panel);
        display_list.fill_rect(self.layout.footer, self.theme.panel_header);
        self.title.build_display_list(display_list);
        self.reload.build_display_list(display_list);
        self.theme_toggle.build_display_list(display_list);
        self.workspace.build_display_list(display_list);
        self.progress.build_display_list(display_list);
        self.editor.build_display_list(display_list);
        self.status.build_display_list(display_list);
    }

    fn accessibility_nodes(&self) -> Vec<AccessibilityNode> {
        let children = vec![
            self.ids.title.clone(),
            self.ids.reload.clone(),
            self.ids.theme.clone(),
            self.ids.workspace.clone(),
            self.ids.progress.clone(),
            self.ids.editor.clone(),
            self.ids.status.clone(),
        ];
        let mut nodes = vec![
            AccessibilityNode::new(
                self.ids.root.clone(),
                AccessibilityRole::Window,
                self.layout.viewport,
            )
            .with_label(DISPLAY_NAME)
            .with_children(children),
        ];
        nodes.extend(self.title.accessibility_nodes());
        nodes.extend(self.reload.accessibility_nodes());
        nodes.extend(self.theme_toggle.accessibility_nodes());
        nodes.extend(self.workspace.accessibility_nodes());
        nodes.extend(self.progress.accessibility_nodes());
        nodes.extend(self.editor.accessibility_nodes());
        nodes.extend(self.status.accessibility_nodes());
        nodes
    }

    fn hit_test(&self, point: PointI) -> Option<NodeId> {
        self.reload
            .hit_test(point)
            .or_else(|| self.theme_toggle.hit_test(point))
            .or_else(|| self.editor.hit_test(point))
            .or_else(|| self.bounds().contains(point).then(|| self.ids.root.clone()))
    }
}

struct ReferenceConsumer {
    ids: SurfaceIds,
    services: ConsumerServices,
    resource_locator: ResourceLocator,
    workspace_model: WorkspaceModel,
    editor: EditableText,
    engine: TextEngine,
    labels: TextLabelCache,
    editor_view: Option<TextView>,
    scroll: TextScroll,
    theme_preset: ThemePreset,
    layout: ConsumerLayout,
    is_editor_focused: bool,
    focused_control: Option<NodeId>,
    drag_anchor: Option<TextLocation>,
    reveal_caret_on_next_frame: bool,
    session_dirty: bool,
    status: String,
}

impl ReferenceConsumer {
    fn new(workspace_override: Option<PathBuf>) -> AppResult<Self> {
        let session_store = StdSessionStore::for_application(APPLICATION_ID)?;
        let services = compose_services(session_store)?;
        let saved = services.sessions.load()?;
        let resource_locator = ResourceLocator::discover(APPLICATION_ID)?;
        let welcome = resource_locator.read_utf8(Path::new(WELCOME_RESOURCE))?;
        let workspace_root = match workspace_override.or_else(|| {
            saved
                .workspace
                .as_ref()
                .map(|workspace| workspace.root.clone())
        }) {
            Some(root) => root,
            None => env::current_dir()?,
        };
        let snapshot = services
            .workspace
            .scan(&workspace_root, WorkspaceScanOptions::default())?;
        let mut workspace_model = WorkspaceModel::new(snapshot);
        if let Some(workspace) = saved
            .workspace
            .as_ref()
            .filter(|workspace| workspace.root.as_path() == workspace_model.snapshot().root())
        {
            let _tree_state_changed = workspace_model.restore_tree_state(
                &workspace.expanded_paths,
                workspace.selected_path.as_deref(),
            );
        }

        let saved_document = saved.documents.iter().find(|document| {
            matches!(
                &document.source,
                SessionDocumentSource::Virtual(identifier) if identifier == VIRTUAL_DOCUMENT_ID
            )
        });
        let initial_text = match saved_document {
            Some(document) => document.text.clone(),
            None => welcome,
        };
        let mut editor = EditableText::new(initial_text);
        let mut scroll = TextScroll::default();
        if let Some(view) = saved
            .views
            .iter()
            .find(|view| view.document_key == DOCUMENT_KEY)
        {
            let document = editor.document().clone();
            let caret = document.location_for_offset(view.caret_byte, SnapBias::Backward);
            editor.set_caret(caret);
            if let (Some(anchor), Some(focus)) =
                (view.selection_anchor_byte, view.selection_focus_byte)
            {
                editor.set_selection(TextRange::new(
                    document.location_for_offset(anchor, SnapBias::Backward),
                    document.location_for_offset(focus, SnapBias::Backward),
                ));
            }
            scroll = TextScroll::new(view.scroll_x, view.scroll_y);
        }

        let viewport = RectI::new(0, 0, 1_080, 720);
        let platform = services.platform_report();
        let status = format!(
            "{} | {:?} / {:?} | resources: {} roots | session: {}",
            services.descriptor.display_name,
            platform.platform,
            platform.support_tier,
            resource_locator.roots().len(),
            services.sessions.path().display()
        );

        Ok(Self {
            ids: SurfaceIds::new()?,
            services,
            resource_locator,
            workspace_model,
            editor,
            engine: TextEngine::new(),
            labels: TextLabelCache::new(),
            editor_view: None,
            scroll,
            theme_preset: ThemePreset::LunaDark,
            layout: ConsumerLayout::calculate(viewport),
            is_editor_focused: true,
            focused_control: None,
            drag_anchor: None,
            reveal_caret_on_next_frame: true,
            session_dirty: false,
            status,
        })
    }

    fn theme(&self) -> Theme {
        self.theme_preset.theme()
    }

    fn text_width(&self) -> u32 {
        let style = text_view_style(self.theme());
        self.layout
            .editor
            .inset(style.content_insets)
            .width
            .saturating_sub(style.gutter_width)
            .saturating_sub(style.scrollbar_width)
            .max(1)
    }

    fn view(&self, layout: TextLayoutSnapshot) -> TextView {
        TextView::new(
            self.ids.editor.clone(),
            self.layout.editor,
            self.editor.document().clone(),
            layout,
            self.editor.caret(),
            self.editor.selection(),
            self.scroll,
            text_view_style(self.theme()),
            "Reference consumer editable text",
            self.is_editor_focused,
            true,
        )
    }

    fn current_view(&self) -> Option<&TextView> {
        self.editor_view.as_ref()
    }

    fn mark_session_dirty(&mut self) {
        self.session_dirty = true;
    }

    fn mark_edit(&mut self, changed: bool) {
        if changed {
            self.mark_session_dirty();
            self.reveal_caret_on_next_frame = true;
        }
    }

    fn reload_workspace(&mut self) -> HostControl {
        let root = self.workspace_model.snapshot().root().to_path_buf();
        match self
            .services
            .workspace
            .scan(&root, WorkspaceScanOptions::default())
        {
            Ok(snapshot) => {
                self.workspace_model = WorkspaceModel::new(snapshot);
                self.status = format!(
                    "Reloaded {} workspace rows from {}",
                    self.workspace_model.visible_rows().len(),
                    self.workspace_model.snapshot().root().display()
                );
                self.mark_session_dirty();
                HostControl::Redraw
            }
            Err(error) => {
                self.status = format!("Workspace reload failed: {error}");
                HostControl::Redraw
            }
        }
    }

    fn cycle_theme(&mut self) -> HostControl {
        self.theme_preset = self.theme_preset.next();
        self.labels.clear();
        self.status = format!("Theme changed to {}", self.theme_preset.label());
        HostControl::Redraw
    }

    fn persist_session(&mut self) -> AppResult<()> {
        let state = session_state_for(
            &self.editor,
            self.scroll,
            self.workspace_model.snapshot().root(),
            &self.workspace_model,
        );
        self.services.sessions.save(&state)?;
        self.session_dirty = false;
        self.status = format!(
            "Session saved to {} | resource roots: {}",
            self.services.sessions.path().display(),
            self.resource_locator.roots().len()
        );
        Ok(())
    }

    fn persist_or_report(&mut self) -> bool {
        match self.persist_session() {
            Ok(()) => true,
            Err(error) => {
                self.status = format!("Session save failed: {error}");
                false
            }
        }
    }

    fn apply_pointer_location(&mut self, position: PointI, extending: bool) -> bool {
        let Some(view) = self.current_view() else {
            return false;
        };
        let Some(location) = view.text_hit_test(position) else {
            return false;
        };
        if extending {
            let anchor = self.drag_anchor.unwrap_or(self.editor.caret());
            self.editor.set_selection(TextRange::new(anchor, location));
        } else {
            self.editor.set_caret(location);
            self.drag_anchor = Some(location);
        }
        self.is_editor_focused = true;
        self.focused_control = None;
        self.reveal_caret_on_next_frame = true;
        self.mark_session_dirty();
        true
    }

    fn handle_navigation_key(&mut self, key: NamedKey, modifiers: Modifiers) -> HostControl {
        let extending = modifiers.contains(Modifiers::SHIFT);
        let mut reveal_caret = true;
        let mut changed = false;
        match key {
            NamedKey::ArrowLeft => self.editor.move_backward(extending),
            NamedKey::ArrowRight => self.editor.move_forward(extending),
            NamedKey::ArrowUp => self.editor.move_up(extending),
            NamedKey::ArrowDown => self.editor.move_down(extending),
            NamedKey::Home => self.editor.move_to_line_start(extending),
            NamedKey::End => self.editor.move_to_line_end(extending),
            NamedKey::Backspace => changed = self.editor.delete_backward().did_change,
            NamedKey::Delete => changed = self.editor.delete_forward().did_change,
            NamedKey::Enter => changed = self.editor.insert_newline().did_change,
            NamedKey::Tab => changed = self.editor.insert_text("    ").did_change,
            NamedKey::PageUp => {
                let amount = i32::try_from(self.layout.editor.height).unwrap_or(i32::MAX);
                self.scroll.y = self.scroll.y.saturating_sub(amount).max(0);
                reveal_caret = false;
            }
            NamedKey::PageDown => {
                let amount = i32::try_from(self.layout.editor.height).unwrap_or(i32::MAX);
                self.scroll.y = self.scroll.y.saturating_add(amount);
                reveal_caret = false;
            }
            NamedKey::Escape => return HostControl::Continue,
        }
        self.mark_edit(changed);
        self.reveal_caret_on_next_frame = reveal_caret;
        self.mark_session_dirty();
        HostControl::Redraw
    }
}

impl NativeApplication for ReferenceConsumer {
    fn window_config(&self) -> WindowConfig {
        WindowConfig {
            title: format!("{DISPLAY_NAME} — M8.2"),
            initial_size: SizeI::new(1_080, 720),
            minimum_size: Some(SizeI::new(720, 480)),
        }
    }

    fn accepts_text_input(&self) -> bool {
        self.is_editor_focused
    }

    fn ime_cursor_area(&self) -> Option<RectI> {
        self.current_view()
            .and_then(TextView::caret_bounds)
            .or(Some(RectI::new(
                self.layout.editor.x.saturating_add(16),
                self.layout.editor.y.saturating_add(16),
                2,
                22,
            )))
    }

    fn build_frame(&mut self, viewport: RectI) -> Result<UiFrame, ApplicationError> {
        self.layout = ConsumerLayout::calculate(viewport);
        let theme = self.theme();
        let text_width = self.text_width();
        let editor_layout = self.engine.shape(
            self.editor.document(),
            TextLayoutRequest::new(text_width, 16.0, 23.0, theme.foreground),
        )?;
        if self.reveal_caret_on_next_frame {
            self.scroll = self.view(editor_layout.clone()).scroll_revealing_caret();
            self.reveal_caret_on_next_frame = false;
        }
        let editor_view = self.view(editor_layout.clone());
        self.editor_view = Some(editor_view.clone());
        let workspace_root = self.workspace_model.snapshot().root().to_path_buf();
        let workspace_rows = self.workspace_model.visible_rows().len();
        let status = self.status.clone();
        let focused_control = self.focused_control.clone();
        let surface = build_surface(
            &mut self.engine,
            &mut self.labels,
            SurfaceInputs {
                ids: self.ids.clone(),
                layout: self.layout,
                theme,
                theme_preset: self.theme_preset,
                editor_view,
                workspace_root: &workspace_root,
                workspace_rows,
                status: &status,
                reload_is_focused: focused_control.as_ref() == Some(&self.ids.reload),
                theme_is_focused: focused_control.as_ref() == Some(&self.ids.theme),
            },
        )?;
        Ok(UiFrame::build(&surface, theme.background)?)
    }

    fn handle_input(&mut self, event: InputEvent) -> HostControl {
        match event {
            InputEvent::Keyboard(keyboard) if keyboard.is_pressed => {
                if keyboard.key == Key::Named(NamedKey::Escape) {
                    return if self.persist_or_report() {
                        HostControl::Exit
                    } else {
                        HostControl::Redraw
                    };
                }
                let command_modified = keyboard.modifiers.contains(Modifiers::SUPER)
                    || (keyboard.modifiers.contains(Modifiers::CONTROL)
                        && !keyboard.modifiers.contains(Modifiers::ALT));
                if command_modified && let Key::Character(value) = &keyboard.key {
                    if value.eq_ignore_ascii_case("s") {
                        let _session_saved = self.persist_or_report();
                        return HostControl::Redraw;
                    }
                    if value.eq_ignore_ascii_case("r") {
                        return self.reload_workspace();
                    }
                    if value.eq_ignore_ascii_case("t") {
                        return self.cycle_theme();
                    }
                    if value.eq_ignore_ascii_case("a") {
                        self.editor.set_selection(TextRange::new(
                            TextLocation::default(),
                            self.editor.document().end_location(),
                        ));
                        self.reveal_caret_on_next_frame = true;
                        self.mark_session_dirty();
                        return HostControl::Redraw;
                    }
                }
                if let Key::Named(key) = &keyboard.key {
                    return self.handle_navigation_key(*key, keyboard.modifiers);
                }
                let fallback = match &keyboard.key {
                    Key::Character(value) => Some(value.as_str()),
                    Key::Named(_) | Key::Unidentified => None,
                };
                if let Some(text) = keyboard.text.as_deref().or(fallback)
                    && !command_modified
                    && !text.is_empty()
                    && !text.chars().all(char::is_control)
                {
                    let changed = self.editor.insert_text(text).did_change;
                    self.mark_edit(changed);
                    return HostControl::Redraw;
                }
            }
            InputEvent::Text(text) => {
                let changed = self.editor.insert_text(&text).did_change;
                self.mark_edit(changed);
                return HostControl::Redraw;
            }
            InputEvent::Ime(ImeEvent::Commit(text)) => {
                let changed = self.editor.insert_text(&text).did_change;
                self.mark_edit(changed);
                return HostControl::Redraw;
            }
            InputEvent::Ime(ImeEvent::Enabled | ImeEvent::Disabled | ImeEvent::Preedit { .. }) => {}
            InputEvent::Pointer(pointer) => match pointer.kind {
                PointerEventKind::Pressed(PointerButton::Primary) => {
                    if self.layout.reload_button.contains(pointer.position) {
                        self.focused_control = Some(self.ids.reload.clone());
                        self.is_editor_focused = false;
                        return self.reload_workspace();
                    }
                    if self.layout.theme_toggle.contains(pointer.position) {
                        self.focused_control = Some(self.ids.theme.clone());
                        self.is_editor_focused = false;
                        return self.cycle_theme();
                    }
                    let extending = pointer.modifiers.contains(Modifiers::SHIFT);
                    if extending {
                        self.drag_anchor = self
                            .editor
                            .selection()
                            .map(|selection| selection.anchor)
                            .or(Some(self.editor.caret()));
                    }
                    if self.apply_pointer_location(pointer.position, extending) {
                        return HostControl::Redraw;
                    }
                }
                PointerEventKind::Moved if self.drag_anchor.is_some() => {
                    if self.apply_pointer_location(pointer.position, true) {
                        return HostControl::Redraw;
                    }
                }
                PointerEventKind::Released(PointerButton::Primary) => self.drag_anchor = None,
                PointerEventKind::Moved
                | PointerEventKind::Pressed(_)
                | PointerEventKind::Released(_)
                | PointerEventKind::Left => {}
            },
            InputEvent::Scroll(scroll) => {
                if let Some(view) = self.current_view() {
                    let maximum = view.maximum_scroll();
                    let (delta_x, delta_y) =
                        if scroll.modifiers.contains(Modifiers::SHIFT) && scroll.delta_x == 0 {
                            (scroll.delta_y, 0)
                        } else {
                            (scroll.delta_x, scroll.delta_y)
                        };
                    self.scroll.scroll_by(
                        delta_x.saturating_neg(),
                        delta_y.saturating_neg(),
                        maximum.x,
                        maximum.y,
                    );
                    self.mark_session_dirty();
                    return HostControl::Redraw;
                }
            }
            InputEvent::FocusGained => {
                self.is_editor_focused = true;
                return HostControl::Redraw;
            }
            InputEvent::FocusLost => {
                self.is_editor_focused = false;
                self.drag_anchor = None;
                let _session_saved = self.persist_or_report();
                return HostControl::Redraw;
            }
            InputEvent::Keyboard(_) => {}
        }
        HostControl::Continue
    }

    fn handle_accessibility_action(&mut self, request: AccessibilityActionRequest) -> HostControl {
        if request.target.as_ref() == Some(&self.ids.reload)
            && request.kind == AccessibilityActionKind::Click
        {
            return self.reload_workspace();
        }
        if request.target.as_ref() == Some(&self.ids.theme)
            && request.kind == AccessibilityActionKind::Click
        {
            return self.cycle_theme();
        }
        if request.target.as_ref() != Some(&self.ids.editor) {
            return HostControl::Continue;
        }
        match request.kind {
            AccessibilityActionKind::Focus => {
                self.is_editor_focused = true;
                self.focused_control = None;
                HostControl::Redraw
            }
            AccessibilityActionKind::ReplaceSelectedText => {
                let AccessibilityActionData::Value(value) = request.data else {
                    return HostControl::Continue;
                };
                let changed = self.editor.insert_text(&value).did_change;
                self.mark_edit(changed);
                HostControl::Redraw
            }
            AccessibilityActionKind::SetValue => {
                let AccessibilityActionData::Value(value) = request.data else {
                    return HostControl::Continue;
                };
                let end = self.editor.document().end_location();
                self.editor
                    .set_selection(TextRange::new(TextLocation::default(), end));
                let changed = self.editor.insert_text(&value).did_change;
                self.mark_edit(changed);
                HostControl::Redraw
            }
            AccessibilityActionKind::Click
            | AccessibilityActionKind::ShowContextMenu
            | AccessibilityActionKind::Increment
            | AccessibilityActionKind::Decrement
            | AccessibilityActionKind::Other => HostControl::Continue,
        }
    }

    fn handle_lifecycle(&mut self, event: NativeLifecycleEvent) -> HostControl {
        match event {
            NativeLifecycleEvent::Suspended | NativeLifecycleEvent::MemoryWarning => {
                let _session_saved = self.persist_or_report();
                HostControl::Continue
            }
            NativeLifecycleEvent::Resumed => HostControl::Continue,
        }
    }

    fn has_unsaved_changes(&self) -> bool {
        self.session_dirty
    }

    fn request_close(&mut self) -> HostControl {
        if self.persist_or_report() {
            HostControl::Exit
        } else {
            HostControl::Redraw
        }
    }
}

fn text_view_style(theme: Theme) -> TextViewStyle {
    let mut style = TextViewStyle::from_theme(theme);
    style.gutter_width = 0;
    style
}

fn session_state_for(
    editor: &EditableText,
    scroll: TextScroll,
    workspace_root: &Path,
    workspace_model: &WorkspaceModel,
) -> SessionState {
    let document = editor.document();
    let caret_byte = document.absolute_offset(editor.caret(), SnapBias::Backward);
    let (selection_anchor_byte, selection_focus_byte) =
        editor.selection().map_or((None, None), |selection| {
            (
                Some(document.absolute_offset(selection.anchor, SnapBias::Backward)),
                Some(document.absolute_offset(selection.focus, SnapBias::Backward)),
            )
        });
    let selected_path = workspace_model.selected().and_then(|selected| {
        workspace_model
            .snapshot()
            .node(selected)
            .map(|node| node.path().to_path_buf())
    });

    SessionState {
        recent_files: Vec::new(),
        workspace: Some(SessionWorkspace {
            root: workspace_root.to_path_buf(),
            expanded_paths: workspace_model.expanded_paths(),
            selected_path,
        }),
        documents: vec![SessionDocument {
            document_key: DOCUMENT_KEY,
            source: SessionDocumentSource::Virtual(VIRTUAL_DOCUMENT_ID.to_owned()),
            title: "Reference Consumer Note".to_owned(),
            text: document.text().to_owned(),
            is_dirty: false,
            storage_snapshot: None,
        }],
        views: vec![SessionDocumentView {
            view_key: VIEW_KEY,
            document_key: DOCUMENT_KEY,
            caret_byte,
            selection_anchor_byte,
            selection_focus_byte,
            scroll_x: scroll.x,
            scroll_y: scroll.y,
        }],
        pane_tree: Some(SessionPaneTree {
            focused_pane_key: PANE_KEY,
            root: SessionPaneNode::Leaf {
                pane_key: PANE_KEY,
                tabs: vec![SessionPaneTab {
                    view_key: VIEW_KEY,
                    is_pinned: false,
                    is_preview: false,
                }],
                active_view_key: VIEW_KEY,
                tab_scroll_offset: 0,
            },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{Options, RenderBackend, SurfaceIds, parse_backend};
    use std::ffi::OsString;

    #[test]
    fn backend_parser_accepts_documented_aliases() {
        assert_eq!(parse_backend("cpu").ok(), Some(RenderBackend::Cpu));
        assert_eq!(parse_backend("softbuffer").ok(), Some(RenderBackend::Cpu));
        assert_eq!(parse_backend("wgpu").ok(), Some(RenderBackend::Wgpu));
        assert_eq!(parse_backend("gpu").ok(), Some(RenderBackend::Wgpu));
        assert!(parse_backend("other").is_err());
    }

    #[test]
    fn options_keep_the_consumer_outside_product_policy() {
        let options = Options::parse([
            OsString::from("--backend"),
            OsString::from("wgpu"),
            OsString::from("--workspace"),
            OsString::from("/tmp/reference-consumer"),
        ]);
        assert!(matches!(
            options.map(|value| value.backend),
            Ok(RenderBackend::Wgpu)
        ));
    }

    #[test]
    fn surface_ids_are_stable_and_hierarchical() {
        let ids = SurfaceIds::new();
        assert!(ids.is_ok());
        let ids = ids.ok();
        assert_eq!(
            ids.as_ref().map(|value| value.editor.as_str()),
            Some("reference-consumer.editor")
        );
    }
}
