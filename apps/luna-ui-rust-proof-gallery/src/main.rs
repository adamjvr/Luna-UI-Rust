// SPDX-License-Identifier: MPL-2.0

//! Native M3 proof gallery for Luna UI Rust.
//!
//! This application is intentionally separate from the editor harness. It continuously exercises
//! responsive geometry, stateful controls, theme switching, multilingual shaping, immutable image
//! composition, timed invalidation, hit testing, and accessibility without turning animation into
//! editor overhead. Escape exits; click the button, toggle, or theme card to mutate proof state.

use luna_accessibility::{AccessibilityNode, AccessibilityRole};
use luna_core::{NodeId, PointI, RectI, SizeI};
use luna_host_winit::{
    AccessibilityActionKind, AccessibilityActionRequest, ApplicationError, HostControl,
    NativeApplication, WindowConfig, run_native,
};
use luna_input::{InputEvent, Key, NamedKey, PointerButton, PointerEventKind};
use luna_render::DisplayList;
use luna_text::TextDocument;
use luna_text_cosmic::{TextEngine, TextLayoutRequest, TextLayoutSnapshot};
use luna_theme::Theme;
use luna_ui::{
    Button, ControlState, ProgressBar, ProofGallery, ProofGalleryState, TextAlignment, TextLabel,
    Toggle, UiFrame, Widget,
};
use std::collections::BTreeMap;
use std::error::Error;
use std::time::Duration;

const ROOT_ID: &str = "m3-gallery-window";
const GALLERY_ID: &str = "m3-gallery";
const BUTTON_ID: &str = "m3-gallery-button";
const TOGGLE_ID: &str = "m3-gallery-toggle";
const PROGRESS_ID: &str = "m3-gallery-progress";
const MULTILINGUAL_SAMPLE: &str = "Latin: Luna UI Rust\nArabic: مرحباً بالعالم\nDevanagari: नमस्ते दुनिया\nJapanese: こんにちは世界\nEmoji: 🌙 🦋 👨‍👩‍👧‍👦";
const ACCESSIBILITY_NOTE: &str = "Every card, control, dialog, tab, tree row, and text surface exposes a stable NodeId and shared logical bounds.";

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    run_native(ProofGalleryApplication::new()?)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LabelCacheKey {
    width: u32,
    height: u32,
    activation_count: u32,
    light_theme: bool,
}

struct ProofGalleryApplication {
    root_id: NodeId,
    gallery_id: NodeId,
    button_id: NodeId,
    toggle_id: NodeId,
    progress_id: NodeId,
    theme_card_id: NodeId,
    state: ProofGalleryState,
    engine: TextEngine,
    labels: BTreeMap<String, TextLayoutSnapshot>,
    label_cache_key: Option<LabelCacheKey>,
    viewport: RectI,
    light_theme: bool,
    pointer_position: PointI,
}

impl ProofGalleryApplication {
    fn new() -> Result<Self, ApplicationError> {
        Ok(Self {
            root_id: NodeId::new(ROOT_ID)?,
            gallery_id: NodeId::new(GALLERY_ID)?,
            button_id: NodeId::new(BUTTON_ID)?,
            toggle_id: NodeId::new(TOGGLE_ID)?,
            progress_id: NodeId::new(PROGRESS_ID)?,
            theme_card_id: NodeId::new(GALLERY_ID)?.child("theme")?,
            state: ProofGalleryState::default(),
            engine: TextEngine::new(),
            labels: BTreeMap::new(),
            label_cache_key: None,
            viewport: RectI::new(0, 0, 1_180, 760),
            light_theme: false,
            pointer_position: PointI::new(-1, -1),
        })
    }

    const fn theme(&self) -> Theme {
        if self.light_theme {
            Theme::luna_light()
        } else {
            Theme::luna_dark()
        }
    }

    fn shape_label(
        &mut self,
        key: &str,
        text: &str,
        width: u32,
        font_size: f32,
        line_height: f32,
        theme: Theme,
    ) -> Result<(), ApplicationError> {
        let snapshot = self.engine.shape(
            &TextDocument::new(text),
            TextLayoutRequest::new(1, font_size, line_height, theme.foreground)
                .with_maximum_raster_width(width.max(1)),
        )?;
        self.labels.insert(key.to_owned(), snapshot);
        Ok(())
    }

