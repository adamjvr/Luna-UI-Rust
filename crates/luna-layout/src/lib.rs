// SPDX-License-Identifier: MPL-2.0

//! Deterministic reusable layout primitives for Luna UI Rust.
//!
//! Layout in Luna is a pure calculation. A caller supplies integer logical-pixel constraints and
//! receives an immutable snapshot of child rectangles. Rendering, hit testing, and accessibility
//! then consume that same snapshot instead of independently recomputing geometry.
//!
//! M1 deliberately keeps the model compact: linear rows and columns, stacks, and two-pane splits.
//! These primitives are sufficient to build editor shells while leaving text measurement and
//! product-specific policy to later layers.

use luna_core::{InsetsI, NodeId, RectI, SizeI};
use std::collections::BTreeMap;

/// The primary direction in which a linear layout places children.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Axis {
    /// Children advance from left to right.
    Horizontal,
    /// Children advance from top to bottom.
    Vertical,
}

/// Alignment on the axis perpendicular to a linear layout's main axis.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum CrossAlignment {
    /// Place the child at the leading cross-axis edge.
    #[default]
    Start,
    /// Center the child within the available cross-axis extent.
    Center,
    /// Place the child at the trailing cross-axis edge.
    End,
    /// Stretch the child to fill the available cross-axis extent.
    Stretch,
}

/// Requested main-axis behavior for one linear-layout child.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MainAxisSize {
    /// Reserve an exact logical-pixel extent, clipped when the container is too small.
    Fixed(u32),
    /// Share remaining room proportionally with other flexible children.
    Flex {
        /// Relative share of remaining room. A zero weight is treated as one.
        weight: u16,
    },
}

/// Requested cross-axis behavior for one linear-layout child.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CrossAxisSize {
    /// Use a fixed extent, clipped to the available cross-axis room.
    Fixed(u32),
    /// Fill the available cross-axis room.
    Stretch,
}

/// One child request supplied to [`LinearLayout`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinearItem {
    /// Stable identity assigned to the output frame.
    pub id: NodeId,
    /// Main-axis sizing behavior.
    pub main: MainAxisSize,
    /// Cross-axis sizing behavior.
    pub cross: CrossAxisSize,
}

impl LinearItem {
    /// Creates a fixed-size child that stretches across the cross axis.
    #[must_use]
    pub fn fixed(id: NodeId, extent: u32) -> Self {
        Self {
            id,
            main: MainAxisSize::Fixed(extent),
            cross: CrossAxisSize::Stretch,
        }
    }

    /// Creates a flexible child that stretches across the cross axis.
    #[must_use]
    pub fn flex(id: NodeId, weight: u16) -> Self {
        Self {
            id,
            main: MainAxisSize::Flex { weight },
            cross: CrossAxisSize::Stretch,
        }
    }

    /// Replaces the cross-axis sizing behavior.
    #[must_use]
    pub const fn with_cross(mut self, cross: CrossAxisSize) -> Self {
        self.cross = cross;
        self
    }
}

/// Immutable rectangle assigned to one stable child ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutFrame {
    /// Stable child identity.
    pub id: NodeId,
    /// Assigned logical-pixel rectangle.
    pub bounds: RectI,
}

/// Immutable result of a layout pass.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LayoutSnapshot {
    frames: Vec<LayoutFrame>,
    by_id: BTreeMap<NodeId, usize>,
}

impl LayoutSnapshot {
    /// Creates a validated snapshot from frames in paint/traversal order.
    ///
    /// Duplicate IDs are resolved deterministically by keeping the first frame. Layout callers are
    /// expected to use unique IDs, but this fail-closed behavior prevents later geometry systems
    /// from disagreeing over which duplicate should win.
    #[must_use]
    pub fn new(frames: impl IntoIterator<Item = LayoutFrame>) -> Self {
        let mut ordered = Vec::new();
        let mut by_id = BTreeMap::new();

        for frame in frames {
            if by_id.contains_key(&frame.id) {
                continue;
            }
            let index = ordered.len();
            by_id.insert(frame.id.clone(), index);
            ordered.push(frame);
        }

        Self {
            frames: ordered,
            by_id,
        }
    }

