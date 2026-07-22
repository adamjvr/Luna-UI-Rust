// SPDX-License-Identifier: MPL-2.0

//! Platform-neutral input values and events.
//!
//! Native hosts translate operating-system events into these types. Widgets and application code
//! therefore never need to depend directly on winit, SDL, Cocoa, Wayland, X11, or Win32 event
//! structures.

use luna_core::PointI;

/// Modifier-key state represented as a compact bit set without a third-party dependency.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Modifiers(u8);

impl Modifiers {
    /// No modifiers are active.
    pub const NONE: Self = Self(0);
    /// Shift is active.
    pub const SHIFT: Self = Self(1 << 0);
    /// Control is active.
    pub const CONTROL: Self = Self(1 << 1);
    /// Alt/Option is active.
    pub const ALT: Self = Self(1 << 2);
    /// Super/Command/Windows is active.
    pub const SUPER: Self = Self(1 << 3);

    /// Returns a set containing flags from both operands.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns whether every flag in `other` is active.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// Mouse, pen, or touch button normalized by the host.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PointerButton {
    /// Primary activation button.
    Primary,
    /// Secondary/context button.
    Secondary,
    /// Middle button.
    Middle,
    /// Additional device-specific button.
    Other(u16),
}

/// A named non-text key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NamedKey {
    /// Escape.
    Escape,
    /// Enter/Return.
    Enter,
    /// Tab.
    Tab,
    /// Backspace.
    Backspace,
    /// Delete/Forward Delete.
    Delete,
    /// Left arrow.
    ArrowLeft,
    /// Right arrow.
    ArrowRight,
    /// Up arrow.
    ArrowUp,
    /// Down arrow.
    ArrowDown,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
}

/// Logical key value after host keyboard-layout processing.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Key {
    /// A named control/navigation key.
    Named(NamedKey),
    /// A printable logical key value.
    Character(String),
    /// A key the host could not map yet.
    Unidentified,
}

/// Pointer event kind.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PointerEventKind {
    /// Pointer moved.
    Moved,
    /// Pointer button was pressed.
    Pressed(PointerButton),
    /// Pointer button was released.
    Released(PointerButton),
    /// Pointer left the window or surface.
    Left,
}

/// A normalized pointer event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointerEvent {
    /// Event kind.
    pub kind: PointerEventKind,
    /// Position in Luna logical coordinates.
    pub position: PointI,
    /// Modifier state sampled with the event.
    pub modifiers: Modifiers,
    /// Monotonic host timestamp in microseconds.
    pub timestamp_micros: u64,
}

/// A normalized keyboard event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyboardEvent {
    /// Logical key.
    pub key: Key,
    /// Whether this event is a press (`true`) or release (`false`).
    pub is_pressed: bool,
    /// Whether the operating system marked this as key repeat.
    pub is_repeat: bool,
    /// Modifier state sampled with the event.
    pub modifiers: Modifiers,
    /// Monotonic host timestamp in microseconds.
    pub timestamp_micros: u64,
}

/// A normalized scroll event in logical pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScrollEvent {
    /// Horizontal delta.
    pub delta_x: i32,
    /// Vertical delta.
    pub delta_y: i32,
    /// Modifier state sampled with the event.
    pub modifiers: Modifiers,
    /// Monotonic host timestamp in microseconds.
    pub timestamp_micros: u64,
}

/// Input delivered to the deterministic Luna UI lane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputEvent {
    /// Pointer activity.
    Pointer(PointerEvent),
    /// Keyboard activity.
    Keyboard(KeyboardEvent),
    /// Text committed by the platform text-input system.
    Text(String),
    /// Scrolling.
    Scroll(ScrollEvent),
    /// The window or surface gained focus.
    FocusGained,
    /// The window or surface lost focus.
    FocusLost,
}

#[cfg(test)]
mod tests {
    use super::Modifiers;

    #[test]
    fn modifier_sets_compose_without_external_bitflags() {
        let modifiers = Modifiers::CONTROL.union(Modifiers::SHIFT);

        assert!(modifiers.contains(Modifiers::CONTROL));
        assert!(modifiers.contains(Modifiers::SHIFT));
        assert!(!modifiers.contains(Modifiers::ALT));
    }
}
