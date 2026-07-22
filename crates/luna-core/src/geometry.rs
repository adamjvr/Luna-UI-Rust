// SPDX-License-Identifier: MPL-2.0

/// An integer point in Luna's logical coordinate space.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct PointI {
    /// Horizontal coordinate.
    pub x: i32,
    /// Vertical coordinate.
    pub y: i32,
}

impl PointI {
    /// Creates a point.
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// A non-negative integer size in Luna's logical coordinate space.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct SizeI {
    /// Width in logical pixels.
    pub width: u32,
    /// Height in logical pixels.
    pub height: u32,
}

impl SizeI {
    /// Creates a size.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Returns whether either dimension is zero.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// Integer insets used for padding and margins.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct InsetsI {
    /// Top inset.
    pub top: u32,
    /// Right inset.
    pub right: u32,
    /// Bottom inset.
    pub bottom: u32,
    /// Left inset.
    pub left: u32,
}

impl InsetsI {
    /// Creates four independent insets.
    #[must_use]
    pub const fn new(top: u32, right: u32, bottom: u32, left: u32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Creates equal horizontal and vertical insets.
    #[must_use]
    pub const fn symmetric(horizontal: u32, vertical: u32) -> Self {
        Self::new(vertical, horizontal, vertical, horizontal)
    }
}

/// An axis-aligned integer rectangle.
///
/// Width and height are unsigned, which rules out negative extents at the type level. Edge
/// calculations use a wider signed representation internally to avoid accidental `i32` overflow.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct RectI {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width in logical pixels.
    pub width: u32,
    /// Height in logical pixels.
    pub height: u32,
}

impl RectI {
    /// Creates a rectangle.
    #[must_use]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns whether the rectangle has no drawable area.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Returns the exclusive right edge using a widened representation.
    #[must_use]
    pub fn right(self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    /// Returns the exclusive bottom edge using a widened representation.
    #[must_use]
    pub fn bottom(self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }

    /// Returns whether the point lies inside the half-open rectangle.
    #[must_use]
    pub fn contains(self, point: PointI) -> bool {
        let point_x = i64::from(point.x);
        let point_y = i64::from(point.y);

        !self.is_empty()
            && point_x >= i64::from(self.x)
            && point_x < self.right()
            && point_y >= i64::from(self.y)
            && point_y < self.bottom()
    }

    /// Returns the overlapping rectangle, or `None` when the rectangles do not overlap.
    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let left = i64::from(self.x).max(i64::from(other.x));
        let top = i64::from(self.y).max(i64::from(other.y));
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        if right <= left || bottom <= top {
            return None;
        }

        Some(Self {
            x: i32::try_from(left).ok()?,
            y: i32::try_from(top).ok()?,
            width: u32::try_from(right - left).ok()?,
            height: u32::try_from(bottom - top).ok()?,
        })
    }

    /// Returns a rectangle inset on every side, saturating to an empty rectangle when needed.
    #[must_use]
    pub fn inset(self, insets: InsetsI) -> Self {
        let horizontal = insets.left.saturating_add(insets.right);
        let vertical = insets.top.saturating_add(insets.bottom);

        Self {
            x: self
                .x
                .saturating_add(i32::try_from(insets.left).unwrap_or(i32::MAX)),
            y: self
                .y
                .saturating_add(i32::try_from(insets.top).unwrap_or(i32::MAX)),
            width: self.width.saturating_sub(horizontal),
            height: self.height.saturating_sub(vertical),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InsetsI, PointI, RectI};

    #[test]
    fn contains_uses_half_open_edges() {
        let rectangle = RectI::new(10, 20, 30, 40);

        assert!(rectangle.contains(PointI::new(10, 20)));
        assert!(rectangle.contains(PointI::new(39, 59)));
        assert!(!rectangle.contains(PointI::new(40, 59)));
        assert!(!rectangle.contains(PointI::new(39, 60)));
    }

    #[test]
    fn inset_saturates_instead_of_underflowing() {
        let rectangle = RectI::new(0, 0, 4, 4);
        let inset = rectangle.inset(InsetsI::symmetric(8, 8));

        assert!(inset.is_empty());
    }
}
