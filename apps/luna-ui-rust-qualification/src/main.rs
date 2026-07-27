// SPDX-License-Identifier: MPL-2.0

//! Deterministic executable release-qualification fixture for Luna UI Rust M7.

use luna_accessibility::{AccessibilityNode, AccessibilityRole, AccessibilityTree};
use luna_core::{NodeId, PointI, RectI, SizeI};
use luna_documents::{DocumentRegistry, DocumentViewRegistry};
use luna_editor::{
    ByteSelection, EditorParityFixture, ParityOperation, ParityResult, SelectionSet,
};
use luna_panes::{PaneAxis, PaneLayoutMetrics, PaneTree};
use luna_qualification::{QualificationMeasurement, QualificationMetric, QualificationProfile};
use luna_render::{CpuRenderer, DisplayList, Framebuffer, RasterImage};
use luna_render_wgpu::{WgpuResourcePolicy, WgpuSceneCompiler};
use luna_text::TextDocument;
use luna_text_cosmic::{TextEngine, TextLayoutCache, TextLayoutRequest};
use luna_theme::{Rgba8, ThemePreset};
use luna_ui::{DropdownMenu, DropdownMenuState, MenuCommand, MenuDefinition, MenuItem};
use luna_workspaces::{MemoryWorkspaceService, WorkspaceScanOptions, WorkspaceService};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let output_path = output_path_from_args(env::args().skip(1))?;
    let measurements = collect_measurements()?;
    let report = QualificationProfile::m7_release().evaluate(measurements)?;
    let json = report.to_json();

    if let Some(path) = output_path {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, format!("{json}\n"))?;
        println!("M7 qualification passed: {}", path.display());
    } else {
        println!("{json}");
    }
    Ok(())
}

fn output_path_from_args(
    mut arguments: impl Iterator<Item = String>,
) -> Result<Option<PathBuf>, Box<dyn Error>> {
    let Some(argument) = arguments.next() else {
        return Ok(None);
    };
    if argument != "--output" {
        return Err(format!("unknown qualification argument: {argument}").into());
    }
    let Some(path) = arguments.next() else {
        return Err("--output requires a path".into());
    };
    if let Some(extra) = arguments.next() {
        return Err(format!("unexpected qualification argument: {extra}").into());
    }
    Ok(Some(PathBuf::from(path)))
}

fn collect_measurements() -> Result<Vec<QualificationMeasurement>, Box<dyn Error>> {
    let editor_operations = editor_fixture()?;
    let (layout_misses, raster_misses) = text_fixture()?;
    let (pane_leaves, pane_splitters) = pane_fixture()?;
    let workspace_nodes = workspace_fixture()?;
    let menu_rows = menu_fixture()?;
    let (cpu_commands, gpu_batches, gpu_atlas_bytes) = rendering_fixture()?;
    let accessibility_nodes = accessibility_fixture()?;
    let policy = WgpuResourcePolicy::default();

    Ok(vec![
        measurement_usize(
            QualificationMetric::EditorReplayOperations,
            editor_operations,
        ),
        measurement_u64(QualificationMetric::TextLayoutMisses, layout_misses),
        measurement_u64(QualificationMetric::TextRasterMisses, raster_misses),
        measurement_usize(QualificationMetric::PaneLeaves, pane_leaves),
        measurement_usize(QualificationMetric::PaneSplitters, pane_splitters),
        measurement_usize(QualificationMetric::MenuRows, menu_rows),
        measurement_usize(QualificationMetric::WorkspaceNodes, workspace_nodes),
        measurement_usize(QualificationMetric::CpuDisplayCommands, cpu_commands),
        measurement_usize(QualificationMetric::GpuDrawBatches, gpu_batches),
        measurement_usize(QualificationMetric::GpuAtlasBytes, gpu_atlas_bytes),
        measurement_usize(
            QualificationMetric::GpuVertexCapacityBytes,
            policy.max_vertex_buffer_bytes(),
        ),
        measurement_usize(
            QualificationMetric::GpuIndexCapacityBytes,
            policy.max_index_buffer_bytes(),
        ),
        measurement_usize(QualificationMetric::AccessibilityNodes, accessibility_nodes),
    ])
}

fn measurement_usize(metric: QualificationMetric, value: usize) -> QualificationMeasurement {
    QualificationMeasurement {
        metric,
        value: u64::try_from(value).unwrap_or(u64::MAX),
    }
}

const fn measurement_u64(metric: QualificationMetric, value: u64) -> QualificationMeasurement {
    QualificationMeasurement { metric, value }
}

fn editor_fixture() -> Result<usize, Box<dyn Error>> {
    let fixture = EditorParityFixture {
        name: "m7 multi-cursor replay".to_owned(),
        initial_text: "one\ntwo".to_owned(),
        initial_selections: SelectionSet::single(ByteSelection::caret(0)),
        operations: vec![
            ParityOperation::AddCursorBelow,
            ParityOperation::Insert("> ".to_owned()),
            ParityOperation::Undo,
            ParityOperation::Redo,
        ],
        expected: ParityResult {
            text: "> one\n> two".to_owned(),
            selections: SelectionSet::new(
                "> one\n> two",
                [ByteSelection::caret(2), ByteSelection::caret(8)],
                1,
            )?,
            undo_depth: 1,
            redo_depth: 0,
        },
    };
    fixture.verify()?;
    Ok(fixture.operations.len())
}

