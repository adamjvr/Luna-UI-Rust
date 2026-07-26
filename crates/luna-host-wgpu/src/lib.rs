// SPDX-License-Identifier: MPL-2.0

//! Native `wgpu` presentation host for Luna UI Rust.
//!
//! This host drives the same [`luna_host_winit::NativeApplication`] contract as the CPU/softbuffer
//! host. Applications can therefore compare CPU and GPU presentation without changing widget,
//! input, command, or accessibility code. Surface invalidation and device-loss callbacks rebuild
//! GPU resources while retaining application state and stable semantic identities.

use accesskit::Action;
use accesskit_winit::{Adapter as AccessKitAdapter, Event as AccessKitEvent};
use luna_accessibility_accesskit::AccessKitBridge;
use luna_core::{RectI, SizeI};
use luna_host_core::{FrameRuntime, InvalidationReason};
use luna_host_winit::{
    AccessibilityActionKind, AccessibilityActionRequest, HostControl, NativeApplication,
    WindowConfig, WinitInputTranslator,
};
use luna_render_wgpu::{WgpuRenderError, WgpuRenderStats, WgpuRenderer};
use luna_ui::UiFrame;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

/// Native GPU-host startup or runtime failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WgpuHostError {
    message: String,
}

impl WgpuHostError {
    fn from_display(context: &str, error: impl Display) -> Self {
        Self {
            message: format!("{context}: {error}"),
        }
    }

    fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for WgpuHostError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for WgpuHostError {}

/// Runs a Luna application in a native winit window using the `wgpu` renderer.
///
/// The application contract is identical to [`luna_host_winit::run_native`]. Set `WGPU_BACKEND`
/// to exercise a specific driver family during comparison testing.
pub fn run_native_wgpu(application: impl NativeApplication + 'static) -> Result<(), WgpuHostError> {
    let event_loop = EventLoop::<HostEvent>::with_user_event()
        .build()
        .map_err(|error| WgpuHostError::from_display("failed to create winit event loop", error))?;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
        Box::new(event_loop.owned_display_handle()),
    ));
    let proxy = event_loop.create_proxy();
    let mut host = WgpuHost::new(application, instance, proxy);
    let loop_result = event_loop.run_app(&mut host);

    if let Some(error) = host.fatal_error.take() {
        return Err(error);
    }
    loop_result.map_err(|error| WgpuHostError::from_display("winit event loop failed", error))
}

#[derive(Debug)]
enum HostEvent {
    AccessKit(AccessKitEvent),
    DeviceLost,
}

impl From<AccessKitEvent> for HostEvent {
    fn from(value: AccessKitEvent) -> Self {
        Self::AccessKit(value)
    }
}

struct GpuRuntime {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: WgpuRenderer,
    device_lost: Arc<AtomicBool>,
    device_lost_message: Arc<Mutex<Option<String>>>,
}

