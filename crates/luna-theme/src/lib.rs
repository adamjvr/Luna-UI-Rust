// SPDX-License-Identifier: MPL-2.0

//! Theme tokens shared by widgets and renderers.
//!
//! Luna keeps theme values strongly typed and product-neutral. File-format adapters such as
//! Sublime `.sublime-color-scheme` importers belong in later leaf crates; widgets consume this
//! compact semantic palette and may derive translucent or blended variants without embedding a
//! particular application's appearance policy.

/// An eight-bit-per-channel RGBA color.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Rgba8 {
    /// Red channel.
    pub red: u8,
    /// Green channel.
    pub green: u8,
    /// Blue channel.
    pub blue: u8,
    /// Alpha channel.
    pub alpha: u8,
}

impl Rgba8 {
    /// Creates an opaque color.
    #[must_use]
    pub const fn opaque(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: u8::MAX,
        }
    }

    /// Creates a color with an explicit alpha channel.
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    /// Returns the same RGB color with a replacement alpha channel.
    #[must_use]
    pub const fn with_alpha(self, alpha: u8) -> Self {
        Self { alpha, ..self }
    }

    /// Linearly interpolates two colors using an integer amount from zero through 255.
    ///
    /// Integer arithmetic keeps derived widget colors deterministic across platforms and avoids
    /// pulling floating-point color policy into the reusable theme crate.
    #[must_use]
    pub fn mix(self, other: Self, amount: u8) -> Self {
        let inverse = u16::from(u8::MAX.saturating_sub(amount));
        let amount = u16::from(amount);
        Self::new(
            mix_channel(self.red, other.red, inverse, amount),
            mix_channel(self.green, other.green, inverse, amount),
            mix_channel(self.blue, other.blue, inverse, amount),
            mix_channel(self.alpha, other.alpha, inverse, amount),
        )
    }
}

fn mix_channel(left: u8, right: u8, inverse: u16, amount: u16) -> u8 {
    let weighted = u16::from(left)
        .saturating_mul(inverse)
        .saturating_add(u16::from(right).saturating_mul(amount));
    u8::try_from(weighted.saturating_add(127) / 255).unwrap_or(u8::MAX)
}

/// Named theme presets exposed by Luna's native proofs and editor shell.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ThemePreset {
    /// Luna's neutral dark reference palette.
    LunaDark,
    /// Luna's neutral light reference palette.
    LunaLight,
    /// Warm amber/orange phosphor on black, inspired by vintage monochrome monitors.
    AmberMonitor,
    /// Bright green phosphor on black, inspired by classic terminal displays.
    GreenTerminal,
    /// Milky graphite and translucent blue inspired by late-1990s/early-2000s desktop hardware.
    Different,
}

impl ThemePreset {
    /// Every built-in preset in stable menu/gallery order.
    pub const ALL: [Self; 5] = [
        Self::LunaDark,
        Self::LunaLight,
        Self::AmberMonitor,
        Self::GreenTerminal,
        Self::Different,
    ];

    /// Stable command/session identifier.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::LunaDark => "luna-dark",
            Self::LunaLight => "luna-light",
            Self::AmberMonitor => "amber-monitor",
            Self::GreenTerminal => "green-terminal",
            Self::Different => "different",
        }
    }

    /// Human-readable menu label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LunaDark => "Luna Dark",
            Self::LunaLight => "Luna Light",
            Self::AmberMonitor => "Amber Monitor",
            Self::GreenTerminal => "Green Terminal",
            Self::Different => "Different",
        }
    }

    /// Resolves the semantic palette for this preset.
    #[must_use]
    pub const fn theme(self) -> Theme {
        match self {
            Self::LunaDark => Theme::luna_dark(),
            Self::LunaLight => Theme::luna_light(),
            Self::AmberMonitor => Theme::amber_monitor(),
            Self::GreenTerminal => Theme::green_terminal(),
            Self::Different => Theme::different(),
        }
    }

    /// Returns the next preset, wrapping at the end of [`Self::ALL`].
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::LunaDark => Self::LunaLight,
            Self::LunaLight => Self::AmberMonitor,
            Self::AmberMonitor => Self::GreenTerminal,
            Self::GreenTerminal => Self::Different,
            Self::Different => Self::LunaDark,
        }
    }

    /// Parses a stable preset identifier.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| preset.id() == id)
    }
}

/// Compact semantic palette shared by Luna's native proofs and reusable editor anatomy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Theme {
    /// Window/background surface.
    pub background: Rgba8,
    /// Elevated panel surface.
    pub panel: Rgba8,
    /// Panel header/chrome surface.
    pub panel_header: Rgba8,
    /// Accent/focus color.
    pub accent: Rgba8,
    /// Primary foreground color.
    pub foreground: Rgba8,
}

