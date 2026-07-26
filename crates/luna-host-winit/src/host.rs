// SPDX-License-Identifier: MPL-2.0

use crate::WinitInputTranslator;
use accesskit::{Action, ActionData};
use accesskit_winit::{Adapter as AccessKitAdapter, Event as AccessKitEvent};
use luna_accessibility_accesskit::AccessKitBridge;
use luna_core::{NodeId, RectI, SizeI};
use luna_host_core::{FrameRuntime, FrameToken, InvalidationClass, InvalidationReason};
use luna_input::InputEvent;
use luna_render::{CpuRenderer, Framebuffer};
use luna_ui::UiFrame;
use softbuffer::{Context, Surface};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{
    ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy, OwnedDisplayHandle,
};
use winit::window::{Window, WindowId};

/// Error type applications may return while constructing an immutable Luna frame.
pub type ApplicationError = Box<dyn Error + Send + Sync + 'static>;

/// Native window configuration requested by a Luna application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowConfig {
    /// Native window title.
    pub title: String,
    /// Initial client size in Luna logical pixels.
    pub initial_size: SizeI,
    /// Optional minimum client size in Luna logical pixels.
    pub minimum_size: Option<SizeI>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Luna UI Rust".to_owned(),
            initial_size: SizeI::new(960, 540),
            minimum_size: Some(SizeI::new(480, 270)),
        }
    }
}

/// Result requested after an application handles native input or an accessibility action.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HostControl {
    /// Keep waiting without forcing a new frame.
    #[default]
    Continue,
    /// Request a redraw using the host's context-dependent default invalidation reason.
    Redraw,
    /// Request a redraw using a precise application-selected invalidation class.
    Invalidate(InvalidationClass),
    /// Exit the native event loop.
    Exit,
}

/// Platform-neutral application lifecycle event delivered by native hosts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeLifecycleEvent {
    /// Native resources are available and the application is active.
    Resumed,
    /// Native presentation resources are being suspended or invalidated.
    Suspended,
    /// The operating system requested aggressive cache/resource release.
    MemoryWarning,
}

/// Platform-neutral category of a native accessibility action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessibilityActionKind {
    /// Activate a semantic button or control.
    Click,
    /// Move semantic/keyboard focus to a node.
    Focus,
    /// Replace the selected text with supplied UTF-8 content.
    ReplaceSelectedText,
    /// Replace a control's complete textual value.
    SetValue,
    /// Request the target's context menu.
    ShowContextMenu,
    /// Increment a range-like control.
    Increment,
    /// Decrement a range-like control.
    Decrement,
    /// An AccessKit action not yet represented by Luna's host contract.
    Other,
}

/// Optional data carried by a native accessibility request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessibilityActionData {
    /// No supported payload was supplied.
    None,
    /// UTF-8 replacement or value text.
    Value(String),
}

/// Accessibility action translated back to Luna's stable semantic identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessibilityActionRequest {
    /// Stable Luna target, when the AccessKit ID has appeared in a prior frame.
    pub target: Option<NodeId>,
    /// Product-neutral action category.
    pub kind: AccessibilityActionKind,
    /// Product-neutral action payload.
    pub data: AccessibilityActionData,
}

/// Application contract driven by [`run_native`].
///
/// The host owns winit, softbuffer, DPI conversion, event-loop lifecycle, and AccessKit plumbing.
/// The application owns state and produces immutable [`UiFrame`] snapshots. This keeps operating
/// system types out of reusable widgets and application command logic.
pub trait NativeApplication {
    /// Returns the desired native window configuration.
    fn window_config(&self) -> WindowConfig {
        WindowConfig::default()
    }

    /// Builds one complete immutable frame for the supplied logical viewport.
    fn build_frame(&mut self, viewport: RectI) -> Result<UiFrame, ApplicationError>;

    /// Returns whether the focused application surface accepts native text input.
    fn accepts_text_input(&self) -> bool {
        false
    }

    /// Returns the logical candidate-window anchor for native IME UI.
    fn ime_cursor_area(&self) -> Option<RectI> {
        None
    }

    /// Reports whether application state contains unsaved document changes.
    ///
    /// On macOS the native host mirrors this into the window's document-edited indicator. Other
    /// hosts may use it for diagnostics without changing close policy.
    fn has_unsaved_changes(&self) -> bool {
        false
    }