impl GpuRuntime {
    fn create(
        instance: &wgpu::Instance,
        window: Arc<Window>,
        proxy: EventLoopProxy<HostEvent>,
    ) -> Result<Self, WgpuHostError> {
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| WgpuHostError::from_display("failed to create wgpu surface", error))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .map_err(|error| WgpuHostError::from_display("failed to request wgpu adapter", error))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Luna UI Rust wgpu device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| WgpuHostError::from_display("failed to request wgpu device", error))?;

        let physical = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, physical.width.max(1), physical.height.max(1))
            .ok_or_else(|| WgpuHostError::message("wgpu surface has no supported configuration"))?;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);

        let device_lost = Arc::new(AtomicBool::new(false));
        let device_lost_message = Arc::new(Mutex::new(None));
        let lost_flag = Arc::clone(&device_lost);
        let lost_message = Arc::clone(&device_lost_message);
        device.set_device_lost_callback(move |reason, message| {
            lost_flag.store(true, Ordering::Release);
            if let Ok(mut slot) = lost_message.lock() {
                *slot = Some(format!("{reason:?}: {message}"));
            }
            let _ = proxy.send_event(HostEvent::DeviceLost);
        });

        let info = adapter.get_info();
        eprintln!(
            "[luna-wgpu adapter] name={} backend={:?} device_type={:?} driver={} info={}",
            info.name, info.backend, info.device_type, info.driver, info.driver_info
        );
        let renderer = WgpuRenderer::new(&device, config.format);
        Ok(Self {
            surface,
            device,
            queue,
            config,
            renderer,
            device_lost,
            device_lost_message,
        })
    }

    fn resize(&mut self, size: SizeI) {
        self.config.width = size.width.max(1);
        self.config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    fn lost_detail(&self) -> String {
        self.device_lost_message
            .lock()
            .ok()
            .and_then(|slot| slot.clone())
            .unwrap_or_else(|| "device lost without driver detail".to_owned())
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct GpuTiming {
    application_build: Duration,
    encode: Duration,
    presentation: Duration,
    accessibility: Duration,
    total: Duration,
}

#[derive(Debug)]
struct GpuMetrics {
    started_at: Instant,
    frames: u64,
    total_frames: u64,
    timing: GpuTiming,
    renderer: WgpuRenderStats,
    surface_reconfigurations: u64,
    device_recoveries: u64,
}

impl GpuMetrics {
    fn new() -> Self {
        Self {
            started_at: Instant::now(),
            frames: 0,
            total_frames: 0,
            timing: GpuTiming::default(),
            renderer: WgpuRenderStats::default(),
            surface_reconfigurations: 0,
            device_recoveries: 0,
        }
    }

    fn record(&mut self, timing: GpuTiming, renderer: WgpuRenderStats) {
        self.frames = self.frames.saturating_add(1);
        self.total_frames = self.total_frames.saturating_add(1);
        self.timing.application_build += timing.application_build;
        self.timing.encode += timing.encode;
        self.timing.presentation += timing.presentation;
        self.timing.accessibility += timing.accessibility;
        self.timing.total += timing.total;
        self.renderer.commands = self.renderer.commands.saturating_add(renderer.commands);
        self.renderer.batches = self.renderer.batches.saturating_add(renderer.batches);
        self.renderer.vertices = self.renderer.vertices.saturating_add(renderer.vertices);
        self.renderer.indices = self.renderer.indices.saturating_add(renderer.indices);
        self.renderer.atlas_images = self
            .renderer
            .atlas_images
            .saturating_add(renderer.atlas_images);
        self.renderer.atlas_bytes = self
            .renderer
            .atlas_bytes
            .saturating_add(renderer.atlas_bytes);

        if self.total_frames == 1 || self.started_at.elapsed() >= Duration::from_secs(1) {
            let divisor = self.frames.max(1) as f64;
            eprintln!(
                "[luna-wgpu metrics] frames={} total_frames={} avg_ms={{app:{:.3}, encode:{:.3}, present:{:.3}, accessibility:{:.3}, total:{:.3}}} avg_scene={{commands:{:.1}, batches:{:.1}, vertices:{:.1}, indices:{:.1}, atlas_images:{:.1}, atlas_kib:{:.1}}} recoveries={{surface:{}, device:{}}}",
                self.frames,
                self.total_frames,
                millis(self.timing.application_build, divisor),
                millis(self.timing.encode, divisor),
                millis(self.timing.presentation, divisor),
                millis(self.timing.accessibility, divisor),
                millis(self.timing.total, divisor),
                self.renderer.commands as f64 / divisor,
                self.renderer.batches as f64 / divisor,
                self.renderer.vertices as f64 / divisor,
                self.renderer.indices as f64 / divisor,
                self.renderer.atlas_images as f64 / divisor,
                self.renderer.atlas_bytes as f64 / divisor / 1024.0,
                self.surface_reconfigurations,
                self.device_recoveries,
            );
            self.started_at = Instant::now();
            self.frames = 0;
            self.timing = GpuTiming::default();
            self.renderer = WgpuRenderStats::default();
        }
    }
}

struct WgpuHost<A: NativeApplication> {
    application: A,
    instance: wgpu::Instance,
    window: Option<Arc<Window>>,
    gpu: Option<GpuRuntime>,
    accesskit_adapter: Option<AccessKitAdapter>,
    accesskit_bridge: AccessKitBridge,
    accessibility_active: bool,
    last_accessibility_fingerprint: Option<u64>,
    last_accessibility_scale_bits: Option<u64>,
    input: WinitInputTranslator,
    frame_runtime: FrameRuntime,
    started_at: Instant,
    last_update_at: Instant,
    next_update_at: Option<Instant>,
    last_frame: Option<UiFrame>,
    proxy: EventLoopProxy<HostEvent>,
    metrics: GpuMetrics,
    fatal_error: Option<WgpuHostError>,
}

impl<A: NativeApplication> WgpuHost<A> {
    fn new(application: A, instance: wgpu::Instance, proxy: EventLoopProxy<HostEvent>) -> Self {
        Self {
            application,
            instance,
            window: None,
            gpu: None,
            accesskit_adapter: None,
            accesskit_bridge: AccessKitBridge::new(),
            accessibility_active: false,
            last_accessibility_fingerprint: None,
            last_accessibility_scale_bits: None,
            input: WinitInputTranslator::new(),
            frame_runtime: FrameRuntime::new(),
            started_at: Instant::now(),
            last_update_at: Instant::now(),
            next_update_at: None,
            last_frame: None,
            proxy,
            metrics: GpuMetrics::new(),
            fatal_error: None,
        }
    }

    fn create_native_resources(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), WgpuHostError> {
        if self.window.is_none() {
            let config = self.application.window_config();
            let mut attributes = window_attributes(config);
            attributes = attributes.with_visible(false);
            let window =
                Arc::new(event_loop.create_window(attributes).map_err(|error| {
                    WgpuHostError::from_display("failed to create window", error)
                })?);
            let adapter = AccessKitAdapter::with_event_loop_proxy(
                event_loop,
                window.as_ref(),
                self.proxy.clone(),
            );
            window.set_ime_allowed(true);
            self.accesskit_adapter = Some(adapter);
            self.window = Some(window);
        }
        if self.gpu.is_none() {
            let window = self
                .window
                .as_ref()
                .cloned()
                .ok_or_else(|| WgpuHostError::message("window missing while creating GPU"))?;
            self.gpu = Some(GpuRuntime::create(
                &self.instance,
                window,
                self.proxy.clone(),
            )?);
        }
        if let Some(window) = &self.window {
            window.set_visible(true);
            window.request_redraw();
        }
        self.frame_runtime
            .request_frame(InvalidationReason::InitialFrame);
        Ok(())
    }

    fn request_redraw(&mut self, reason: InvalidationReason) {
        self.frame_runtime.request_frame(reason);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn apply_control(
        &mut self,
        control: HostControl,
        event_loop: &ActiveEventLoop,
        default_reason: InvalidationReason,
    ) {
        match control {
            HostControl::Continue => {}
            HostControl::Redraw => self.request_redraw(default_reason),
            HostControl::Invalidate(class) => {
                self.request_redraw(InvalidationReason::Classified(class));
            }
            HostControl::Exit => event_loop.exit(),
        }
    }

    fn rebuild_gpu(&mut self, reason: &str) -> Result<(), WgpuHostError> {
        eprintln!("[luna-wgpu recovery] rebuilding GPU runtime after {reason}");
        let window = self
            .window
            .as_ref()
            .cloned()
            .ok_or_else(|| WgpuHostError::message("window missing during GPU recovery"))?;
        self.gpu = Some(GpuRuntime::create(
            &self.instance,
            window,
            self.proxy.clone(),
        )?);
        Ok(())
    }

    fn recover_device_if_needed(&mut self) -> Result<(), WgpuHostError> {
        let lost = self
            .gpu
            .as_ref()
            .is_some_and(|gpu| gpu.device_lost.load(Ordering::Acquire));
        if !lost {
            return Ok(());
        }
        let detail = self
            .gpu
            .as_ref()
            .map(GpuRuntime::lost_detail)
            .unwrap_or_else(|| "unknown device loss".to_owned());
        self.rebuild_gpu(&detail)?;
        self.metrics.device_recoveries = self.metrics.device_recoveries.saturating_add(1);
        Ok(())
    }

    fn render(&mut self) -> Result<(), WgpuHostError> {
        let total_started = Instant::now();
        self.recover_device_if_needed()?;
        let window = self
            .window
            .as_ref()
            .cloned()
            .ok_or_else(|| WgpuHostError::message("redraw requested without a window"))?;
        let physical = window.inner_size();
        if physical.width == 0 || physical.height == 0 {
            return Ok(());
        }
        if !self.frame_runtime.has_pending_frame() {
            self.frame_runtime
                .request_frame(InvalidationReason::SurfaceExposed);
        }
        let Some(token) = self.frame_runtime.begin_frame(self.now_micros()) else {
            return Ok(());
        };
        let scale_factor = normalized_scale_factor(window.scale_factor());
        let viewport = RectI::new(
            0,
            0,
            physical_extent_to_logical(physical.width, scale_factor),
            physical_extent_to_logical(physical.height, scale_factor),
        );
        let application_started = Instant::now();
        let frame = self.application.build_frame(viewport).map_err(|error| {
            WgpuHostError::from_display("application frame build failed", error)
        })?;
        let application_build = application_started.elapsed();

        let size = SizeI::new(physical.width, physical.height);
        let acquire = {
            let gpu = self
                .gpu
                .as_mut()
                .ok_or_else(|| WgpuHostError::message("GPU runtime missing during render"))?;
            if gpu.config.width != size.width || gpu.config.height != size.height {
                gpu.resize(size);
                self.metrics.surface_reconfigurations =
                    self.metrics.surface_reconfigurations.saturating_add(1);
            }
            gpu.surface.get_current_texture()
        };
        let surface_texture = match acquire {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(_) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.resize(size);
                }
                self.metrics.surface_reconfigurations =
                    self.metrics.surface_reconfigurations.saturating_add(1);
                self.request_redraw(InvalidationReason::SurfaceResized);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.resize(size);
                }
                self.metrics.surface_reconfigurations =
                    self.metrics.surface_reconfigurations.saturating_add(1);
                self.request_redraw(InvalidationReason::SurfaceResized);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.rebuild_gpu("surface loss")?;
                self.metrics.surface_reconfigurations =
                    self.metrics.surface_reconfigurations.saturating_add(1);
                self.request_redraw(InvalidationReason::SurfaceResized);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                self.request_redraw(InvalidationReason::SurfaceExposed);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(WgpuHostError::message(
                    "wgpu surface acquisition reported a validation failure",
                ));
            }
        };

        let encode_started = Instant::now();
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let gpu = self
            .gpu
            .as_mut()
            .ok_or_else(|| WgpuHostError::message("GPU runtime missing during encoding"))?;
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Luna UI Rust frame encoder"),
            });
        let mut layers = Vec::with_capacity(2);
        if let Some(retained) = frame.retained_display_list.as_ref() {
            layers.push(retained.display_list.as_ref());
        }
        layers.push(&frame.display_list);
        let renderer_stats = gpu
            .renderer
            .render_layers(
                &gpu.device,
                &gpu.queue,
                &mut encoder,
                &view,
                &layers,
                size,
                scale_factor,
            )
            .map_err(map_render_error)?;
        gpu.queue.submit([encoder.finish()]);
        let encode = encode_started.elapsed();

        let presentation_started = Instant::now();
        window.pre_present_notify();
        surface_texture.present();
        let presentation = presentation_started.elapsed();

        let accessibility_started = Instant::now();
        self.update_accessibility_if_needed(&frame, scale_factor);
        let accessibility = accessibility_started.elapsed();
        self.last_frame = Some(frame);
        let _ = FrameRuntime::finish_frame(&token, self.now_micros());
        self.metrics.record(
            GpuTiming {
                application_build,
                encode,
                presentation,
                accessibility,
                total: total_started.elapsed(),
            },
            renderer_stats,
        );
        Ok(())
    }

    fn update_accessibility_if_needed(&mut self, frame: &UiFrame, scale_factor: f64) {
        if !self.accessibility_active {
            return;
        }
        let fingerprint = frame.accessibility_tree.fingerprint();
        let scale_bits = scale_factor.to_bits();
        if self.last_accessibility_fingerprint == Some(fingerprint)
            && self.last_accessibility_scale_bits == Some(scale_bits)
        {
            return;
        }
        let update = self
            .accesskit_bridge
            .full_update(frame.accessibility_tree.as_ref(), scale_factor);
        if let Some(adapter) = self.accesskit_adapter.as_mut() {
            adapter.update_if_active(|| update);
        }
        self.last_accessibility_fingerprint = Some(fingerprint);
        self.last_accessibility_scale_bits = Some(scale_bits);
    }

    fn handle_accesskit_event(&mut self, event: AccessKitEvent, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };
        if event.window_id != window.id() {
            return;
        }
        match event.window_event {
            accesskit_winit::WindowEvent::InitialTreeRequested => {
                self.accessibility_active = true;
                if let Some(frame) = &self.last_frame {
                    let tree = Arc::clone(&frame.accessibility_tree);
                    let scale_factor = normalized_scale_factor(window.scale_factor());
                    let update = self
                        .accesskit_bridge
                        .full_update(tree.as_ref(), scale_factor);
                    if let Some(adapter) = self.accesskit_adapter.as_mut() {
                        adapter.update_if_active(|| update);
                    }
                    self.last_accessibility_fingerprint = Some(tree.fingerprint());
                    self.last_accessibility_scale_bits = Some(scale_factor.to_bits());
                } else {
                    self.request_redraw(InvalidationReason::AccessibilityChanged);
                }
            }
            accesskit_winit::WindowEvent::ActionRequested(request) => {
                let target = self
                    .accesskit_bridge
                    .luna_id_for(request.target_node)
                    .cloned();
                let kind = match request.action {
                    Action::Click => AccessibilityActionKind::Click,
                    Action::Focus => AccessibilityActionKind::Focus,
                    _ => AccessibilityActionKind::Other,
                };
                let control = self
                    .application
                    .handle_accessibility_action(AccessibilityActionRequest { target, kind });
                self.apply_control(
                    control,
                    event_loop,
                    InvalidationReason::AccessibilityChanged,
                );
            }
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {
                self.accessibility_active = false;
                self.last_accessibility_fingerprint = None;
                self.last_accessibility_scale_bits = None;
            }
        }
    }

    fn schedule_application_update(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() || self.gpu.is_none() {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        let Some(interval) = self.application.frame_interval() else {
            self.next_update_at = None;
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        };
        let interval = interval.max(Duration::from_millis(1));
        let now = Instant::now();
        let deadline = self.next_update_at.unwrap_or(now);
        if now >= deadline {
            let elapsed = now.saturating_duration_since(self.last_update_at);
            self.last_update_at = now;
            self.next_update_at = Some(now + interval);
            let control = self.application.update(elapsed);
            self.apply_control(control, event_loop, InvalidationReason::Animation);
        }
        if let Some(next) = self.next_update_at {
            event_loop.set_control_flow(ControlFlow::WaitUntil(next));
        }
    }

    fn now_micros(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_micros()).unwrap_or(u64::MAX)
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: WgpuHostError) {
        self.fatal_error = Some(error);
        event_loop.exit();
    }
}