impl Theme {
    /// Luna's dark reference palette.
    #[must_use]
    pub const fn luna_dark() -> Self {
        Self {
            background: Rgba8::opaque(18, 20, 24),
            panel: Rgba8::opaque(37, 41, 48),
            panel_header: Rgba8::opaque(48, 53, 62),
            accent: Rgba8::opaque(130, 105, 255),
            foreground: Rgba8::opaque(232, 234, 238),
        }
    }

    /// Luna's light reference palette used by the proof gallery theme switch.
    #[must_use]
    pub const fn luna_light() -> Self {
        Self {
            background: Rgba8::opaque(239, 241, 246),
            panel: Rgba8::opaque(255, 255, 255),
            panel_header: Rgba8::opaque(220, 224, 233),
            accent: Rgba8::opaque(92, 64, 214),
            foreground: Rgba8::opaque(31, 35, 43),
        }
    }

    /// Vintage amber/orange phosphor palette on an almost-black background.
    #[must_use]
    pub const fn amber_monitor() -> Self {
        Self {
            background: Rgba8::opaque(5, 3, 0),
            panel: Rgba8::opaque(14, 8, 1),
            panel_header: Rgba8::opaque(28, 15, 2),
            accent: Rgba8::opaque(255, 132, 24),
            foreground: Rgba8::opaque(255, 179, 71),
        }
    }

    /// Bright green terminal phosphor palette on an almost-black background.
    #[must_use]
    pub const fn green_terminal() -> Self {
        Self {
            background: Rgba8::opaque(0, 5, 1),
            panel: Rgba8::opaque(0, 14, 3),
            panel_header: Rgba8::opaque(0, 28, 7),
            accent: Rgba8::opaque(0, 255, 102),
            foreground: Rgba8::opaque(103, 255, 136),
        }
    }

    /// Milky graphite, translucent blueberry, and candy-aqua desktop palette.
    ///
    /// The preset intentionally uses only Luna's semantic tokens. Its nostalgic character comes
    /// from cool translucent surfaces, dark graphite text, and one saturated focus color rather
    /// than product-specific assets or platform widgets.
    #[must_use]
    pub const fn different() -> Self {
        Self {
            background: Rgba8::opaque(222, 232, 239),
            panel: Rgba8::opaque(247, 250, 252),
            panel_header: Rgba8::opaque(166, 205, 225),
            accent: Rgba8::opaque(0, 122, 184),
            foreground: Rgba8::opaque(31, 45, 55),
        }
    }

    /// Muted foreground derived from the current panel and foreground tokens.
    #[must_use]
    pub fn muted_foreground(self) -> Rgba8 {
        self.foreground.mix(self.panel, 112)
    }

    /// Border/separator token derived from foreground and background.
    #[must_use]
    pub fn border(self) -> Rgba8 {
        self.foreground.mix(self.background, 190)
    }

    /// Hover surface derived from panel and accent.
    #[must_use]
    pub fn hover_surface(self) -> Rgba8 {
        self.panel.mix(self.accent, 48)
    }

    /// Selection surface with an explicit translucent accent.
    #[must_use]
    pub const fn selection(self) -> Rgba8 {
        self.accent.with_alpha(88)
    }
}

#[cfg(test)]
mod tests {
    use super::{Rgba8, Theme, ThemePreset};

    #[test]
    fn integer_color_mix_preserves_endpoints() {
        let left = Rgba8::opaque(10, 20, 30);
        let right = Rgba8::opaque(110, 120, 130);

        assert_eq!(left.mix(right, 0), left);
        assert_eq!(left.mix(right, u8::MAX), right);
    }

    #[test]
    fn light_and_dark_reference_palettes_are_distinct() {
        assert_ne!(Theme::luna_dark(), Theme::luna_light());
    }

    #[test]
    fn terminal_presets_are_distinct_and_cycle_stably() {
        assert_ne!(Theme::amber_monitor(), Theme::green_terminal());
        assert_eq!(ThemePreset::LunaDark.next(), ThemePreset::LunaLight);
        assert_eq!(ThemePreset::GreenTerminal.next(), ThemePreset::Different);
        assert_eq!(ThemePreset::Different.next(), ThemePreset::LunaDark);
        assert_eq!(
            ThemePreset::from_id("amber-monitor"),
            Some(ThemePreset::AmberMonitor)
        );
        assert_eq!(
            ThemePreset::from_id("different"),
            Some(ThemePreset::Different)
        );
    }

    #[test]
    fn different_preset_is_light_but_not_the_reference_light_palette() {
        let different = Theme::different();
        assert_ne!(different, Theme::luna_light());
        assert!(different.background.red > 200);
        assert!(different.accent.blue > different.accent.red);
    }

    #[test]
    fn all_presets_keep_dark_backgrounds_for_terminal_modes() {
        for preset in [ThemePreset::AmberMonitor, ThemePreset::GreenTerminal] {
            let theme = preset.theme();
            assert!(theme.background.red < 8);
            assert!(theme.background.green < 8);
            assert!(theme.background.blue < 8);
        }
    }
}