    /// Returns frames in deterministic traversal order.
    #[must_use]
    pub fn frames(&self) -> &[LayoutFrame] {
        &self.frames
    }

    /// Looks up the bounds assigned to a stable child ID.
    #[must_use]
    pub fn bounds(&self, id: &NodeId) -> Option<RectI> {
        self.by_id
            .get(id)
            .and_then(|index| self.frames.get(*index))
            .map(|frame| frame.bounds)
    }

    /// Returns whether no child frames were produced.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

/// Pure row/column layout request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinearLayout {
    /// Placement direction.
    pub axis: Axis,
    /// Outer container bounds.
    pub bounds: RectI,
    /// Padding removed before children are placed.
    pub padding: InsetsI,
    /// Gap inserted between neighboring children.
    pub gap: u32,
    /// Default cross-axis alignment for non-stretching children.
    pub cross_alignment: CrossAlignment,
    /// Ordered child sizing requests.
    pub items: Vec<LinearItem>,
}

impl LinearLayout {
    /// Calculates child frames using integer arithmetic and deterministic remainder distribution.
    #[must_use]
    pub fn calculate(&self) -> LayoutSnapshot {
        let content = self.bounds.inset(self.padding);
        if self.items.is_empty() {
            return LayoutSnapshot::default();
        }

        let gap_count = u32::try_from(self.items.len().saturating_sub(1)).unwrap_or(u32::MAX);
        let requested_gap = self.gap.saturating_mul(gap_count);
        let main_extent = axis_extent(content, self.axis);
        let gap_room = requested_gap.min(main_extent);
        let distributable = main_extent.saturating_sub(gap_room);

        let fixed_total = self
            .items
            .iter()
            .map(|item| match item.main {
                MainAxisSize::Fixed(value) => value,
                MainAxisSize::Flex { .. } => 0,
            })
            .fold(0_u32, u32::saturating_add);
        let flex_room = distributable.saturating_sub(fixed_total.min(distributable));
        let flex_weight = self
            .items
            .iter()
            .map(|item| match item.main {
                MainAxisSize::Fixed(_) => 0_u32,
                MainAxisSize::Flex { weight } => u32::from(weight.max(1)),
            })
            .fold(0_u32, u32::saturating_add);

        let mut remaining_fixed_room = fixed_total.min(distributable);
        let mut remaining_flex_room = flex_room;
        let mut remaining_flex_weight = flex_weight;
        let mut remaining_gap_room = gap_room;
        let mut remaining_gap_count = gap_count;
        let mut cursor = axis_origin(content, self.axis);
        let mut frames = Vec::with_capacity(self.items.len());

        for (index, item) in self.items.iter().enumerate() {
            let main = match item.main {
                MainAxisSize::Fixed(requested) => {
                    let assigned = requested.min(remaining_fixed_room);
                    remaining_fixed_room = remaining_fixed_room.saturating_sub(assigned);
                    assigned
                }
                MainAxisSize::Flex { weight } => {
                    let weight = u32::from(weight.max(1));
                    let assigned = if remaining_flex_weight == 0 {
                        0
                    } else {
                        // Dividing the current remainder instead of the original total guarantees
                        // that integer truncation is distributed deterministically and the final
                        // flexible child receives every leftover pixel.
                        let numerator = u64::from(remaining_flex_room) * u64::from(weight);
                        u32::try_from(numerator / u64::from(remaining_flex_weight))
                            .unwrap_or(remaining_flex_room)
                            .min(remaining_flex_room)
                    };
                    remaining_flex_room = remaining_flex_room.saturating_sub(assigned);
                    remaining_flex_weight = remaining_flex_weight.saturating_sub(weight);
                    assigned
                }
            };

            let cross_available = cross_extent(content, self.axis);
            let cross = match item.cross {
                CrossAxisSize::Fixed(requested) => requested.min(cross_available),
                CrossAxisSize::Stretch => cross_available,
            };
            let cross_offset = cross_offset(
                cross_available,
                cross,
                match item.cross {
                    CrossAxisSize::Stretch => CrossAlignment::Stretch,
                    CrossAxisSize::Fixed(_) => self.cross_alignment,
                },
            );

            let bounds = compose_rect(content, self.axis, cursor, main, cross_offset, cross);
            frames.push(LayoutFrame {
                id: item.id.clone(),
                bounds,
            });

            let gap_after = if index + 1 < self.items.len() && remaining_gap_count > 0 {
                // Equal gaps divide the current remainder. When the container is narrower than the
                // requested gaps, the final gaps receive any leftover pixels without crossing the
                // content edge.
                let assigned = remaining_gap_room / remaining_gap_count;
                remaining_gap_room = remaining_gap_room.saturating_sub(assigned);
                remaining_gap_count = remaining_gap_count.saturating_sub(1);
                assigned
            } else {
                0
            };
            cursor = cursor
                .saturating_add(i32::try_from(main).unwrap_or(i32::MAX))
                .saturating_add(i32::try_from(gap_after).unwrap_or(i32::MAX));
        }

        LayoutSnapshot::new(frames)
    }
}