fn text_fixture() -> Result<(u64, u64), Box<dyn Error>> {
    let document = TextDocument::new(
        (0..240)
            .map(|line| format!("qualification line {line}\n"))
            .collect::<String>(),
    );
    let request = TextLayoutRequest::new(480, 14.0, 20.0, Rgba8::opaque(232, 236, 244));
    let mut engine = TextEngine::new();
    let mut cache = TextLayoutCache::new();
    for scroll_y in [0, 40, 120, 2_400, 2_440, 0] {
        cache.update(&mut engine, &document, 1, request, scroll_y, 240)?;
    }
    let stats = cache.stats();
    Ok((stats.layout_misses, stats.raster_misses))
}

fn pane_fixture() -> Result<(usize, usize), Box<dyn Error>> {
    let mut documents = DocumentRegistry::new();
    let mut views = DocumentViewRegistry::new();
    let first = views.create_view(documents.create_untitled(0));
    let second = views.create_view(documents.create_untitled(0));
    let third = views.create_view(documents.create_untitled(0));
    let fourth = views.create_view(documents.create_untitled(0));
    let mut panes = PaneTree::new(first);
    let second_pane = panes.split_focused(PaneAxis::Horizontal, second);
    let _third_pane = panes.split(second_pane, PaneAxis::Vertical, third)?;
    let _fourth_pane = panes.split_focused(PaneAxis::Vertical, fourth);
    let layout = panes.layout(RectI::new(0, 0, 1_280, 800), PaneLayoutMetrics::default());
    Ok((layout.leaves.len(), layout.splitters.len()))
}

fn workspace_fixture() -> Result<usize, Box<dyn Error>> {
    let root = Path::new("/workspace");
    let workspace = MemoryWorkspaceService::new(root)?;
    workspace.insert_directory(Path::new("src"))?;
    workspace.insert_directory(Path::new("docs"))?;
    workspace.insert_file(Path::new("src/main.rs"))?;
    workspace.insert_file(Path::new("src/lib.rs"))?;
    workspace.insert_file(Path::new("docs/README.md"))?;
    workspace.insert_file(Path::new("Cargo.toml"))?;
    let snapshot = workspace.scan(root, WorkspaceScanOptions::default())?;
    Ok(snapshot.nodes().len())
}

fn menu_fixture() -> Result<usize, Box<dyn Error>> {
    let theme_items = ThemePreset::ALL
        .into_iter()
        .map(|preset| {
            MenuItem::command(
                MenuCommand::new(format!("theme:{}", preset.id()), preset.label(), "")
                    .with_checked(preset == ThemePreset::LunaDark),
            )
        })
        .collect();
    let definition = MenuDefinition::new(
        "view",
        "View",
        vec![
            MenuItem::command(MenuCommand::new("view.sidebar", "Sidebar", "Ctrl+B")),
            MenuItem::Separator,
            MenuItem::submenu(MenuDefinition::new(
                "view.color-scheme",
                "Color Scheme",
                theme_items,
            )),
            MenuItem::Separator,
            MenuItem::command(MenuCommand::new(
                "view.command-reference",
                "Editor Command Reference",
                "",
            )),
        ],
    );
    let state = DropdownMenuState {
        active_menu_id: Some("view".to_owned()),
        selected_index: 2,
        active_submenu_index: Some(2),
        submenu_selected_index: 0,
        selection_path: vec![2, 0],
    };
    let menu = DropdownMenu::new_with_state(
        NodeId::new("qualification-menu")?,
        RectI::new(0, 0, 1_280, 800),
        RectI::new(320, 0, 64, 28),
        ThemePreset::LunaDark.theme(),
        definition,
        &state,
    )?;
    Ok(menu.layout().rows.len())
}

fn rendering_fixture() -> Result<(usize, usize, usize), Box<dyn Error>> {
    let mut display_list = DisplayList::new();
    display_list.clear(Rgba8::opaque(18, 21, 28));
    display_list.push_clip(RectI::new(8, 8, 240, 144));
    display_list.fill_rect(RectI::new(12, 12, 220, 120), Rgba8::opaque(70, 91, 146));
    let image = RasterImage::new(
        SizeI::new(2, 2),
        vec![
            255, 220, 180, 255, 180, 220, 255, 255, 180, 255, 220, 255, 255, 255, 255, 255,
        ],
    )?;
    display_list.draw_image(PointI::new(24, 24), image);
    display_list.pop_clip();

    let mut framebuffer = Framebuffer::new(SizeI::new(256, 160))?;
    CpuRenderer::render(&display_list, &mut framebuffer);
    let stats = WgpuSceneCompiler::analyze_layers(&[&display_list], SizeI::new(256, 160), 1.0)?;
    Ok((
        display_list.commands().len(),
        stats.batches,
        stats.atlas_bytes,
    ))
}

fn accessibility_fixture() -> Result<usize, Box<dyn Error>> {
    let root = NodeId::new("qualification-root")?;
    let menu = root.child("menu")?;
    let editor = root.child("editor")?;
    let status = root.child("status")?;
    let nodes = vec![
        AccessibilityNode::new(
            root.clone(),
            AccessibilityRole::Window,
            RectI::new(0, 0, 1_280, 800),
        )
        .with_label("Luna qualification fixture")
        .with_children(vec![menu.clone(), editor.clone(), status.clone()]),
        AccessibilityNode::new(
            menu,
            AccessibilityRole::MenuBar,
            RectI::new(0, 0, 1_280, 28),
        ),
        AccessibilityNode::new(
            editor,
            AccessibilityRole::TextArea,
            RectI::new(0, 28, 1_280, 744),
        )
        .with_label("Editor")
        .with_editable(true),
        AccessibilityNode::new(
            status,
            AccessibilityRole::Status,
            RectI::new(0, 772, 1_280, 28),
        )
        .with_label("Ready"),
    ];
    let tree = AccessibilityTree::new(root, nodes)?;
    Ok(tree.nodes().count())
}
