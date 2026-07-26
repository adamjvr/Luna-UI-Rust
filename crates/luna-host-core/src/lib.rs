// SPDX-License-Identifier: MPL-2.0

//! Platform-neutral host/runtime contracts.
//!
//! A native host owns the operating-system event loop, window, presentation surface, dialogs, and
//! accessibility adapter. This crate owns deterministic frame invalidation state that can be tested
//! without creating a native window.

use std::collections::BTreeSet;

/// Native platform family used by Luna's documented support policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NativePlatform {
    /// Linux desktop environments are the primary development and release target.
    Linux,
    /// macOS is the supported secondary target undergoing real-hardware hardening.
    MacOs,
    /// Windows may compile through upstream crates but is not an official project target.
    Windows,
    /// Another platform not covered by Luna's desktop support policy.
    Other,
}

/// Project commitment level for one native platform.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PlatformSupportTier {
    /// Blocking CI, primary graphical acceptance, packaging, and release support.
    Primary,
    /// Maintained native target with advisory CI until repeated hardware acceptance is recorded.
    Secondary,
    /// Community-compatible when practical, without CI, packaging, or release guarantees.
    BestEffort,
    /// Outside the current desktop project scope.
    Unsupported,
}

/// Returns Luna's support tier for a platform without consulting runtime state.
#[must_use]
pub const fn platform_support_tier(platform: NativePlatform) -> PlatformSupportTier {
    match platform {
        NativePlatform::Linux => PlatformSupportTier::Primary,
        NativePlatform::MacOs => PlatformSupportTier::Secondary,
        NativePlatform::Windows => PlatformSupportTier::BestEffort,
        NativePlatform::Other => PlatformSupportTier::Unsupported,
    }
}

/// Returns the platform family compiled into the current binary.
#[must_use]
pub const fn current_native_platform() -> NativePlatform {
    if cfg!(target_os = "linux") {
        NativePlatform::Linux
    } else if cfg!(target_os = "macos") {
        NativePlatform::MacOs
    } else if cfg!(target_os = "windows") {
        NativePlatform::Windows
    } else {
        NativePlatform::Other
    }
}

/// Stable high-level category describing why frame work is required.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InvalidationClass {
    /// First frame after native resources are created.
    Initial,
    /// Time-driven animation or proof sampling.
    Animation,
    /// Caret, selection, focus, or other paint-only overlays changed.
    PaintOverlay,
    /// Text-adjacent overlay content changed without document layout changes.
    TextOverlay,
    /// Glyph pixels must be regenerated while logical text geometry remains valid.
    TextRaster,
    /// Logical text shaping and caret geometry must be rebuilt.
    TextLayout,
    /// Widget or shell geometry must be rebuilt.
    WidgetLayout,
    /// Accessibility semantics or focus changed.
    Accessibility,
    /// Native surface size, scale, exposure, or lifecycle changed.
    Surface,
    /// A complete application frame rebuild is required.
    FullFrame,
    /// Explicit application-defined diagnostic work.
    Explicit,
}

/// Why a new frame is required.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InvalidationReason {
    /// First frame after surface creation.
    InitialFrame,
    /// Widget/application state changed.
    StateChanged,
    /// Surface dimensions changed.
    SurfaceResized,
    /// A platform expose event requested repaint without queued Luna work.
    SurfaceExposed,
    /// Theme tokens changed.
    ThemeChanged,
    /// Animation needs another sample.
    Animation,
    /// Caret, selection, focus, or another paint-only overlay changed.
    PaintOverlay,
    /// Text-adjacent overlay content changed.
    TextOverlay,
    /// Glyph pixels require regeneration.
    TextRaster,
    /// Text shaping or logical geometry requires regeneration.
    TextLayout,
    /// Widget layout requires regeneration.
    WidgetLayout,
    /// Accessibility semantics or focus changed.
    AccessibilityChanged,
    /// A complete immutable frame rebuild is required.
    FullFrame,
    /// Application-selected high-level category without diagnostic text.
    Classified(InvalidationClass),
    /// Explicit application-defined reason with diagnostic text.
    Explicit(String),
}

impl InvalidationReason {
    /// Returns the stable diagnostic class for this concrete reason.
    #[must_use]
    pub const fn class(&self) -> InvalidationClass {
        match self {
            Self::InitialFrame => InvalidationClass::Initial,
            Self::Animation => InvalidationClass::Animation,
            Self::PaintOverlay => InvalidationClass::PaintOverlay,
            Self::TextOverlay => InvalidationClass::TextOverlay,
            Self::TextRaster => InvalidationClass::TextRaster,
            Self::TextLayout => InvalidationClass::TextLayout,
            Self::WidgetLayout => InvalidationClass::WidgetLayout,
            Self::AccessibilityChanged => InvalidationClass::Accessibility,
            Self::SurfaceResized | Self::SurfaceExposed => InvalidationClass::Surface,
            Self::StateChanged | Self::ThemeChanged | Self::FullFrame => {
                InvalidationClass::FullFrame
            }
            Self::Classified(class) => *class,
            Self::Explicit(_) => InvalidationClass::Explicit,
        }
    }
}

