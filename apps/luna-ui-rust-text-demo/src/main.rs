// SPDX-License-Identifier: MPL-2.0

//! Native M2 proof application for Luna UI Rust editor-grade text.
//!
//! The demo exercises cosmic-text advanced shaping and fallback, grapheme-safe mutation,
//! line/UTF-8 coordinates, mouse hit testing, drag selection, caret motion, scrolling, clipping,
//! and text accessibility. Type normally; use arrows, Shift-arrows, Home/End, Page Up/Down,
//! Backspace/Delete, Control-A, the mouse wheel, or pointer drag. Escape exits.

use luna_core::{InsetsI, NodeId, PointI, RectI, SizeI};
use luna_host_winit::{
    AccessibilityActionKind, AccessibilityActionRequest, ApplicationError, HostControl,
    NativeApplication, WindowConfig, run_native,
};
use luna_input::{InputEvent, Key, Modifiers, NamedKey, PointerButton, PointerEventKind};
use luna_text::{EditableText, TextLocation, TextRange, TextScroll};
use luna_text_cosmic::{TextEngine, TextLayoutRequest, TextLayoutSnapshot};
use luna_theme::Theme;
use luna_ui::{TextView, TextViewStyle, UiFrame};
use std::error::Error;

const EDITOR_ID: &str = "m2-text-demo.editor";
const DEMO_TEXT: &str = "Luna UI Rust — M2 editor-grade text\n\n\
This surface is shaped with cosmic-text using advanced shaping.\n\
Ligatures: ffi fi fl -> != == <= >=\n\
Combining grapheme: e\u{301} remains one deletion unit.\n\
Emoji family: 👨‍👩‍👧‍👦 remains one deletion unit.\n\
Arabic / bidi: مرحباً بالعالم — Luna\n\
Devanagari fallback: नमस्ते दुनिया\n\
Japanese fallback: こんにちは世界\n\n\
Click, drag, type, scroll, and resize the window.\n\
The caret, selection, pixels, hit tests, and accessibility all share one snapshot.\n";

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    run_native(TextDemoApplication::new()?)?;
    Ok(())
}

struct TextDemoApplication {
    editor_id: NodeId,
    editor: EditableText,
    engine: TextEngine,
    layout: Option<TextLayoutSnapshot>,
    scroll: TextScroll,
    theme: Theme,
    viewport: RectI,
    is_focused: bool,
    drag_anchor: Option<TextLocation>,
    reveal_caret_on_next_frame: bool,
}

impl TextDemoApplication {
    fn new() -> Result<Self, ApplicationError> {
        Ok(Self {
            editor_id: NodeId::new(EDITOR_ID)?,
            editor: EditableText::new(DEMO_TEXT),
            engine: TextEngine::new(),
            layout: None,
            scroll: TextScroll::default(),
            theme: Theme::luna_dark(),
            viewport: RectI::new(0, 0, 1_000, 680),
            is_focused: true,
            drag_anchor: None,
            reveal_caret_on_next_frame: true,
        })
    }

    fn editor_bounds(&self) -> RectI {
        self.viewport.inset(InsetsI::symmetric(16, 16))
    }

    fn text_width(&self) -> u32 {
        let style = TextViewStyle::from_theme(self.theme);
        self.editor_bounds()
            .inset(style.content_insets)
            .width
            .saturating_sub(style.gutter_width)
            .max(1)
    }

    fn view(&self, layout: TextLayoutSnapshot) -> TextView {
        TextView::new(
            self.editor_id.clone(),
            self.editor_bounds(),
            self.editor.document().clone(),
            layout,
            self.editor.caret(),
            self.editor.selection(),
            self.scroll,
            TextViewStyle::from_theme(self.theme),
            "Luna M2 editable text demo",
            self.is_focused,
            true,
        )
    }

    fn current_view(&self) -> Option<TextView> {
        self.layout.clone().map(|layout| self.view(layout))
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
        self.is_focused = true;
        self.reveal_caret_on_next_frame = true;
        true
    }

