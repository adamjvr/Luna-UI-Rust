// SPDX-License-Identifier: MPL-2.0

use crate::WinitInputTranslator;
use accesskit::Action;
use accesskit_winit::{Adapter as AccessKitAdapter, Event as AccessKitEvent};
use luna_accessibility_accesskit::AccessKitBridge;
use luna_core::{NodeId, RectI, SizeI};
use luna_host_core::{FrameRuntime, InvalidationReason};
use luna_input::InputEvent;
use luna_render::{CpuRenderer, Framebuffer};
use luna_ui::UiFrame;
use softbuffer::{Context, Surface};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy, OwnedDisplayHandle};
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
    /// Invalidate application state and request a redraw.
    Redraw,
    /// Exit the native event loop.
    Exit,
}

/// Platform-neutral category of a native accessibility action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessibilityActionKind {
    /// Activate a semantic button or control.
    Click,
    /// Move semantic/keyboard focus to a node.
    Focus,
    /// An AccessKit action not yet represented by Luna's M1 host contract.
    Other,
}

/// Accessibility action translated back to Luna's stable semantic identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessibilityActionRequest {
    /// Stable Luna target, when the AccessKit ID has appeared in a prior frame.
    pub target: Option<NodeId>,
    /// Product-neutral action category.
    pub kind: AccessibilityActionKind,
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

    /// Handles one normalized input event.
    fn handle_input(&mut self, _event: InputEvent) -> HostControl {
        HostControl::Continue
    }

    /// Handles an assistive-technology action routed through AccessKit.
    fn handle_accessibility_action(&mut self, _request: AccessibilityActionRequest) -> HostControl {
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

struct WinitHost<A: NativeApplication> {
    application: A,
    context: Context<OwnedDisplayHandle>,
    surface: Option<Surface<OwnedDisplayHandle, Arc<Window>>>,
    window: Option<Arc<Window>>,
    accesskit_adapter: Option<AccessKitAdapter>,
    accesskit_bridge: AccessKitBridge,
    input: WinitInputTranslator,
    frame_runtime: FrameRuntime,
    started_at: Instant,
    last_frame: Option<UiFrame>,
    proxy: EventLoopProxy<HostEvent>,
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
            window: None,
            accesskit_adapter: None,
            accesskit_bridge: AccessKitBridge::new(),
            input: WinitInputTranslator::new(),
            frame_runtime: FrameRuntime::new(),
            started_at: Instant::now(),
            last_frame: None,
            proxy,
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
            window.set_ime_allowed(true);
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

    fn apply_control(&mut self, control: HostControl, event_loop: &ActiveEventLoop) {
        match control {
            HostControl::Continue => {}
            HostControl::Redraw => self.request_redraw(InvalidationReason::StateChanged),
            HostControl::Exit => event_loop.exit(),
        }
    }

    fn render(&mut self) -> Result<(), HostError> {
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
        let now_micros = self.now_micros();
        self.frame_runtime
            .request_frame(InvalidationReason::Explicit(
                "native-redraw-request".to_owned(),
            ));
        let Some(token) = self.frame_runtime.begin_frame(now_micros) else {
            return Ok(());
        };

        let scale_factor = normalized_scale_factor(window.scale_factor());
        let logical_width = physical_extent_to_logical(physical_size.width, scale_factor);
        let logical_height = physical_extent_to_logical(physical_size.height, scale_factor);
        let viewport = RectI::new(0, 0, logical_width, logical_height);
        let frame = self
            .application
            .build_frame(viewport)
            .map_err(|error| HostError::from_display("application frame build failed", error))?;
        let mut framebuffer =
            Framebuffer::new(SizeI::new(physical_size.width, physical_size.height))
                .map_err(|error| HostError::from_display("framebuffer allocation failed", error))?;
        CpuRenderer::render_scaled(&frame.display_list, &mut framebuffer, scale_factor);

        let surface = self
            .surface
            .as_mut()
            .ok_or_else(|| HostError::message("redraw requested without a presentation surface"))?;
        surface
            .resize(width, height)
            .map_err(|error| HostError::from_display("surface resize failed", error))?;
        let mut buffer = surface
            .buffer_mut()
            .map_err(|error| HostError::from_display("surface buffer acquisition failed", error))?;
        for (destination, source) in buffer.iter_mut().zip(framebuffer.bytes().chunks_exact(4)) {
            // Softbuffer uses 0x00RRGGBB while Luna's reference framebuffer is BGRA8.
            *destination =
                u32::from(source[0]) | (u32::from(source[1]) << 8) | (u32::from(source[2]) << 16);
        }
        buffer
            .present()
            .map_err(|error| HostError::from_display("surface presentation failed", error))?;

        let accessibility_update = self
            .accesskit_bridge
            .full_update(&frame.accessibility_tree, scale_factor);
        if let Some(adapter) = self.accesskit_adapter.as_mut() {
            adapter.update_if_active(|| accessibility_update);
        }
        self.last_frame = Some(frame);
        let _stats = FrameRuntime::finish_frame(&token, self.now_micros());
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
                if let Some(frame) = &self.last_frame {
                    let update = self
                        .accesskit_bridge
                        .full_update(&frame.accessibility_tree, window.scale_factor());
                    if let Some(adapter) = self.accesskit_adapter.as_mut() {
                        adapter.update_if_active(|| update);
                    }
                } else {
                    self.request_redraw(InvalidationReason::Explicit(
                        "accessibility-initial-tree".to_owned(),
                    ));
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
                self.apply_control(control, event_loop);
            }
            accesskit_winit::WindowEvent::AccessibilityDeactivated => {}
        }
    }

    fn now_micros(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_micros()).unwrap_or(u64::MAX)
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
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        // On mobile platforms a graphics surface may become invalid while suspended. The Arc-held
        // window identity is retained, but the presentation surface is recreated on resume.
        self.surface = None;
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
            self.apply_control(control, event_loop);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: HostEvent) {
        match event {
            HostEvent::AccessKit(event) => self.handle_accesskit_event(event, event_loop),
        }
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
    use super::{WindowConfig, normalized_scale_factor, physical_extent_to_logical};
    use luna_core::SizeI;

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
}
