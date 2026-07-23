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
    use super::{Rgba8, Theme};

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
}