/// Coalesced reasons for producing a frame.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InvalidationSet {
    reasons: BTreeSet<InvalidationReason>,
}

impl InvalidationSet {
    /// Adds a reason. Duplicate reasons are coalesced automatically.
    pub fn insert(&mut self, reason: InvalidationReason) {
        self.reasons.insert(reason);
    }

    /// Returns whether no frame is currently required.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.reasons.is_empty()
    }

    /// Iterates reasons in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = &InvalidationReason> {
        self.reasons.iter()
    }

    /// Removes all reasons and returns the resulting snapshot.
    pub fn take(&mut self) -> Self {
        Self {
            reasons: std::mem::take(&mut self.reasons),
        }
    }
}

/// Token proving that the runtime began a specific frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameToken {
    /// Monotonically increasing frame number.
    pub frame_number: u64,
    /// Monotonic frame-start timestamp in microseconds.
    pub started_at_micros: u64,
    /// Reasons consumed by this frame.
    pub invalidations: InvalidationSet,
}

/// Timing result recorded after a frame is presented.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameStats {
    /// Frame number.
    pub frame_number: u64,
    /// CPU-side elapsed duration in microseconds.
    pub elapsed_micros: u64,
}

/// Small deterministic frame scheduler state machine.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FrameRuntime {
    next_frame_number: u64,
    invalidations: InvalidationSet,
}

impl FrameRuntime {
    /// Creates a runtime that requests its initial frame.
    #[must_use]
    pub fn new() -> Self {
        let mut runtime = Self::default();
        runtime.request_frame(InvalidationReason::InitialFrame);
        runtime
    }

    /// Requests a future frame for the supplied reason.
    pub fn request_frame(&mut self, reason: InvalidationReason) {
        self.invalidations.insert(reason);
    }

    /// Returns whether at least one reason is waiting to be consumed.
    #[must_use]
    pub fn has_pending_frame(&self) -> bool {
        !self.invalidations.is_empty()
    }

    /// Begins a frame when work is pending, consuming current invalidations.
    pub fn begin_frame(&mut self, now_micros: u64) -> Option<FrameToken> {
        if self.invalidations.is_empty() {
            return None;
        }

        let frame_number = self.next_frame_number;
        self.next_frame_number = self.next_frame_number.saturating_add(1);

        Some(FrameToken {
            frame_number,
            started_at_micros: now_micros,
            invalidations: self.invalidations.take(),
        })
    }

    /// Records presentation timing for a completed frame.
    #[must_use]
    pub fn finish_frame(token: &FrameToken, presented_at_micros: u64) -> FrameStats {
        FrameStats {
            frame_number: token.frame_number,
            elapsed_micros: presented_at_micros.saturating_sub(token.started_at_micros),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FrameRuntime, InvalidationClass, InvalidationReason, NativePlatform, PlatformSupportTier,
        platform_support_tier,
    };

    #[test]
    fn support_policy_keeps_windows_best_effort() {
        assert_eq!(
            platform_support_tier(NativePlatform::Linux),
            PlatformSupportTier::Primary
        );
        assert_eq!(
            platform_support_tier(NativePlatform::MacOs),
            PlatformSupportTier::Secondary
        );
        assert_eq!(
            platform_support_tier(NativePlatform::Windows),
            PlatformSupportTier::BestEffort
        );
    }

    #[test]
    fn duplicate_invalidations_coalesce() -> Result<(), Box<dyn std::error::Error>> {
        let mut runtime = FrameRuntime::new();
        runtime.request_frame(InvalidationReason::StateChanged);
        runtime.request_frame(InvalidationReason::StateChanged);

        let frame = runtime
            .begin_frame(100)
            .ok_or_else(|| std::io::Error::other("a requested frame should begin"))?;

        assert_eq!(frame.invalidations.iter().count(), 2);
        assert!(!runtime.has_pending_frame());
        assert!(runtime.begin_frame(101).is_none());
        Ok(())
    }

    #[test]
    fn reasons_map_to_stable_classes() {
        assert_eq!(
            InvalidationReason::Animation.class(),
            InvalidationClass::Animation
        );
        assert_eq!(
            InvalidationReason::TextLayout.class(),
            InvalidationClass::TextLayout
        );
        assert_eq!(
            InvalidationReason::ThemeChanged.class(),
            InvalidationClass::FullFrame
        );
        assert_eq!(
            InvalidationReason::Classified(InvalidationClass::PaintOverlay).class(),
            InvalidationClass::PaintOverlay
        );
    }
}