/// One child in a stack layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackItem {
    /// Stable identity assigned to the output frame.
    pub id: NodeId,
    /// Optional desired size. `None` fills the stack's content bounds.
    pub size: Option<SizeI>,
    /// Horizontal alignment used when `size` is present.
    pub horizontal: CrossAlignment,
    /// Vertical alignment used when `size` is present.
    pub vertical: CrossAlignment,
}

/// Pure overlapping stack layout request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackLayout {
    /// Outer container bounds.
    pub bounds: RectI,
    /// Padding removed before children are placed.
    pub padding: InsetsI,
    /// Children in back-to-front paint order.
    pub items: Vec<StackItem>,
}

impl StackLayout {
    /// Calculates overlapping child frames.
    #[must_use]
    pub fn calculate(&self) -> LayoutSnapshot {
        let content = self.bounds.inset(self.padding);
        let frames = self.items.iter().map(|item| {
            let bounds = item.size.map_or(content, |size| {
                let width = size.width.min(content.width);
                let height = size.height.min(content.height);
                RectI::new(
                    aligned_origin(content.x, content.width, width, item.horizontal),
                    aligned_origin(content.y, content.height, height, item.vertical),
                    width,
                    height,
                )
            });
            LayoutFrame {
                id: item.id.clone(),
                bounds,
            }
        });
        LayoutSnapshot::new(frames)
    }
}

/// Result of dividing a rectangle into two panes and a divider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplitSnapshot {
    /// First/leading pane.
    pub first: RectI,
    /// Divider hit and paint bounds.
    pub divider: RectI,
    /// Second/trailing pane.
    pub second: RectI,
}

/// Pure two-pane split request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplitLayout {
    /// Outer split bounds.
    pub bounds: RectI,
    /// Horizontal creates left/right panes; vertical creates top/bottom panes.
    pub axis: Axis,
    /// Leading pane ratio in thousandths, clamped to `0..=1000`.
    pub ratio_per_mille: u16,
    /// Divider thickness in logical pixels.
    pub divider_extent: u32,
    /// Minimum leading pane extent.
    pub minimum_first: u32,
    /// Minimum trailing pane extent.
    pub minimum_second: u32,
}

impl SplitLayout {
    /// Calculates the leading pane, divider, and trailing pane rectangles.
    #[must_use]
    pub fn calculate(self) -> SplitSnapshot {
        let total = axis_extent(self.bounds, self.axis);
        let divider = self.divider_extent.min(total);
        let pane_room = total.saturating_sub(divider);
        let preferred =
            u32::try_from(u64::from(pane_room) * u64::from(self.ratio_per_mille.min(1000)) / 1000)
                .unwrap_or(pane_room);

        let first_min = self.minimum_first.min(pane_room);
        let second_min = self.minimum_second.min(pane_room.saturating_sub(first_min));
        let maximum_first = pane_room.saturating_sub(second_min);
        let first_extent = preferred.clamp(first_min, maximum_first.max(first_min));
        let second_extent = pane_room.saturating_sub(first_extent);
        let origin = axis_origin(self.bounds, self.axis);
        let divider_origin = origin.saturating_add(i32::try_from(first_extent).unwrap_or(i32::MAX));
        let second_origin =
            divider_origin.saturating_add(i32::try_from(divider).unwrap_or(i32::MAX));

        SplitSnapshot {
            first: rect_on_axis(self.bounds, self.axis, origin, first_extent),
            divider: rect_on_axis(self.bounds, self.axis, divider_origin, divider),
            second: rect_on_axis(self.bounds, self.axis, second_origin, second_extent),
        }
    }
}