    /// Handles a native application lifecycle transition.
    fn handle_lifecycle(&mut self, _event: NativeLifecycleEvent) -> HostControl {
        HostControl::Continue
    }

    /// Resolves an operating-system window-close request.
    ///
    /// Applications may persist session state, show dirty-document policy, return `Continue` to
    /// veto closing, or return `Exit` to terminate the native loop.
    fn request_close(&mut self) -> HostControl {
        HostControl::Exit
    }

    /// Handles one normalized input event.
    fn handle_input(&mut self, _event: InputEvent) -> HostControl {
        HostControl::Continue
    }

    /// Handles an assistive-technology action routed through AccessKit.
    fn handle_accessibility_action(&mut self, _request: AccessibilityActionRequest) -> HostControl {
        HostControl::Continue
    }

    /// Returns the desired logical update cadence for animated or time-driven applications.
    ///
    /// Returning `None` keeps the native loop event-driven. Returning a duration schedules
    /// [`Self::update`] through winit's `WaitUntil` control flow without rendering outside
    /// `RedrawRequested`. Applications should prefer the slowest cadence that represents their
    /// proof or animation correctly.
    fn frame_interval(&self) -> Option<Duration> {
        None
    }

    /// Advances application-owned logical time.
    ///
    /// The host passes elapsed monotonic time since the previous update. A time-driven application
    /// normally returns [`HostControl::Redraw`]; static editor applications may retain the default.
    fn update(&mut self, _elapsed: Duration) -> HostControl {
        HostControl::Continue
    }
}

/// Native-host startup or runtime failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostError {
    message: String,
}

