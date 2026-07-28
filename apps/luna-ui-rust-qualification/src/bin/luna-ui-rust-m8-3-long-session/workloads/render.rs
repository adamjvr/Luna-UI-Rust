// SPDX-License-Identifier: MPL-2.0

use super::{WorkloadResult, require, usize_to_u64};
use crate::report::WorkloadReport;
use luna_core::{InsetsI, NodeId, RectI, SizeI};
use luna_host_winit::{ApplicationError, HostControl, NativeApplication, NativeLifecycleEvent};
use luna_render::{CpuRenderer, Framebuffer};
use luna_render_wgpu::{WgpuResourcePolicy, WgpuSceneCompiler};
use luna_theme::ThemePreset;
use luna_ui::{DemoPanel, UiFrame};

const WORKLOAD: &str = "render_lifecycle_transitions";
const MAXIMUM_CPU_COMMANDS: u64 = 1_024;
const MAXIMUM_GPU_BATCHES: u64 = 256;
const MAXIMUM_GPU_ATLAS_BYTES: u64 = 8 * 1_024 * 1_024;
const MAXIMUM_FRAMEBUFFER_BYTES: u64 = 64 * 1_024 * 1_024;
const MAXIMUM_ACCESSIBILITY_NODES: u64 = 32;
const MAXIMUM_VERTEX_POLICY_BYTES: u64 = 32 * 1_024 * 1_024;
const MAXIMUM_INDEX_POLICY_BYTES: u64 = 16 * 1_024 * 1_024;

pub(super) fn run(cycles: u32) -> WorkloadResult<WorkloadReport> {
    let mut application = ProbeApplication::new()?;
    let sizes = [
        SizeI::new(640, 360),
        SizeI::new(960, 540),
        SizeI::new(1_280, 720),
        SizeI::new(1_440, 900),
    ];
    let scales = [1.0_f64, 1.25, 1.5, 2.0];
    let mut frames = 0_u64;
    let mut lifecycle_transitions = 0_u64;
    let mut maximum_cpu_commands = 0_u64;
    let mut maximum_gpu_batches = 0_u64;
    let mut maximum_gpu_atlas_bytes = 0_u64;
    let mut maximum_framebuffer_bytes = 0_u64;
    let mut maximum_accessibility_nodes = 0_u64;

    for cycle in 0..cycles {
        let resumed = application.handle_lifecycle(NativeLifecycleEvent::Resumed);
        lifecycle_transitions = lifecycle_transitions.saturating_add(1);
        require(
            WORKLOAD,
            resumed != HostControl::Exit && application.is_active(),
            "resumed lifecycle transition did not activate the probe",
        )?;

        let preset_index = usize::try_from(cycle).unwrap_or(0) % ThemePreset::ALL.len();
        let preset = ThemePreset::ALL
            .get(preset_index)
            .copied()
            .ok_or(super::invariant(
                WORKLOAD,
                "theme preset index escaped the catalog",
            ))?;
        application.set_theme(preset);
        let size_index = usize::try_from(cycle).unwrap_or(0) % sizes.len();
        let size = sizes.get(size_index).copied().ok_or(super::invariant(
            WORKLOAD,
            "viewport index escaped the fixture",
        ))?;
        let viewport = RectI::new(0, 0, size.width, size.height);
        let frame = application.build_frame(viewport)?;
        maximum_accessibility_nodes =
            maximum_accessibility_nodes.max(usize_to_u64(frame.accessibility_tree.nodes().count()));
        maximum_cpu_commands =
            maximum_cpu_commands.max(usize_to_u64(frame.display_list.commands().len()));

        for scale in scales {
            let physical = CpuRenderer::scale_logical_rect(viewport, scale);
            let physical_size = SizeI::new(physical.width.max(1), physical.height.max(1));
            let mut framebuffer = Framebuffer::new(physical_size)?;
            CpuRenderer::render_scaled(&frame.display_list, &mut framebuffer, scale);
            maximum_framebuffer_bytes =
                maximum_framebuffer_bytes.max(usize_to_u64(framebuffer.bytes().len()));

            let gpu =
                WgpuSceneCompiler::analyze_layers(&[&frame.display_list], physical_size, scale)?;
            maximum_gpu_batches = maximum_gpu_batches.max(usize_to_u64(gpu.batches));
            maximum_gpu_atlas_bytes = maximum_gpu_atlas_bytes.max(usize_to_u64(gpu.atlas_bytes));
            frames = frames.saturating_add(1);
        }

        let suspended = application.handle_lifecycle(NativeLifecycleEvent::Suspended);
        lifecycle_transitions = lifecycle_transitions.saturating_add(1);
        require(
            WORKLOAD,
            suspended != HostControl::Exit && !application.is_active(),
            "suspended lifecycle transition did not deactivate the probe",
        )?;
        let memory_warning = application.handle_lifecycle(NativeLifecycleEvent::MemoryWarning);
        lifecycle_transitions = lifecycle_transitions.saturating_add(1);
        require(
            WORKLOAD,
            memory_warning != HostControl::Exit,
            "memory-pressure transition requested application exit",
        )?;
    }

    let policy = WgpuResourcePolicy::default();
    let vertex_policy_bytes = usize_to_u64(policy.max_vertex_buffer_bytes());
    let index_policy_bytes = usize_to_u64(policy.max_index_buffer_bytes());
    require(
        WORKLOAD,
        vertex_policy_bytes <= MAXIMUM_VERTEX_POLICY_BYTES,
        "default GPU vertex policy exceeded its release limit",
    )?;
    require(
        WORKLOAD,
        index_policy_bytes <= MAXIMUM_INDEX_POLICY_BYTES,
        "default GPU index policy exceeded its release limit",
    )?;
    require(
        WORKLOAD,
        application.memory_warnings() == u64::from(cycles),
        "memory-pressure transition count changed",
    )?;
    require(
        WORKLOAD,
        maximum_cpu_commands <= MAXIMUM_CPU_COMMANDS,
        "CPU display-command high-water mark exceeded its deterministic limit",
    )?;
    require(
        WORKLOAD,
        maximum_gpu_batches <= MAXIMUM_GPU_BATCHES,
        "GPU batch high-water mark exceeded its deterministic limit",
    )?;
    require(
        WORKLOAD,
        maximum_gpu_atlas_bytes <= MAXIMUM_GPU_ATLAS_BYTES,
        "GPU atlas high-water mark exceeded its deterministic limit",
    )?;
    require(
        WORKLOAD,
        maximum_framebuffer_bytes <= MAXIMUM_FRAMEBUFFER_BYTES,
        "CPU framebuffer high-water mark exceeded its deterministic limit",
    )?;
    require(
        WORKLOAD,
        maximum_accessibility_nodes <= MAXIMUM_ACCESSIBILITY_NODES,
        "accessibility-node high-water mark exceeded its deterministic limit",
    )?;

    let mut report = WorkloadReport::new(WORKLOAD);
    report.record("cycles", u64::from(cycles));
    report.record("frames", frames);
    report.record("theme_transitions", u64::from(cycles));
    report.record("lifecycle_transitions", lifecycle_transitions);
    report.record("memory_warnings", application.memory_warnings());
    report.record("maximum_cpu_commands", maximum_cpu_commands);
    report.record("maximum_gpu_batches", maximum_gpu_batches);
    report.record("maximum_gpu_atlas_bytes", maximum_gpu_atlas_bytes);
    report.record("maximum_framebuffer_bytes", maximum_framebuffer_bytes);
    report.record("maximum_accessibility_nodes", maximum_accessibility_nodes);
    report.record("gpu_vertex_policy_bytes", vertex_policy_bytes);
    report.record("gpu_index_policy_bytes", index_policy_bytes);
    report.limit("maximum_cpu_commands", MAXIMUM_CPU_COMMANDS);
    report.limit("maximum_gpu_batches", MAXIMUM_GPU_BATCHES);
    report.limit("maximum_gpu_atlas_bytes", MAXIMUM_GPU_ATLAS_BYTES);
    report.limit("maximum_framebuffer_bytes", MAXIMUM_FRAMEBUFFER_BYTES);
    report.limit("maximum_accessibility_nodes", MAXIMUM_ACCESSIBILITY_NODES);
    report.limit("gpu_vertex_policy_bytes", MAXIMUM_VERTEX_POLICY_BYTES);
    report.limit("gpu_index_policy_bytes", MAXIMUM_INDEX_POLICY_BYTES);
    Ok(report)
}

