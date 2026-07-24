// SPDX-License-Identifier: MPL-2.0

//! Native M3 proof gallery for Luna UI Rust.
//!
//! This application is intentionally separate from the editor harness. It continuously exercises
//! responsive geometry, stateful controls, theme switching, multilingual shaping, immutable image
//! composition, timed invalidation, hit testing, and accessibility without turning animation into
//! editor overhead. M3.1c retains static geometry, paint, labels, and semantics while rebuilding only
//! the animation lane on timed samples.

use luna_accessibility::{AccessibilityNode, AccessibilityRole, AccessibilityTree};
use luna_core::{NodeId, PointI, RectI, SizeI};
use luna_host_core::InvalidationClass;
use luna_host_winit::{
    AccessibilityActionKind, AccessibilityActionRequest, ApplicationError, HostControl,
    NativeApplication, WindowConfig, run_native,
};
use luna_input::{InputEvent, Key, NamedKey, PointerButton, PointerEventKind};
use luna_render::DisplayList;
use luna_text_cosmic::{TextEngine, TextLayoutSnapshot};
use luna_theme::Theme;
use luna_ui::{
    Button, ControlState, ProgressBar, ProofGallery, ProofGalleryLayout, ProofGalleryState,
    RetainedDisplayList, TextAlignment, TextLabel, TextLabelCache, Toggle, UiFrame, Widget,
};
use std::collections::BTreeMap;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum GalleryHitTarget {
    #[default]
    None,
    Button,
    Toggle,
    ThemeCard,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StaticGalleryKey {
    viewport: RectI,
    activation_count: u32,
    toggle_is_on: bool,
    light_theme: bool,
    hover_target: GalleryHitTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SemanticGalleryKey {
    viewport: RectI,
    activation_count: u32,
    toggle_is_on: bool,
    light_theme: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LayoutCacheEntry {
    viewport: RectI,
    layout: ProofGalleryLayout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StaticGalleryCacheEntry {
    key: StaticGalleryKey,
    revision: u64,
    display_list: Arc<DisplayList>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SemanticGalleryCacheEntry {
    key: SemanticGalleryKey,
    tree: Arc<AccessibilityTree>,
}

#[derive(Debug)]
struct GalleryCacheMetrics {
    report_started_at: Instant,
    window_frames: u64,
    total_frames: u64,
    layout_hits: u64,
    layout_misses: u64,
    static_hits: u64,
    static_misses: u64,
    semantic_hits: u64,
    semantic_misses: u64,
    coalesced_pointer_moves: u64,
}

impl GalleryCacheMetrics {
    fn new() -> Self {
        Self {
            report_started_at: Instant::now(),
            window_frames: 0,
            total_frames: 0,
            layout_hits: 0,
            layout_misses: 0,
            static_hits: 0,
            static_misses: 0,
            semantic_hits: 0,
            semantic_misses: 0,
            coalesced_pointer_moves: 0,
        }
    }

    fn record_frame(
        &mut self,
        layout_hit: bool,
        static_hit: bool,
        semantic_hit: bool,
        label_stats: luna_ui::TextLabelCacheStats,
    ) {
        self.window_frames = self.window_frames.saturating_add(1);
        self.total_frames = self.total_frames.saturating_add(1);
        if layout_hit {
            self.layout_hits = self.layout_hits.saturating_add(1);
        } else {
            self.layout_misses = self.layout_misses.saturating_add(1);
        }
        if static_hit {
            self.static_hits = self.static_hits.saturating_add(1);
        } else {
            self.static_misses = self.static_misses.saturating_add(1);
        }
        if semantic_hit {
            self.semantic_hits = self.semantic_hits.saturating_add(1);
        } else {
            self.semantic_misses = self.semantic_misses.saturating_add(1);
        }

        if self.total_frames == 1 || self.report_started_at.elapsed() >= Duration::from_secs(1) {
            eprintln!(
                "[luna-gallery cache] frames={} total_frames={} layout={{hits:{}, misses:{}}} static={{hits:{}, misses:{}}} semantics={{hits:{}, misses:{}}} labels={{hits:{}, misses:{}, entries:{}}} coalesced_pointer_moves={}",
                self.window_frames,
                self.total_frames,
                self.layout_hits,
                self.layout_misses,
                self.static_hits,
                self.static_misses,
                self.semantic_hits,
                self.semantic_misses,
                label_stats.hits,
                label_stats.misses,
                label_stats.entries,
                self.coalesced_pointer_moves,
            );
            self.report_started_at = Instant::now();
            self.window_frames = 0;
            self.layout_hits = 0;
            self.layout_misses = 0;
            self.static_hits = 0;
            self.static_misses = 0;
            self.semantic_hits = 0;
            self.semantic_misses = 0;
            self.coalesced_pointer_moves = 0;
        }
    }

    fn record_coalesced_pointer_move(&mut self) {
        self.coalesced_pointer_moves = self.coalesced_pointer_moves.saturating_add(1);
    }
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
    label_cache: TextLabelCache,
    labels: BTreeMap<String, TextLayoutSnapshot>,
    label_cache_key: Option<LabelCacheKey>,
    layout_cache: Option<LayoutCacheEntry>,
    static_cache: Option<StaticGalleryCacheEntry>,
    semantic_cache: Option<SemanticGalleryCacheEntry>,
    next_static_revision: u64,
    viewport: RectI,
    light_theme: bool,
    hover_target: GalleryHitTarget,
    metrics: GalleryCacheMetrics,
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
            label_cache: TextLabelCache::new(),
            labels: BTreeMap::new(),
            label_cache_key: None,
            layout_cache: None,
            static_cache: None,
            semantic_cache: None,
            next_static_revision: 1,
            viewport: RectI::new(0, 0, 1_180, 760),
            light_theme: false,
            hover_target: GalleryHitTarget::None,
            metrics: GalleryCacheMetrics::new(),
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
        slot_id: &str,
        text: &str,
        width: u32,
        font_size: f32,
        line_height: f32,
        theme: Theme,
    ) -> Result<(), ApplicationError> {
        let snapshot = self.label_cache.layout(
            &mut self.engine,
            slot_id,
            text,
            width,
            font_size,
            line_height,
            theme.foreground,
        )?;
        self.labels.insert(slot_id.to_owned(), snapshot);
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

    fn ensure_layout(
        &mut self,
        viewport: RectI,
    ) -> Result<(ProofGalleryLayout, bool), ApplicationError> {
        if let Some(cache) = &self.layout_cache
            && cache.viewport == viewport
        {
            return Ok((cache.layout.clone(), true));
        }
        let gallery = ProofGallery::new(
            self.gallery_id.clone(),
            viewport,
            self.theme(),
            ProofGalleryState::default(),
        )?;
        let layout = gallery.layout().clone();
        self.layout_cache = Some(LayoutCacheEntry {
            viewport,
            layout: layout.clone(),
        });
        Ok((layout, false))
    }

    fn static_key(&self) -> StaticGalleryKey {
        StaticGalleryKey {
            viewport: self.viewport,
            activation_count: self.state.activation_count,
            toggle_is_on: self.state.toggle_is_on,
            light_theme: self.light_theme,
            hover_target: self.hover_target,
        }
    }

    fn semantic_key(&self) -> SemanticGalleryKey {
        SemanticGalleryKey {
            viewport: self.viewport,
            activation_count: self.state.activation_count,
            toggle_is_on: self.state.toggle_is_on,
            light_theme: self.light_theme,
        }
    }

    fn make_controls(
        &self,
        gallery: &ProofGallery,
        include_hover: bool,
    ) -> Result<(Button, Toggle, ProgressBar), ApplicationError> {
        let theme = self.theme();
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
                is_hovered: include_hover && self.hover_target == GalleryHitTarget::Button,
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
                is_hovered: include_hover && self.hover_target == GalleryHitTarget::Toggle,
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
        Ok((button, toggle, progress))
    }

    fn append_static_labels(&self, display_list: &mut DisplayList, gallery: &ProofGallery) {
        if let Some(label) = self.label(
            "header",
            "m3-gallery-header-label",
            gallery.layout().header,
            "Luna UI Rust Proof Gallery",
            TextAlignment::Leading,
        ) {
            label.build_display_list(display_list);
        }
        if let Some(label) = self.label(
            "subtitle",
            "m3-gallery-subtitle-label",
            gallery.layout().subtitle,
            "Deterministic regression surface — resize, activate controls, inspect accessibility",
            TextAlignment::Leading,
        ) {
            label.build_display_list(display_list);
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
                label.build_display_list(display_list);
            }
        }
        if let Some(label) = self.label(
            "text-sample",
            "m3-gallery-text-sample",
            gallery.layout().text_sample,
            MULTILINGUAL_SAMPLE,
            TextAlignment::Leading,
        ) {
            label.build_display_list(display_list);
        }
        if let Some(label) = self.label(
            "accessibility",
            "m3-gallery-accessibility-note",
            gallery.layout().accessibility_note,
            ACCESSIBILITY_NOTE,
            TextAlignment::Leading,
        ) {
            label.build_display_list(display_list);
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
            label.build_display_list(display_list);
        }
    }

    fn ensure_static_scene(
        &mut self,
        layout: &ProofGalleryLayout,
    ) -> Result<(Arc<DisplayList>, u64, bool), ApplicationError> {
        let key = self.static_key();
        if let Some(cache) = &self.static_cache
            && cache.key == key
        {
            return Ok((Arc::clone(&cache.display_list), cache.revision, true));
        }

        let gallery = ProofGallery::from_layout_snapshot(
            self.gallery_id.clone(),
            self.theme(),
            self.state,
            layout.clone(),
        );
        self.prepare_labels(&gallery)?;
        let (button, toggle, progress) = self.make_controls(&gallery, true)?;
        let mut display_list = DisplayList::new();
        display_list.clear(self.theme().background);
        gallery.build_static_display_list(&mut display_list);
        button.build_display_list(&mut display_list);
        toggle.build_display_list(&mut display_list);
        progress.build_display_list(&mut display_list);
        self.append_static_labels(&mut display_list, &gallery);
        if self.hover_target == GalleryHitTarget::ThemeCard {
            draw_hover_border(
                &mut display_list,
                gallery.layout().cards[5].bounds,
                self.theme().accent,
            );
        }

        let revision = self.next_static_revision;
        self.next_static_revision = self.next_static_revision.saturating_add(1);
        let display_list = Arc::new(display_list);
        self.static_cache = Some(StaticGalleryCacheEntry {
            key,
            revision,
            display_list: Arc::clone(&display_list),
        });
        Ok((display_list, revision, false))
    }

    fn build_accessibility_tree(
        &self,
        gallery: &ProofGallery,
    ) -> Result<AccessibilityTree, ApplicationError> {
        let (button, toggle, progress) = self.make_controls(gallery, false)?;
        let mut nodes = Vec::new();
        let mut root_children = vec![
            self.gallery_id.clone(),
            self.button_id.clone(),
            self.toggle_id.clone(),
            self.progress_id.clone(),
        ];
        nodes.push(
            AccessibilityNode::new(
                self.root_id.clone(),
                AccessibilityRole::Window,
                self.viewport,
            )
            .with_label("Luna UI Rust proof gallery window")
            .with_children(root_children.clone()),
        );
        nodes.extend(gallery.accessibility_nodes());
        nodes.extend(button.accessibility_nodes());
        nodes.extend(toggle.accessibility_nodes());
        nodes.extend(progress.accessibility_nodes());

        if let Some(label) = self.label(
            "header",
            "m3-gallery-header-label",
            gallery.layout().header,
            "Luna UI Rust Proof Gallery",
            TextAlignment::Leading,
        ) {
            root_children.push(label.id().clone());
            nodes.extend(label.accessibility_nodes());
        }
        if let Some(label) = self.label(
            "subtitle",
            "m3-gallery-subtitle-label",
            gallery.layout().subtitle,
            "Deterministic regression surface — resize, activate controls, inspect accessibility",
            TextAlignment::Leading,
        ) {
            root_children.push(label.id().clone());
            nodes.extend(label.accessibility_nodes());
        }
        if let Some(label) = self.label(
            "text-sample",
            "m3-gallery-text-sample",
            gallery.layout().text_sample,
            MULTILINGUAL_SAMPLE,
            TextAlignment::Leading,
        ) {
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
            root_children.push(label.id().clone());
            nodes.extend(label.accessibility_nodes());
        }

        nodes[0] = AccessibilityNode::new(
            self.root_id.clone(),
            AccessibilityRole::Window,
            self.viewport,
        )
        .with_label("Luna UI Rust proof gallery window")
        .with_children(root_children);
        Ok(AccessibilityTree::new(self.root_id.clone(), nodes)?)
    }

    fn ensure_semantics(
        &mut self,
        layout: &ProofGalleryLayout,
    ) -> Result<(Arc<AccessibilityTree>, bool), ApplicationError> {
        let key = self.semantic_key();
        if let Some(cache) = &self.semantic_cache
            && cache.key == key
        {
            return Ok((Arc::clone(&cache.tree), true));
        }
        let gallery = ProofGallery::from_layout_snapshot(
            self.gallery_id.clone(),
            self.theme(),
            self.state,
            layout.clone(),
        );
        let tree = Arc::new(self.build_accessibility_tree(&gallery)?);
        self.semantic_cache = Some(SemanticGalleryCacheEntry {
            key,
            tree: Arc::clone(&tree),
        });
        Ok((tree, false))
    }

    fn hit_target(&self, point: PointI) -> GalleryHitTarget {
        let Some(cache) = &self.layout_cache else {
            return GalleryHitTarget::None;
        };
        if cache.layout.button.contains(point) {
            GalleryHitTarget::Button
        } else if cache.layout.toggle.contains(point) {
            GalleryHitTarget::Toggle
        } else if cache
            .layout
            .cards
            .get(5)
            .is_some_and(|card| card.bounds.contains(point))
        {
            GalleryHitTarget::ThemeCard
        } else {
            GalleryHitTarget::None
        }
    }

    fn update_hover(&mut self, target: GalleryHitTarget) -> HostControl {
        if self.hover_target == target {
            self.metrics.record_coalesced_pointer_move();
            return HostControl::Continue;
        }
        self.hover_target = target;
        HostControl::Invalidate(InvalidationClass::PaintOverlay)
    }

    fn activate_button(&mut self) -> HostControl {
        self.state.activation_count = self.state.activation_count.saturating_add(1);
        self.label_cache_key = None;
        HostControl::Invalidate(InvalidationClass::TextLayout)
    }

    fn activate_toggle(&mut self) -> HostControl {
        self.state.toggle_is_on = !self.state.toggle_is_on;
        HostControl::Invalidate(InvalidationClass::PaintOverlay)
    }

    fn activate_theme(&mut self) -> HostControl {
        self.light_theme = !self.light_theme;
        self.label_cache_key = None;
        HostControl::Invalidate(InvalidationClass::FullFrame)
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
        let (layout, layout_hit) = self.ensure_layout(viewport)?;
        let (static_display_list, static_revision, static_hit) =
            self.ensure_static_scene(&layout)?;
        let (accessibility_tree, semantic_hit) = self.ensure_semantics(&layout)?;
        let gallery = ProofGallery::from_layout_snapshot(
            self.gallery_id.clone(),
            self.theme(),
            self.state,
            layout,
        );
        let mut dynamic_display_list = DisplayList::new();
        gallery.build_animation_display_list(&mut dynamic_display_list);
        self.metrics.record_frame(
            layout_hit,
            static_hit,
            semantic_hit,
            self.label_cache.stats(),
        );

        Ok(UiFrame::from_retained_snapshots(
            RetainedDisplayList::new(
                static_revision,
                static_display_list,
                gallery.layout().animation_lane,
            ),
            dynamic_display_list,
            accessibility_tree,
        ))
    }

    fn handle_input(&mut self, event: InputEvent) -> HostControl {
        match event {
            InputEvent::Keyboard(keyboard)
                if keyboard.is_pressed && keyboard.key == Key::Named(NamedKey::Escape) =>
            {
                HostControl::Exit
            }
            InputEvent::Pointer(pointer) => match pointer.kind {
                PointerEventKind::Moved => self.update_hover(self.hit_target(pointer.position)),
                PointerEventKind::Left => self.update_hover(GalleryHitTarget::None),
                PointerEventKind::Pressed(PointerButton::Primary) => {
                    let target = self.hit_target(pointer.position);
                    let hover_changed = self.hover_target != target;
                    self.hover_target = target;
                    match target {
                        GalleryHitTarget::Button => self.activate_button(),
                        GalleryHitTarget::Toggle => self.activate_toggle(),
                        GalleryHitTarget::ThemeCard => self.activate_theme(),
                        GalleryHitTarget::None if hover_changed => {
                            HostControl::Invalidate(InvalidationClass::PaintOverlay)
                        }
                        GalleryHitTarget::None => HostControl::Continue,
                    }
                }
                PointerEventKind::Released(_) | PointerEventKind::Pressed(_) => {
                    HostControl::Continue
                }
            },
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
        HostControl::Invalidate(InvalidationClass::Animation)
    }
}

fn draw_hover_border(display_list: &mut DisplayList, bounds: RectI, color: luna_theme::Rgba8) {
    if bounds.is_empty() {
        return;
    }
    let thickness = 2_u32.min(bounds.width).min(bounds.height);
    display_list.fill_rect(
        RectI::new(bounds.x, bounds.y, bounds.width, thickness),
        color,
    );
    display_list.fill_rect(
        RectI::new(
            bounds.x,
            i32::try_from(bounds.bottom().saturating_sub(i64::from(thickness))).unwrap_or(i32::MAX),
            bounds.width,
            thickness,
        ),
        color,
    );
    display_list.fill_rect(
        RectI::new(bounds.x, bounds.y, thickness, bounds.height),
        color,
    );
    display_list.fill_rect(
        RectI::new(
            i32::try_from(bounds.right().saturating_sub(i64::from(thickness))).unwrap_or(i32::MAX),
            bounds.y,
            thickness,
            bounds.height,
        ),
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::{GalleryHitTarget, ProofGalleryApplication};
    use luna_core::{PointI, RectI};
    use luna_host_winit::NativeApplication;
    use std::error::Error;
    use std::sync::Arc;

    #[test]
    fn pointer_motion_inside_one_target_is_coalesced() -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut application = ProofGalleryApplication::new()?;
        let _ = application.build_frame(RectI::new(0, 0, 1_180, 760))?;
        let button = application
            .layout_cache
            .as_ref()
            .map(|cache| cache.layout.button)
            .ok_or_else(|| std::io::Error::other("layout cache missing"))?;
        let point = PointI::new(button.x.saturating_add(1), button.y.saturating_add(1));

        assert_eq!(application.hit_target(point), GalleryHitTarget::Button);
        let first = application.update_hover(GalleryHitTarget::Button);
        let second = application.update_hover(GalleryHitTarget::Button);
        assert_ne!(first, second);
        assert_eq!(second, luna_host_winit::HostControl::Continue);
        Ok(())
    }

    #[test]
    fn animation_reuses_layout_static_paint_and_semantics()
    -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut application = ProofGalleryApplication::new()?;
        let first = application.build_frame(RectI::new(0, 0, 1_180, 760))?;
        application.state.animation_millis = 200;
        let second = application.build_frame(RectI::new(0, 0, 1_180, 760))?;

        assert_eq!(
            first
                .retained_display_list
                .as_ref()
                .map(|retained| retained.revision),
            second
                .retained_display_list
                .as_ref()
                .map(|retained| retained.revision)
        );
        assert!(Arc::ptr_eq(
            &first.accessibility_tree,
            &second.accessibility_tree
        ));
        assert_ne!(first.display_list, second.display_list);
        Ok(())
    }
}