fn axis_extent(bounds: RectI, axis: Axis) -> u32 {
    match axis {
        Axis::Horizontal => bounds.width,
        Axis::Vertical => bounds.height,
    }
}

fn cross_extent(bounds: RectI, axis: Axis) -> u32 {
    match axis {
        Axis::Horizontal => bounds.height,
        Axis::Vertical => bounds.width,
    }
}

fn axis_origin(bounds: RectI, axis: Axis) -> i32 {
    match axis {
        Axis::Horizontal => bounds.x,
        Axis::Vertical => bounds.y,
    }
}

fn compose_rect(
    content: RectI,
    axis: Axis,
    main_origin: i32,
    main_extent: u32,
    cross_offset: u32,
    cross_extent: u32,
) -> RectI {
    let cross_offset = i32::try_from(cross_offset).unwrap_or(i32::MAX);
    match axis {
        Axis::Horizontal => RectI::new(
            main_origin,
            content.y.saturating_add(cross_offset),
            main_extent,
            cross_extent,
        ),
        Axis::Vertical => RectI::new(
            content.x.saturating_add(cross_offset),
            main_origin,
            cross_extent,
            main_extent,
        ),
    }
}

fn cross_offset(available: u32, assigned: u32, alignment: CrossAlignment) -> u32 {
    let spare = available.saturating_sub(assigned);
    match alignment {
        CrossAlignment::Start | CrossAlignment::Stretch => 0,
        CrossAlignment::Center => spare / 2,
        CrossAlignment::End => spare,
    }
}

fn aligned_origin(origin: i32, available: u32, assigned: u32, alignment: CrossAlignment) -> i32 {
    origin.saturating_add(
        i32::try_from(cross_offset(available, assigned, alignment)).unwrap_or(i32::MAX),
    )
}

