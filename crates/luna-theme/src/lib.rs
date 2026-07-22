// SPDX-License-Identifier: MPL-2.0

//! Theme tokens shared by widgets and renderers.
//!
//! M0 intentionally starts with strongly typed color tokens. Sublime-compatible theme parsing is
//! a later adapter layer and will populate this product-neutral structure rather than leaking file
//! format concerns into widgets.

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
}

/// Minimal semantic theme used by the M0 proof surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Theme {
    /// Window/background surface.
    pub background: Rgba8,
    /// Elevated panel surface.
    pub panel: Rgba8,
    /// Panel header surface.
    pub panel_header: Rgba8,
    /// Accent/focus color.
    pub accent: Rgba8,
    /// Primary foreground color reserved for the text phase.
    pub foreground: Rgba8,
}

impl Theme {
    /// Luna's initial dark reference palette.
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
}