    fn prepare_labels(&mut self, gallery: &ProofGallery) -> Result<(), ApplicationError> {
        let key = LabelCacheKey {
            width: self.viewport.width,
            height: self.viewport.height,
            activation_count: self.state.activation_count,
            light_theme: self.light_theme,
        };
        if self.label_cache_key == Some(key) {
            return Ok(());
        }
        self.labels.clear();
        let theme = self.theme();
        self.shape_label(
            "header",
            "Luna UI Rust Proof Gallery",
            gallery.layout().header.width,
            24.0,
            31.0,
            theme,
        )?;
        self.shape_label(
            "subtitle",
            "Deterministic regression surface — resize, activate controls, inspect accessibility",
            gallery.layout().subtitle.width,
            13.0,
            19.0,
            theme,
        )?;
        for card in &gallery.layout().cards {
            self.shape_label(
                &format!("card-{}", card.id),
                &card.title,
                card.bounds.width.saturating_sub(24),
                15.0,
                20.0,
                theme,
            )?;
        }
        self.shape_label(
            "button",
            &format!("Activate ({})", self.state.activation_count),
            gallery.layout().button.width,
            14.0,
            20.0,
            theme,
        )?;
        self.shape_label(
            "toggle",
            "Persistent toggle",
            gallery.layout().toggle.width.saturating_sub(52),
            14.0,
            20.0,
            theme,
        )?;
        self.shape_label(
            "text-sample",
            MULTILINGUAL_SAMPLE,
            gallery.layout().text_sample.width,
            14.0,
            21.0,
            theme,
        )?;
        self.shape_label(
            "accessibility",
            ACCESSIBILITY_NOTE,
            gallery.layout().accessibility_note.width,
            13.0,
            20.0,
            theme,
        )?;
        self.shape_label(
            "theme-note",
            if self.light_theme {
                "Light reference palette\nClick this card for dark mode"
            } else {
                "Dark reference palette\nClick this card for light mode"
            },
            gallery.layout().cards[5].content.width,
            14.0,
            22.0,
            theme,
        )?;
        self.label_cache_key = Some(key);
        Ok(())
    }

    fn label(
        &self,
        key: &str,
        id: &str,
        bounds: RectI,
        text: &str,
        alignment: TextAlignment,
    ) -> Option<TextLabel> {
        let layout = self.labels.get(key)?.clone();
        let node_id = NodeId::new(id).ok()?;
        Some(TextLabel::new(node_id, bounds, text, layout, alignment))
    }

    fn activate_button(&mut self) -> HostControl {
        self.state.activation_count = self.state.activation_count.saturating_add(1);
        self.label_cache_key = None;
        HostControl::Redraw
    }

    fn activate_toggle(&mut self) -> HostControl {
        self.state.toggle_is_on = !self.state.toggle_is_on;
        HostControl::Redraw
    }

    fn activate_theme(&mut self) -> HostControl {
        self.light_theme = !self.light_theme;
        self.label_cache_key = None;
        HostControl::Redraw
    }
}

impl NativeApplication for ProofGalleryApplication {
    fn window_config(&self) -> WindowConfig {
        WindowConfig {
            title: "Luna UI Rust — Proof Gallery".to_owned(),
            initial_size: SizeI::new(1_180, 760),
            minimum_size: Some(SizeI::new(620, 540)),
        }
    }

