// SPDX-License-Identifier: MPL-2.0

//! Native M1 proof application for Luna UI Rust.
//!
//! The application opens a real desktop window, renders the composite workspace fixture through
//! softbuffer, responds to pointer activation and Control-P, and exposes the same widget geometry
//! through AccessKit. Press Escape or close the window to exit.

use luna_commands::{CommandDefinition, CommandId, CommandRegistry, KeyBinding, KeyChord};
use luna_core::{NodeId, RectI, SizeI};
use luna_host_winit::{
    AccessibilityActionKind, AccessibilityActionRequest, ApplicationError, HostControl,
    NativeApplication, WindowConfig, run_native,
};
use luna_input::{InputEvent, Key, Modifiers, NamedKey, PointerButton, PointerEventKind};
use luna_theme::Theme;
use luna_ui::{UiFrame, Widget, WorkspaceDemo, WorkspaceDemoState};
use std::error::Error;

const ROOT_ID: &str = "native-demo";
const TOGGLE_COMMAND_ID: &str = "luna.demo.toggle-sidebar-accent";

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    run_native(NativeDemoApplication::new()?)?;
    Ok(())
}

struct NativeDemoApplication {
    theme: Theme,
    state: WorkspaceDemoState,
    commands: CommandRegistry,
    toggle_command: CommandId,
    command_button_id: NodeId,
    last_viewport: RectI,
}

impl NativeDemoApplication {
    fn new() -> Result<Self, ApplicationError> {
        let toggle_command = CommandId::new(TOGGLE_COMMAND_ID)?;
        let mut commands = CommandRegistry::new();
        commands.register_command(
            CommandDefinition::new(toggle_command.clone(), "Toggle sidebar accent")
                .with_description("Demonstrates typed command resolution from Control-P."),
        )?;
        commands.register_binding(KeyBinding::new(
            KeyChord::new(Key::Character("p".to_owned()), Modifiers::CONTROL),
            toggle_command.clone(),
        ))?;
        let root = NodeId::new(ROOT_ID)?;
        let command_button_id = root.child("sidebar")?.child("command")?;

        Ok(Self {
            theme: Theme::luna_dark(),
            state: WorkspaceDemoState::default(),
            commands,
            toggle_command,
            command_button_id,
            last_viewport: RectI::new(0, 0, 960, 540),
        })
    }

    fn toggle_sidebar(&mut self) {
        self.state.sidebar_is_accented = !self.state.sidebar_is_accented;
        self.state.focused_node = Some(self.command_button_id.clone());
    }

    fn widget(&self) -> Result<WorkspaceDemo, ApplicationError> {
        Ok(WorkspaceDemo::new(
            NodeId::new(ROOT_ID)?,
            self.last_viewport,
            self.theme,
            self.state.clone(),
        )?)
    }
}

impl NativeApplication for NativeDemoApplication {
    fn window_config(&self) -> WindowConfig {
        WindowConfig {
            title: "Luna UI Rust — M1 Native Host".to_owned(),
            initial_size: SizeI::new(960, 540),
            minimum_size: Some(SizeI::new(560, 320)),
        }
    }

    fn build_frame(&mut self, viewport: RectI) -> Result<UiFrame, ApplicationError> {
        self.last_viewport = viewport;
        let widget = self.widget()?;
        Ok(UiFrame::build(&widget, self.theme.background)?)
    }

    fn handle_input(&mut self, event: InputEvent) -> HostControl {
        match event {
            InputEvent::Keyboard(keyboard) => {
                if keyboard.is_pressed && keyboard.key == Key::Named(NamedKey::Escape) {
                    return HostControl::Exit;
                }
                if self
                    .commands
                    .resolve_keyboard(&keyboard)
                    .is_some_and(|request| request.command == self.toggle_command)
                {
                    self.toggle_sidebar();
                    return HostControl::Redraw;
                }
            }
            InputEvent::Pointer(pointer)
                if pointer.kind == PointerEventKind::Pressed(PointerButton::Primary) =>
            {
                let target = self
                    .widget()
                    .ok()
                    .and_then(|widget| widget.hit_test(pointer.position));
                if target.as_ref() == Some(&self.command_button_id) {
                    self.toggle_sidebar();
                    return HostControl::Redraw;
                }
            }
            InputEvent::Pointer(_)
            | InputEvent::Text(_)
            | InputEvent::Ime(_)
            | InputEvent::Scroll(_)
            | InputEvent::FocusGained
            | InputEvent::FocusLost => {}
        }
        HostControl::Continue
    }

    fn handle_accessibility_action(&mut self, request: AccessibilityActionRequest) -> HostControl {
        if request.target.as_ref() != Some(&self.command_button_id) {
            return HostControl::Continue;
        }
        match request.kind {
            AccessibilityActionKind::Click => {
                self.toggle_sidebar();
                HostControl::Redraw
            }
            AccessibilityActionKind::Focus => {
                self.state.focused_node = Some(self.command_button_id.clone());
                HostControl::Redraw
            }
            AccessibilityActionKind::ReplaceSelectedText
            | AccessibilityActionKind::SetValue
            | AccessibilityActionKind::ShowContextMenu
            | AccessibilityActionKind::Increment
            | AccessibilityActionKind::Decrement
            | AccessibilityActionKind::Other => HostControl::Continue,
        }
    }
}