    fn handle_navigation_key(&mut self, key: NamedKey, modifiers: Modifiers) -> HostControl {
        let extending = modifiers.contains(Modifiers::SHIFT);
        let mut reveal_caret = true;
        match key {
            NamedKey::ArrowLeft => self.editor.move_backward(extending),
            NamedKey::ArrowRight => self.editor.move_forward(extending),
            NamedKey::ArrowUp => self.editor.move_up(extending),
            NamedKey::ArrowDown => self.editor.move_down(extending),
            NamedKey::Home => self.editor.move_to_line_start(extending),
            NamedKey::End => self.editor.move_to_line_end(extending),
            NamedKey::Backspace => {
                let _ = self.editor.delete_backward();
            }
            NamedKey::Delete => {
                let _ = self.editor.delete_forward();
            }
            NamedKey::Enter => {
                let _ = self.editor.insert_newline();
            }
            NamedKey::PageUp => {
                let amount = i32::try_from(self.editor_bounds().height).unwrap_or(i32::MAX);
                self.scroll.y = self.scroll.y.saturating_sub(amount).max(0);
                reveal_caret = false;
            }
            NamedKey::PageDown => {
                let amount = i32::try_from(self.editor_bounds().height).unwrap_or(i32::MAX);
                self.scroll.y = self.scroll.y.saturating_add(amount);
                reveal_caret = false;
            }
            NamedKey::Escape | NamedKey::Tab => return HostControl::Continue,
        }
        self.reveal_caret_on_next_frame = reveal_caret;
        HostControl::Redraw
    }
}

impl NativeApplication for TextDemoApplication {
    fn window_config(&self) -> WindowConfig {
        WindowConfig {
            title: "Luna UI Rust — M2 Editor Text".to_owned(),
            initial_size: SizeI::new(1_000, 680),
            minimum_size: Some(SizeI::new(560, 360)),
        }
    }

    fn build_frame(&mut self, viewport: RectI) -> Result<UiFrame, ApplicationError> {
        self.viewport = viewport;
        let layout = self.engine.shape(
            self.editor.document(),
            TextLayoutRequest::new(self.text_width(), 16.0, 23.0, self.theme.foreground),
        )?;
        if self.reveal_caret_on_next_frame {
            let provisional = self.view(layout.clone());
            self.scroll = provisional.scroll_revealing_caret();
            self.reveal_caret_on_next_frame = false;
        }
        let view = self.view(layout.clone());
        self.layout = Some(layout);
        Ok(UiFrame::build(&view, self.theme.background)?)
    }

    fn handle_input(&mut self, event: InputEvent) -> HostControl {
        match event {
            InputEvent::Keyboard(keyboard) if keyboard.is_pressed => {
                if keyboard.key == Key::Named(NamedKey::Escape) {
                    return HostControl::Exit;
                }
                let command_modified = keyboard.modifiers.contains(Modifiers::SUPER)
                    || (keyboard.modifiers.contains(Modifiers::CONTROL)
                        && !keyboard.modifiers.contains(Modifiers::ALT));
                if command_modified
                    && matches!(&keyboard.key, Key::Character(value) if value.eq_ignore_ascii_case("a"))
                {
                    self.editor.set_selection(TextRange::new(
                        TextLocation::default(),
                        self.editor.document().end_location(),
                    ));
                    self.reveal_caret_on_next_frame = true;
                    return HostControl::Redraw;
                }
                if let Key::Named(key) = &keyboard.key {
                    return self.handle_navigation_key(*key, keyboard.modifiers);
                }
                let logical_fallback = match &keyboard.key {
                    Key::Character(value) => Some(value.as_str()),
                    Key::Named(_) | Key::Unidentified => None,
                };
                if let Some(text) = keyboard.text.as_deref().or(logical_fallback) {
                    if !command_modified && !text.is_empty() && !text.chars().all(char::is_control)
                    {
                        let _ = self.editor.insert_text(text);
                        self.reveal_caret_on_next_frame = true;
                        return HostControl::Redraw;
                    }
                }
            }
            InputEvent::Text(text) => {
                let _ = self.editor.insert_text(&text);
                self.reveal_caret_on_next_frame = true;
                return HostControl::Redraw;
            }
            InputEvent::Pointer(pointer) => match pointer.kind {
                PointerEventKind::Pressed(PointerButton::Primary) => {
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
                PointerEventKind::Released(PointerButton::Primary) => {
                    self.drag_anchor = None;
                }
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
                    return HostControl::Redraw;
                }
            }
            InputEvent::FocusGained => {
                self.is_focused = true;
                return HostControl::Redraw;
            }
            InputEvent::FocusLost => {
                self.is_focused = false;
                self.drag_anchor = None;
                return HostControl::Redraw;
            }
            InputEvent::Keyboard(_) => {}
        }
        HostControl::Continue
    }

    fn handle_accessibility_action(&mut self, request: AccessibilityActionRequest) -> HostControl {
        if request.target.as_ref() != Some(&self.editor_id) {
            return HostControl::Continue;
        }
        match request.kind {
            AccessibilityActionKind::Focus => {
                self.is_focused = true;
                HostControl::Redraw
            }
            AccessibilityActionKind::Click | AccessibilityActionKind::Other => {
                HostControl::Continue
            }
        }
    }
}