impl<A: NativeApplication> ApplicationHandler<HostEvent> for WgpuHost<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.create_native_resources(event_loop) {
            self.fail(event_loop, error);
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.gpu = None;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };
        if window_id != window.id() {
            return;
        }
        if let Some(adapter) = self.accesskit_adapter.as_mut() {
            adapter.process_event(window.as_ref(), &event);
        }
        match &event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.request_redraw(InvalidationReason::SurfaceResized);
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.render() {
                    self.fail(event_loop, error);
                }
                return;
            }
            _ => {}
        }
        if let Some(input) = self.input.translate(&event, window.scale_factor()) {
            let control = self.application.handle_input(input);
            self.apply_control(control, event_loop, InvalidationReason::FullFrame);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: HostEvent) {
        match event {
            HostEvent::AccessKit(event) => self.handle_accesskit_event(event, event_loop),
            HostEvent::DeviceLost => self.request_redraw(InvalidationReason::SurfaceExposed),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.schedule_application_update(event_loop);
    }
}

fn window_attributes(config: WindowConfig) -> winit::window::WindowAttributes {
    let mut attributes = Window::default_attributes()
        .with_title(config.title)
        .with_inner_size(LogicalSize::new(
            f64::from(config.initial_size.width),
            f64::from(config.initial_size.height),
        ));
    if let Some(minimum) = config.minimum_size {
        attributes = attributes.with_min_inner_size(LogicalSize::new(
            f64::from(minimum.width),
            f64::from(minimum.height),
        ));
    }
    attributes
}

fn map_render_error(error: WgpuRenderError) -> WgpuHostError {
    WgpuHostError::from_display("wgpu display-list rendering failed", error)
}

fn normalized_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

fn physical_extent_to_logical(extent: u32, scale_factor: f64) -> u32 {
    let value = (f64::from(extent) / normalized_scale_factor(scale_factor)).round();
    if value <= 0.0 {
        0
    } else if value >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        value as u32
    }
}

fn millis(duration: Duration, divisor: f64) -> f64 {
    duration.as_secs_f64() * 1_000.0 / divisor
}

#[cfg(test)]
mod tests {
    use super::{normalized_scale_factor, physical_extent_to_logical};

    #[test]
    fn logical_extent_conversion_matches_cpu_host() {
        assert_eq!(physical_extent_to_logical(1_440, 1.5), 960);
        assert_eq!(physical_extent_to_logical(960, f64::NAN), 960);
        assert_eq!(normalized_scale_factor(0.0), 1.0);
    }
}