impl HostError {
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

impl Display for HostError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for HostError {}

/// Runs a Luna application in a native winit window using the safe CPU reference renderer.
///
/// M1 deliberately uses softbuffer as the first presentation backend. It proves window lifecycle,
/// DPI, resize, input, accessibility, and immutable frame submission before the later wgpu backend
/// adds GPU complexity.
pub fn run_native(application: impl NativeApplication + 'static) -> Result<(), HostError> {
    let event_loop = EventLoop::<HostEvent>::with_user_event()
        .build()
        .map_err(|error| HostError::from_display("failed to create winit event loop", error))?;
    let context = Context::new(event_loop.owned_display_handle())
        .map_err(|error| HostError::from_display("failed to create softbuffer context", error))?;
    let proxy = event_loop.create_proxy();
    let mut host = WinitHost::new(application, context, proxy);
    let loop_result = event_loop.run_app(&mut host);

    if let Some(error) = host.fatal_error.take() {
        return Err(error);
    }
    loop_result.map_err(|error| HostError::from_display("winit event loop failed", error))
}

#[derive(Debug)]
enum HostEvent {
    AccessKit(AccessKitEvent),
}

impl From<AccessKitEvent> for HostEvent {
    fn from(value: AccessKitEvent) -> Self {
        Self::AccessKit(value)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct FrameTimingSample {
    application_build: Duration,
    cpu_render: Duration,
    conversion: Duration,
    presentation: Duration,
    accessibility: Duration,
    total: Duration,
}

#[derive(Debug)]
struct HostMetrics {
    report_started_at: Instant,
    window_frames: u64,
    total_frames: u64,
    timing: FrameTimingSample,
    framebuffer_allocations: u64,
    surface_resizes: u64,
    accessibility_translations: u64,
    accessibility_skips: u64,
    retained_base_hits: u64,
    retained_base_misses: u64,
    retained_full_restores: u64,
    retained_region_restores: u64,
    invalidations: BTreeMap<InvalidationClass, u64>,
}

impl HostMetrics {
    fn new() -> Self {
        Self {
            report_started_at: Instant::now(),
            window_frames: 0,
            total_frames: 0,
            timing: FrameTimingSample::default(),
            framebuffer_allocations: 0,
            surface_resizes: 0,
            accessibility_translations: 0,
            accessibility_skips: 0,
            retained_base_hits: 0,
            retained_base_misses: 0,
            retained_full_restores: 0,
            retained_region_restores: 0,
            invalidations: BTreeMap::new(),
        }
    }

    fn record(&mut self, token: &FrameToken, sample: FrameTimingSample) {
        self.window_frames = self.window_frames.saturating_add(1);
        self.total_frames = self.total_frames.saturating_add(1);
        self.timing.application_build += sample.application_build;
        self.timing.cpu_render += sample.cpu_render;
        self.timing.conversion += sample.conversion;
        self.timing.presentation += sample.presentation;
        self.timing.accessibility += sample.accessibility;
        self.timing.total += sample.total;
        for reason in token.invalidations.iter() {
            let count = self.invalidations.entry(reason.class()).or_default();
            *count = count.saturating_add(1);
        }

        if self.total_frames == 1 || self.report_started_at.elapsed() >= Duration::from_secs(1) {
            self.report();
        }
    }

    fn report(&mut self) {
        let frames = self.window_frames;
        eprintln!(
            "[luna-host metrics] frames={frames} total_frames={} avg_ms={{app:{:.3}, render:{:.3}, convert:{:.3}, present:{:.3}, accessibility:{:.3}, total:{:.3}}} framebuffer_allocations={} surface_resizes={} accessibility={{translations:{}, skips:{}}} retained={{base_hits:{}, base_misses:{}, full_restores:{}, region_restores:{}}} invalidations={:?}",
            self.total_frames,
            average_millis(self.timing.application_build, frames),
            average_millis(self.timing.cpu_render, frames),
            average_millis(self.timing.conversion, frames),
            average_millis(self.timing.presentation, frames),
            average_millis(self.timing.accessibility, frames),
            average_millis(self.timing.total, frames),
            self.framebuffer_allocations,
            self.surface_resizes,
            self.accessibility_translations,
            self.accessibility_skips,
            self.retained_base_hits,
            self.retained_base_misses,
            self.retained_full_restores,
            self.retained_region_restores,
            self.invalidations,
        );
        self.report_started_at = Instant::now();
        self.window_frames = 0;
        self.timing = FrameTimingSample::default();
        self.invalidations.clear();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RetainedFrameSignature {
    revision: u64,
    size: SizeI,
    scale_factor_bits: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetainedFramebufferCache {
    signature: RetainedFrameSignature,
    framebuffer: Framebuffer,
}

struct WinitHost<A: NativeApplication> {
    application: A,
    context: Context<OwnedDisplayHandle>,
    surface: Option<Surface<OwnedDisplayHandle, Arc<Window>>>,
    surface_size: Option<SizeI>,
    window: Option<Arc<Window>>,
    framebuffer: Option<Framebuffer>,
    retained_framebuffer: Option<RetainedFramebufferCache>,
    retained_working_signature: Option<RetainedFrameSignature>,
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
    metrics: HostMetrics,
    fatal_error: Option<HostError>,
}

impl<A: NativeApplication> WinitHost<A> {
    fn new(
        application: A,
        context: Context<OwnedDisplayHandle>,
        proxy: EventLoopProxy<HostEvent>,
    ) -> Self {
        Self {
            application,
            context,
            surface: None,
            surface_size: None,
            window: None,
            framebuffer: None,
            retained_framebuffer: None,
            retained_working_signature: None,
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
            metrics: HostMetrics::new(),
            fatal_error: None,
        }
    }

    fn create_native_resources(&mut self, event_loop: &ActiveEventLoop) -> Result<(), HostError> {
        if self.window.is_none() {
            let config = self.application.window_config();
            let mut attributes = Window::default_attributes()
                .with_title(config.title)
                .with_inner_size(LogicalSize::new(
                    f64::from(config.initial_size.width),
                    f64::from(config.initial_size.height),
                ))
                // AccessKit requires adapter creation before the window is first shown.
                .with_visible(false);
            if let Some(minimum) = config.minimum_size {
                attributes = attributes.with_min_inner_size(LogicalSize::new(
                    f64::from(minimum.width),
                    f64::from(minimum.height),
                ));
            }

            let window = Arc::new(
                event_loop
                    .create_window(attributes)
                    .map_err(|error| HostError::from_display("failed to create window", error))?,
            );
            let adapter = AccessKitAdapter::with_event_loop_proxy(
                event_loop,
                window.as_ref(),
                self.proxy.clone(),
            );
            update_macos_document_edited(window.as_ref(), self.application.has_unsaved_changes());
            window.set_ime_allowed(self.application.accepts_text_input());
            self.accesskit_adapter = Some(adapter);
            self.window = Some(window);
        }

        if self.surface.is_none() {
            let window = self
                .window
                .as_ref()
                .cloned()
                .ok_or_else(|| HostError::message("window missing while creating surface"))?;
            self.surface = Some(
                Surface::new(&self.context, window)
                    .map_err(|error| HostError::from_display("failed to create surface", error))?,
            );
            self.surface_size = None;
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

    fn ensure_framebuffer(&mut self, size: SizeI) -> Result<&mut Framebuffer, HostError> {
        let must_allocate = self
            .framebuffer
            .as_ref()
            .is_none_or(|framebuffer| framebuffer.size() != size);
        if must_allocate {
            self.framebuffer = Some(Framebuffer::new(size).map_err(|error| {
                HostError::from_display("framebuffer allocation failed", error)
            })?);
            self.metrics.framebuffer_allocations =
                self.metrics.framebuffer_allocations.saturating_add(1);
            self.retained_working_signature = None;
        }
        self.framebuffer
            .as_mut()
            .ok_or_else(|| HostError::message("retained framebuffer missing after allocation"))
    }

    fn render_ui_frame(
        &mut self,
        frame: &UiFrame,
        framebuffer_size: SizeI,
        scale_factor: f64,
    ) -> Result<(), HostError> {
        let Some(retained) = frame.retained_display_list.as_ref() else {
            let framebuffer = self.ensure_framebuffer(framebuffer_size)?;
            framebuffer.reset();
            CpuRenderer::render_scaled(&frame.display_list, framebuffer, scale_factor);
            self.retained_working_signature = None;
            return Ok(());
        };

        let signature = RetainedFrameSignature {
            revision: retained.revision,
            size: framebuffer_size,
            scale_factor_bits: scale_factor.to_bits(),
        };
        let base_matches = self
            .retained_framebuffer
            .as_ref()
            .is_some_and(|cache| cache.signature == signature);
        if base_matches {
            self.metrics.retained_base_hits = self.metrics.retained_base_hits.saturating_add(1);
        } else {
            let mut framebuffer = match self.retained_framebuffer.take() {
                Some(cache) if cache.framebuffer.size() == framebuffer_size => cache.framebuffer,
                _ => Framebuffer::new(framebuffer_size).map_err(|error| {
                    HostError::from_display("retained framebuffer allocation failed", error)
                })?,
            };
            framebuffer.reset();
            CpuRenderer::render_scaled(
                retained.display_list.as_ref(),
                &mut framebuffer,
                scale_factor,
            );
            self.retained_framebuffer = Some(RetainedFramebufferCache {
                signature,
                framebuffer,
            });
            self.metrics.retained_base_misses = self.metrics.retained_base_misses.saturating_add(1);
        }

        let _ = self.ensure_framebuffer(framebuffer_size)?;
        let working_matches = base_matches && self.retained_working_signature == Some(signature);
        {
            let base = &self
                .retained_framebuffer
                .as_ref()
                .ok_or_else(|| HostError::message("retained framebuffer missing after render"))?
                .framebuffer;
            let framebuffer = self.framebuffer.as_mut().ok_or_else(|| {
                HostError::message("working framebuffer missing after allocation")
            })?;
            if working_matches {
                let dirty_bounds =
                    CpuRenderer::scale_logical_rect(retained.dirty_bounds, scale_factor);
                let _copied = framebuffer.copy_rect_from(base, dirty_bounds);
            } else {
                let _copied = framebuffer.copy_from(base);
            }
            CpuRenderer::render_scaled(&frame.display_list, framebuffer, scale_factor);
        }
        if working_matches {
            self.metrics.retained_region_restores =
                self.metrics.retained_region_restores.saturating_add(1);
        } else {
            self.metrics.retained_full_restores =
                self.metrics.retained_full_restores.saturating_add(1);
        }
        self.retained_working_signature = Some(signature);
        Ok(())
    }

    fn update_accessibility_if_needed(&mut self, frame: &UiFrame, scale_factor: f64) {
        if !self.accessibility_active {
            self.metrics.accessibility_skips = self.metrics.accessibility_skips.saturating_add(1);
            return;
        }
        let fingerprint = frame.accessibility_tree.fingerprint();
        let scale_bits = scale_factor.to_bits();
        if self.last_accessibility_fingerprint == Some(fingerprint)
            && self.last_accessibility_scale_bits == Some(scale_bits)
        {
            self.metrics.accessibility_skips = self.metrics.accessibility_skips.saturating_add(1);
            return;
        }

        let update = self
            .accesskit_bridge
            .full_update(frame.accessibility_tree.as_ref(), scale_factor);
        if let Some(adapter) = self.accesskit_adapter.as_mut() {
            adapter.update_if_active(|| update);
        }
        self.metrics.accessibility_translations =
            self.metrics.accessibility_translations.saturating_add(1);
        self.last_accessibility_fingerprint = Some(fingerprint);
        self.last_accessibility_scale_bits = Some(scale_bits);
    }

    fn ensure_surface_size(
        &mut self,
        width: NonZeroU32,
        height: NonZeroU32,
    ) -> Result<(), HostError> {
        let size = SizeI::new(width.get(), height.get());
        if self.surface_size == Some(size) {
            return Ok(());
        }
        self.surface
            .as_mut()
            .ok_or_else(|| HostError::message("surface missing while resizing"))?
            .resize(width, height)
            .map_err(|error| HostError::from_display("surface resize failed", error))?;
        self.surface_size = Some(size);
        self.metrics.surface_resizes = self.metrics.surface_resizes.saturating_add(1);
        Ok(())
    }

    fn render(&mut self) -> Result<(), HostError> {
        let total_started = Instant::now();
        let window = self
            .window
            .as_ref()
            .cloned()
            .ok_or_else(|| HostError::message("redraw requested without a window"))?;
        let physical_size = window.inner_size();
        let Some(width) = NonZeroU32::new(physical_size.width) else {
            return Ok(());
        };
        let Some(height) = NonZeroU32::new(physical_size.height) else {
            return Ok(());
        };
        if !self.frame_runtime.has_pending_frame() {
            self.frame_runtime
                .request_frame(InvalidationReason::SurfaceExposed);
        }
        let Some(token) = self.frame_runtime.begin_frame(self.now_micros()) else {
            return Ok(());
        };

        let scale_factor = normalized_scale_factor(window.scale_factor());
        let logical_width = physical_extent_to_logical(physical_size.width, scale_factor);
        let logical_height = physical_extent_to_logical(physical_size.height, scale_factor);
        let viewport = RectI::new(0, 0, logical_width, logical_height);

        let application_started = Instant::now();
        let frame = self
            .application
            .build_frame(viewport)
            .map_err(|error| HostError::from_display("application frame build failed", error))?;
        let application_build = application_started.elapsed();
        update_macos_document_edited(window.as_ref(), self.application.has_unsaved_changes());
        window.set_ime_allowed(self.application.accepts_text_input());
        if let Some(area) = self.application.ime_cursor_area() {
            window.set_ime_cursor_area(
                LogicalPosition::new(f64::from(area.x), f64::from(area.y)),
                LogicalSize::new(f64::from(area.width.max(1)), f64::from(area.height.max(1))),
            );
        }

        let render_started = Instant::now();
        let framebuffer_size = SizeI::new(physical_size.width, physical_size.height);
        self.render_ui_frame(&frame, framebuffer_size, scale_factor)?;
        let cpu_render = render_started.elapsed();

        self.ensure_surface_size(width, height)?;
        let conversion_started = Instant::now();
        let mut buffer = self
            .surface
            .as_mut()
            .ok_or_else(|| HostError::message("redraw requested without a presentation surface"))?
            .buffer_mut()
            .map_err(|error| HostError::from_display("surface buffer acquisition failed", error))?;
        let framebuffer = self
            .framebuffer
            .as_ref()
            .ok_or_else(|| HostError::message("retained framebuffer missing during conversion"))?;
        let _written = framebuffer.copy_xrgb8888(&mut buffer);
        let conversion = conversion_started.elapsed();

        let presentation_started = Instant::now();
        buffer
            .present()
            .map_err(|error| HostError::from_display("surface presentation failed", error))?;
        let presentation = presentation_started.elapsed();

        let accessibility_started = Instant::now();
        self.update_accessibility_if_needed(&frame, scale_factor);
        let accessibility = accessibility_started.elapsed();

        self.last_frame = Some(frame);
        let _stats = FrameRuntime::finish_frame(&token, self.now_micros());
        self.metrics.record(
            &token,
            FrameTimingSample {
                application_build,
                cpu_render,
                conversion,
                presentation,
                accessibility,
                total: total_started.elapsed(),
            },
        );
        Ok(())
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
                    self.metrics.accessibility_translations =
                        self.metrics.accessibility_translations.saturating_add(1);
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
                    Action::ReplaceSelectedText => AccessibilityActionKind::ReplaceSelectedText,
                    Action::SetValue => AccessibilityActionKind::SetValue,
                    Action::ShowContextMenu => AccessibilityActionKind::ShowContextMenu,
                    Action::Increment => AccessibilityActionKind::Increment,
                    Action::Decrement => AccessibilityActionKind::Decrement,
                    _ => AccessibilityActionKind::Other,
                };
                let data = match request.data {
                    Some(ActionData::Value(value)) => AccessibilityActionData::Value(value.into()),
                    _ => AccessibilityActionData::None,
                };
                let control = self
                    .application
                    .handle_accessibility_action(AccessibilityActionRequest { target, kind, data });
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

    fn now_micros(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_micros()).unwrap_or(u64::MAX)
    }

    fn schedule_application_update(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() || self.surface.is_none() {
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

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: HostError) {
        self.fatal_error = Some(error);
        event_loop.exit();
    }
}

impl<A: NativeApplication> ApplicationHandler<HostEvent> for WinitHost<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.create_native_resources(event_loop) {
            self.fail(event_loop, error);
            return;
        }
        let control = self
            .application
            .handle_lifecycle(NativeLifecycleEvent::Resumed);
        self.apply_control(control, event_loop, InvalidationReason::SurfaceExposed);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        let control = self
            .application
            .handle_lifecycle(NativeLifecycleEvent::Suspended);
        self.apply_control(control, event_loop, InvalidationReason::SurfaceExposed);
        // On mobile platforms a graphics surface may become invalid while suspended. The Arc-held
        // window identity is retained, but the presentation surface is recreated on resume.
        self.surface = None;
        self.surface_size = None;
        self.retained_framebuffer = None;
        self.retained_working_signature = None;
    }

    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        self.retained_framebuffer = None;
        self.retained_working_signature = None;
        let control = self
            .application
            .handle_lifecycle(NativeLifecycleEvent::MemoryWarning);
        self.apply_control(control, event_loop, InvalidationReason::FullFrame);
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
                let control = self.application.request_close();
                self.apply_control(control, event_loop, InvalidationReason::StateChanged);
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
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.schedule_application_update(event_loop);
    }
}

#[cfg(target_os = "macos")]
fn update_macos_document_edited(window: &Window, edited: bool) {
    use winit::platform::macos::WindowExtMacOS;
    window.set_document_edited(edited);
}

#[cfg(not(target_os = "macos"))]
fn update_macos_document_edited(_window: &Window, _edited: bool) {}

fn average_millis(duration: Duration, frames: u64) -> f64 {
    if frames == 0 {
        0.0
    } else {
        duration.as_secs_f64() * 1_000.0 / frames as f64
    }
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

#[cfg(test)]
mod tests {
    use super::{
        HostControl, WindowConfig, average_millis, normalized_scale_factor,
        physical_extent_to_logical,
    };
    use luna_core::SizeI;
    use std::time::Duration;

    #[test]
    fn default_window_configuration_is_useful() {
        let config = WindowConfig::default();
        assert_eq!(config.initial_size, SizeI::new(960, 540));
        assert!(config.minimum_size.is_some());
    }

    #[test]
    fn physical_extents_convert_at_fractional_scale() {
        assert_eq!(physical_extent_to_logical(1440, 1.5), 960);
        assert_eq!(physical_extent_to_logical(960, f64::NAN), 960);
        assert_eq!(normalized_scale_factor(0.0), 1.0);
    }

    #[test]
    fn metrics_average_is_reported_per_frame() {
        assert_eq!(average_millis(Duration::from_millis(6), 3), 2.0);
        assert_eq!(average_millis(Duration::from_millis(6), 0), 0.0);
    }

    #[test]
    fn host_control_remains_copyable() {
        fn require_copy<T: Copy>() {}

        require_copy::<HostControl>();
    }
}