fn rect_on_axis(bounds: RectI, axis: Axis, origin: i32, extent: u32) -> RectI {
    match axis {
        Axis::Horizontal => RectI::new(origin, bounds.y, extent, bounds.height),
        Axis::Vertical => RectI::new(bounds.x, origin, bounds.width, extent),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Axis, CrossAlignment, CrossAxisSize, LinearItem, LinearLayout, MainAxisSize, SplitLayout,
        StackItem, StackLayout,
    };
    use luna_core::{InsetsI, NodeId, RectI, SizeI};
    use std::error::Error;

    #[test]
    fn row_distributes_flex_remainder_without_losing_pixels() -> Result<(), Box<dyn Error>> {
        let first = NodeId::new("first")?;
        let second = NodeId::new("second")?;
        let third = NodeId::new("third")?;
        let snapshot = LinearLayout {
            axis: Axis::Horizontal,
            bounds: RectI::new(0, 0, 101, 20),
            padding: InsetsI::default(),
            gap: 1,
            cross_alignment: CrossAlignment::Stretch,
            items: vec![
                LinearItem::fixed(first.clone(), 20),
                LinearItem::flex(second.clone(), 1),
                LinearItem::flex(third.clone(), 1),
            ],
        }
        .calculate();

        assert_eq!(snapshot.bounds(&first), Some(RectI::new(0, 0, 20, 20)));
        assert_eq!(snapshot.bounds(&second), Some(RectI::new(21, 0, 39, 20)));
        assert_eq!(snapshot.bounds(&third), Some(RectI::new(61, 0, 40, 20)));
        Ok(())
    }

    #[test]
    fn flex_before_oversized_fixed_child_stays_inside_container() -> Result<(), Box<dyn Error>> {
        let flexible = NodeId::new("flexible")?;
        let fixed = NodeId::new("fixed")?;
        let snapshot = LinearLayout {
            axis: Axis::Horizontal,
            bounds: RectI::new(0, 0, 50, 10),
            padding: InsetsI::default(),
            gap: 0,
            cross_alignment: CrossAlignment::Stretch,
            items: vec![
                LinearItem::flex(flexible.clone(), 1),
                LinearItem::fixed(fixed.clone(), 80),
            ],
        }
        .calculate();

        assert_eq!(snapshot.bounds(&flexible), Some(RectI::new(0, 0, 0, 10)));
        assert_eq!(snapshot.bounds(&fixed), Some(RectI::new(0, 0, 50, 10)));
        Ok(())
    }

    #[test]
    fn gaps_saturate_when_container_is_too_small() -> Result<(), Box<dyn Error>> {
        let first = NodeId::new("first")?;
        let second = NodeId::new("second")?;
        let third = NodeId::new("third")?;
        let snapshot = LinearLayout {
            axis: Axis::Horizontal,
            bounds: RectI::new(0, 0, 3, 10),
            padding: InsetsI::default(),
            gap: 10,
            cross_alignment: CrossAlignment::Stretch,
            items: vec![
                LinearItem::flex(first.clone(), 1),
                LinearItem::flex(second.clone(), 1),
                LinearItem::flex(third.clone(), 1),
            ],
        }
        .calculate();

        assert_eq!(snapshot.bounds(&first), Some(RectI::new(0, 0, 0, 10)));
        assert_eq!(snapshot.bounds(&second), Some(RectI::new(1, 0, 0, 10)));
        assert_eq!(snapshot.bounds(&third), Some(RectI::new(3, 0, 0, 10)));
        Ok(())
    }

    #[test]
    fn fixed_cross_axis_children_align_from_shared_snapshot() -> Result<(), Box<dyn Error>> {
        let child = NodeId::new("child")?;
        let snapshot = LinearLayout {
            axis: Axis::Horizontal,
            bounds: RectI::new(10, 20, 80, 40),
            padding: InsetsI::default(),
            gap: 0,
            cross_alignment: CrossAlignment::Center,
            items: vec![LinearItem {
                id: child.clone(),
                main: MainAxisSize::Fixed(30),
                cross: CrossAxisSize::Fixed(10),
            }],
        }
        .calculate();

        assert_eq!(snapshot.bounds(&child), Some(RectI::new(10, 35, 30, 10)));
        Ok(())
    }

    #[test]
    fn stack_preserves_back_to_front_order() -> Result<(), Box<dyn Error>> {
        let background = NodeId::new("background")?;
        let overlay = NodeId::new("overlay")?;
        let snapshot = StackLayout {
            bounds: RectI::new(0, 0, 100, 80),
            padding: InsetsI::symmetric(10, 10),
            items: vec![
                StackItem {
                    id: background.clone(),
                    size: None,
                    horizontal: CrossAlignment::Stretch,
                    vertical: CrossAlignment::Stretch,
                },
                StackItem {
                    id: overlay.clone(),
                    size: Some(SizeI::new(20, 10)),
                    horizontal: CrossAlignment::End,
                    vertical: CrossAlignment::End,
                },
            ],
        }
        .calculate();

        assert_eq!(snapshot.frames()[0].id, background);
        assert_eq!(snapshot.bounds(&overlay), Some(RectI::new(70, 60, 20, 10)));
        Ok(())
    }

    #[test]
    fn split_honors_divider_and_minimums() {
        let split = SplitLayout {
            bounds: RectI::new(0, 0, 100, 40),
            axis: Axis::Horizontal,
            ratio_per_mille: 100,
            divider_extent: 4,
            minimum_first: 30,
            minimum_second: 20,
        }
        .calculate();

        assert_eq!(split.first, RectI::new(0, 0, 30, 40));
        assert_eq!(split.divider, RectI::new(30, 0, 4, 40));
        assert_eq!(split.second, RectI::new(34, 0, 66, 40));
    }
}