#[derive(Clone, Debug)]
struct ProbeApplication {
    panel_id: NodeId,
    theme: ThemePreset,
    is_active: bool,
    memory_warnings: u64,
}

impl ProbeApplication {
    fn new() -> Result<Self, luna_core::NodeIdError> {
        Ok(Self {
            panel_id: NodeId::new("m8-3-long-session-panel")?,
            theme: ThemePreset::LunaDark,
            is_active: false,
            memory_warnings: 0,
        })
    }

    fn set_theme(&mut self, theme: ThemePreset) {
        self.theme = theme;
    }

    const fn is_active(&self) -> bool {
        self.is_active
    }

    const fn memory_warnings(&self) -> u64 {
        self.memory_warnings
    }
}

impl NativeApplication for ProbeApplication {
    fn build_frame(&mut self, viewport: RectI) -> Result<UiFrame, ApplicationError> {
        let theme = self.theme.theme();
        let panel = DemoPanel::new(
            self.panel_id.clone(),
            viewport.inset(InsetsI::symmetric(24, 24)),
            theme,
            format!("M8.3 {} lifecycle probe", self.theme.label()),
        );
        Ok(UiFrame::build(&panel, theme.background)?)
    }

    fn handle_lifecycle(&mut self, event: NativeLifecycleEvent) -> HostControl {
        match event {
            NativeLifecycleEvent::Resumed => {
                self.is_active = true;
                HostControl::Redraw
            }
            NativeLifecycleEvent::Suspended => {
                self.is_active = false;
                HostControl::Continue
            }
            NativeLifecycleEvent::MemoryWarning => {
                self.memory_warnings = self.memory_warnings.saturating_add(1);
                HostControl::Redraw
            }
        }
    }
}
