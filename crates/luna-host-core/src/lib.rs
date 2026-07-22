// SPDX-License-Identifier: MPL-2.0

//! Platform-neutral host/runtime contracts.
//!
//! A native host owns the operating-system event loop, window, presentation surface, dialogs, and
//! accessibility adapter. This crate owns deterministic frame invalidation state that can be tested
//! without creating a native window.

use std::collections::BTreeSet;

/// Why a new frame is required.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum InvalidationReason {
    /// First frame after surface creation.
    InitialFrame,
    /// Widget/application state changed.
    StateChanged,
    /// Surface dimensions changed.
    SurfaceResized,
    /// Theme tokens changed.
    ThemeChanged,
    /// Animation needs another sample.
    Animation,
    /// Explicit application-defined reason.
    Explicit(String),
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
    use super::{FrameRuntime, InvalidationReason};

    #[test]
    fn duplicate_invalidations_coalesce() -> Result<(), Box<dyn std::error::Error>> {
        let mut runtime = FrameRuntime::new();
        runtime.request_frame(InvalidationReason::StateChanged);
        runtime.request_frame(InvalidationReason::StateChanged);

        let frame = runtime
            .begin_frame(100)
            .ok_or_else(|| std::io::Error::other("a requested frame should begin"))?;

        assert_eq!(frame.invalidations.iter().count(), 2);
        assert!(runtime.begin_frame(101).is_none());
        Ok(())
    }
}