    fn build_frame(&mut self, viewport: RectI) -> Result<UiFrame, ApplicationError> {
        self.viewport = viewport;
        let theme = self.theme();
        let gallery = ProofGallery::new(self.gallery_id.clone(), viewport, theme, self.state)?;
        self.prepare_labels(&gallery)?;

        let button = Button::new(
            self.button_id.clone(),
            gallery.layout().button,
            format!("Activate ({})", self.state.activation_count),
            self.labels
                .get("button")
                .cloned()
                .ok_or_else(|| std::io::Error::other("button label cache missing"))?,
            theme,
            ControlState {
                is_hovered: gallery.layout().button.contains(self.pointer_position),
                ..ControlState::default()
            },
        );
        let toggle = Toggle::new(
            self.toggle_id.clone(),
            gallery.layout().toggle,
            "Persistent toggle",
            self.labels
                .get("toggle")
                .cloned()
                .ok_or_else(|| std::io::Error::other("toggle label cache missing"))?,
            theme,
            ControlState {
                is_hovered: gallery.layout().toggle.contains(self.pointer_position),
                ..ControlState::default()
            },
            self.state.toggle_is_on,
        );
        let progress_value = u16::try_from(self.state.activation_count % 11).unwrap_or(10);
        let progress = ProgressBar::new(
            self.progress_id.clone(),
            gallery.layout().progress,
            theme,
            progress_value,
            10,
            "Activation cycle",
        );

        let mut display_list = DisplayList::new();
        display_list.clear(theme.background);
        gallery.build_display_list(&mut display_list);
        button.build_display_list(&mut display_list);
        toggle.build_display_list(&mut display_list);
        progress.build_display_list(&mut display_list);

        let mut nodes = Vec::new();
        let mut root_children = vec![
            self.gallery_id.clone(),
            self.button_id.clone(),
            self.toggle_id.clone(),
            self.progress_id.clone(),
        ];
        nodes.push(
            AccessibilityNode::new(self.root_id.clone(), AccessibilityRole::Window, viewport)
                .with_label("Luna UI Rust proof gallery window")
                .with_children(root_children.clone()),
        );
        nodes.extend(gallery.accessibility_nodes());
        nodes.extend(button.accessibility_nodes());
        nodes.extend(toggle.accessibility_nodes());
        nodes.extend(progress.accessibility_nodes());

        let header_text = "Luna UI Rust Proof Gallery";
        if let Some(label) = self.label(
            "header",
            "m3-gallery-header-label",
            gallery.layout().header,
            header_text,
            TextAlignment::Leading,
        ) {
            label.build_display_list(&mut display_list);
            root_children.push(label.id().clone());
            nodes.extend(label.accessibility_nodes());
        }
        let subtitle_text =
            "Deterministic regression surface — resize, activate controls, inspect accessibility";
        if let Some(label) = self.label(
            "subtitle",
            "m3-gallery-subtitle-label",
            gallery.layout().subtitle,
            subtitle_text,
            TextAlignment::Leading,
        ) {
            label.build_display_list(&mut display_list);
            root_children.push(label.id().clone());
            nodes.extend(label.accessibility_nodes());
        }
        for card in &gallery.layout().cards {
            let key = format!("card-{}", card.id);
            let id = format!("m3-gallery-card-title-{}", card.id);
            let title_bounds = RectI::new(
                card.bounds.x.saturating_add(12),
                card.bounds.y.saturating_add(5),
                card.bounds.width.saturating_sub(24),
                28,
            );
            if let Some(label) =
                self.label(&key, &id, title_bounds, &card.title, TextAlignment::Leading)
            {
                label.build_display_list(&mut display_list);
            }
        }
        if let Some(label) = self.label(
            "text-sample",
            "m3-gallery-text-sample",
            gallery.layout().text_sample,
            MULTILINGUAL_SAMPLE,
            TextAlignment::Leading,
        ) {
            label.build_display_list(&mut display_list);
            root_children.push(label.id().clone());
            nodes.extend(label.accessibility_nodes());
        }
        if let Some(label) = self.label(
            "accessibility",
            "m3-gallery-accessibility-note",
            gallery.layout().accessibility_note,
            ACCESSIBILITY_NOTE,
            TextAlignment::Leading,
        ) {
            label.build_display_list(&mut display_list);
            root_children.push(label.id().clone());
            nodes.extend(label.accessibility_nodes());
        }
        if let Some(label) = self.label(
            "theme-note",
            "m3-gallery-theme-note",
            gallery.layout().cards[5].content,
            if self.light_theme {
                "Light reference palette\nClick this card for dark mode"
            } else {
                "Dark reference palette\nClick this card for light mode"
            },
            TextAlignment::Leading,
        ) {
            label.build_display_list(&mut display_list);
        }

        // Replace the root after all optional label IDs are known.
        nodes[0] =
            AccessibilityNode::new(self.root_id.clone(), AccessibilityRole::Window, viewport)
                .with_label("Luna UI Rust proof gallery window")
                .with_children(root_children);
        Ok(UiFrame::from_parts(
            display_list,
            self.root_id.clone(),
            nodes,
        )?)
    }

    fn handle_input(&mut self, event: InputEvent) -> HostControl {
        match event {
            InputEvent::Keyboard(keyboard)
                if keyboard.is_pressed && keyboard.key == Key::Named(NamedKey::Escape) =>
            {
                HostControl::Exit
            }
            InputEvent::Pointer(pointer) => {
                self.pointer_position = pointer.position;
                match pointer.kind {
                    PointerEventKind::Pressed(PointerButton::Primary) => {
                        let Ok(gallery) = ProofGallery::new(
                            self.gallery_id.clone(),
                            self.viewport,
                            self.theme(),
                            self.state,
                        ) else {
                            return HostControl::Continue;
                        };
                        if gallery.layout().button.contains(pointer.position) {
                            self.activate_button()
                        } else if gallery.layout().toggle.contains(pointer.position) {
                            self.activate_toggle()
                        } else if gallery.layout().cards[5].bounds.contains(pointer.position) {
                            self.activate_theme()
                        } else {
                            HostControl::Redraw
                        }
                    }
                    PointerEventKind::Moved
                    | PointerEventKind::Released(_)
                    | PointerEventKind::Pressed(_)
                    | PointerEventKind::Left => HostControl::Redraw,
                }
            }
            InputEvent::Keyboard(_)
            | InputEvent::Text(_)
            | InputEvent::Scroll(_)
            | InputEvent::FocusGained
            | InputEvent::FocusLost => HostControl::Continue,
        }
    }

    fn handle_accessibility_action(&mut self, request: AccessibilityActionRequest) -> HostControl {
        if request.kind != AccessibilityActionKind::Click {
            return HostControl::Continue;
        }
        match request.target.as_ref() {
            Some(target) if target == &self.button_id => self.activate_button(),
            Some(target) if target == &self.toggle_id => self.activate_toggle(),
            Some(target) if target == &self.theme_card_id => self.activate_theme(),
            _ => HostControl::Continue,
        }
    }

    fn frame_interval(&self) -> Option<Duration> {
        Some(Duration::from_millis(16))
    }

    fn update(&mut self, elapsed: Duration) -> HostControl {
        let bounded = elapsed.min(Duration::from_millis(50));
        self.state.animation_millis = self
            .state
            .animation_millis
            .saturating_add(u64::try_from(bounded.as_millis()).unwrap_or(50));
        HostControl::Redraw
    }
}
