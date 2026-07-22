// SPDX-License-Identifier: MPL-2.0

use luna_core::PointI;
use luna_input::{
    InputEvent, Key, KeyboardEvent, Modifiers, NamedKey, PointerButton, PointerEvent,
    PointerEventKind, ScrollEvent,
};
use std::time::Instant;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};

const LOGICAL_PIXELS_PER_SCROLL_LINE: f64 = 40.0;

/// Stateful translator from winit window events to Luna's platform-neutral input model.
///
/// The translator owns sampled modifier and pointer state because several native events do not
/// repeat that information. All coordinates crossing this boundary are converted from physical
/// pixels to Luna logical pixels using the current window scale factor.
#[derive(Debug)]
pub struct WinitInputTranslator {
    started_at: Instant,
    modifiers: Modifiers,
    pointer_position: PointI,
}

impl Default for WinitInputTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl WinitInputTranslator {
    /// Creates an input translator with an independent monotonic timestamp origin.
    #[must_use]
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            modifiers: Modifiers::NONE,
            pointer_position: PointI::default(),
        }
    }

    /// Translates one winit event when it represents product-neutral Luna input.
    ///
    /// Some winit events only update translator state and therefore return `None`. Window lifecycle
    /// and resize events remain the responsibility of the native host.
    pub fn translate(&mut self, event: &WindowEvent, scale_factor: f64) -> Option<InputEvent> {
        let timestamp_micros = self.timestamp_micros();
        match event {
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = map_modifiers(modifiers.state());
                None
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer_position = PointI::new(
                    physical_to_logical(position.x, scale_factor),
                    physical_to_logical(position.y, scale_factor),
                );
                Some(InputEvent::Pointer(PointerEvent {
                    kind: PointerEventKind::Moved,
                    position: self.pointer_position,
                    modifiers: self.modifiers,
                    timestamp_micros,
                }))
            }
            WindowEvent::CursorLeft { .. } => Some(InputEvent::Pointer(PointerEvent {
                kind: PointerEventKind::Left,
                position: self.pointer_position,
                modifiers: self.modifiers,
                timestamp_micros,
            })),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = map_pointer_button(*button);
                let kind = match state {
                    ElementState::Pressed => PointerEventKind::Pressed(button),
                    ElementState::Released => PointerEventKind::Released(button),
                };
                Some(InputEvent::Pointer(PointerEvent {
                    kind,
                    position: self.pointer_position,
                    modifiers: self.modifiers,
                    timestamp_micros,
                }))
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (delta_x, delta_y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (
                        f64::from(*x) * LOGICAL_PIXELS_PER_SCROLL_LINE,
                        f64::from(*y) * LOGICAL_PIXELS_PER_SCROLL_LINE,
                    ),
                    MouseScrollDelta::PixelDelta(position) => {
                        let scale = normalized_scale_factor(scale_factor);
                        (position.x / scale, position.y / scale)
                    }
                };
                Some(InputEvent::Scroll(ScrollEvent {
                    delta_x: clamped_i32(delta_x.round()),
                    delta_y: clamped_i32(delta_y.round()),
                    modifiers: self.modifiers,
                    timestamp_micros,
                }))
            }
            WindowEvent::KeyboardInput { event, .. } => Some(InputEvent::Keyboard(KeyboardEvent {
                key: map_key(&event.logical_key),
                is_pressed: event.state == ElementState::Pressed,
                is_repeat: event.repeat,
                modifiers: self.modifiers,
                timestamp_micros,
            })),
            WindowEvent::Ime(Ime::Commit(text)) if !text.is_empty() => {
                Some(InputEvent::Text(text.clone()))
            }
            WindowEvent::Focused(true) => Some(InputEvent::FocusGained),
            WindowEvent::Focused(false) => Some(InputEvent::FocusLost),
            _ => None,
        }
    }

    fn timestamp_micros(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_micros()).unwrap_or(u64::MAX)
    }
}

fn map_modifiers(state: ModifiersState) -> Modifiers {
    let mut result = Modifiers::NONE;
    if state.shift_key() {
        result = result.union(Modifiers::SHIFT);
    }
    if state.control_key() {
        result = result.union(Modifiers::CONTROL);
    }
    if state.alt_key() {
        result = result.union(Modifiers::ALT);
    }
    if state.super_key() {
        result = result.union(Modifiers::SUPER);
    }
    result
}

fn map_pointer_button(button: MouseButton) -> PointerButton {
    match button {
        MouseButton::Left => PointerButton::Primary,
        MouseButton::Right => PointerButton::Secondary,
        MouseButton::Middle => PointerButton::Middle,
        MouseButton::Back => PointerButton::Other(4),
        MouseButton::Forward => PointerButton::Other(5),
        MouseButton::Other(value) => PointerButton::Other(value),
    }
}

fn map_key(key: &WinitKey) -> Key {
    match key {
        WinitKey::Named(named) => map_named_key(*named).map_or(Key::Unidentified, Key::Named),
        WinitKey::Character(value) => Key::Character(value.as_str().to_owned()),
        WinitKey::Unidentified(_) | WinitKey::Dead(_) => Key::Unidentified,
    }
}

fn map_named_key(key: WinitNamedKey) -> Option<NamedKey> {
    match key {
        WinitNamedKey::Escape => Some(NamedKey::Escape),
        WinitNamedKey::Enter => Some(NamedKey::Enter),
        WinitNamedKey::Tab => Some(NamedKey::Tab),
        WinitNamedKey::Backspace => Some(NamedKey::Backspace),
        WinitNamedKey::Delete => Some(NamedKey::Delete),
        WinitNamedKey::ArrowLeft => Some(NamedKey::ArrowLeft),
        WinitNamedKey::ArrowRight => Some(NamedKey::ArrowRight),
        WinitNamedKey::ArrowUp => Some(NamedKey::ArrowUp),
        WinitNamedKey::ArrowDown => Some(NamedKey::ArrowDown),
        WinitNamedKey::Home => Some(NamedKey::Home),
        WinitNamedKey::End => Some(NamedKey::End),
        WinitNamedKey::PageUp => Some(NamedKey::PageUp),
        WinitNamedKey::PageDown => Some(NamedKey::PageDown),
        _ => None,
    }
}

fn physical_to_logical(value: f64, scale_factor: f64) -> i32 {
    clamped_i32((value / normalized_scale_factor(scale_factor)).round())
}

fn normalized_scale_factor(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    }
}

fn clamped_i32(value: f64) -> i32 {
    if value <= f64::from(i32::MIN) {
        i32::MIN
    } else if value >= f64::from(i32::MAX) {
        i32::MAX
    } else {
        value as i32
    }
}

#[cfg(test)]
mod tests {
    use super::{map_key, map_modifiers, physical_to_logical};
    use luna_input::{Key, Modifiers, NamedKey};
    use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};

    #[test]
    fn modifier_mapping_preserves_all_supported_flags() {
        let state = ModifiersState::SHIFT | ModifiersState::CONTROL | ModifiersState::ALT;
        let mapped = map_modifiers(state);

        assert!(mapped.contains(Modifiers::SHIFT));
        assert!(mapped.contains(Modifiers::CONTROL));
        assert!(mapped.contains(Modifiers::ALT));
        assert!(!mapped.contains(Modifiers::SUPER));
    }

    #[test]
    fn named_keys_map_without_platform_values_leaking() {
        assert_eq!(
            map_key(&WinitKey::Named(WinitNamedKey::Escape)),
            Key::Named(NamedKey::Escape)
        );
    }

    #[test]
    fn physical_coordinates_are_converted_once() {
        assert_eq!(physical_to_logical(300.0, 1.5), 200);
        assert_eq!(physical_to_logical(300.0, f64::NAN), 300);
    }
}
