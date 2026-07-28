// SPDX-License-Identifier: MPL-2.0

use super::{WorkloadResult, invariant, require, usize_to_u64};
use crate::report::WorkloadReport;
use luna_core::{PointI, RectI};
use luna_documents::{DocumentRegistry, DocumentViewId, DocumentViewRegistry};
use luna_panes::{PaneAxis, PaneLayoutMetrics, PaneTree};

const WORKLOAD: &str = "pane_tab_cycles";
const MAXIMUM_LEAVES: u64 = 2;
const MAXIMUM_SPLITTERS: u64 = 1;
const MAXIMUM_VIEWS: u64 = 4;

pub(super) fn run(cycles: u32) -> WorkloadResult<WorkloadReport> {
    let mut maximum_leaves = 0_u64;
    let mut maximum_splitters = 0_u64;
    let mut maximum_views = 0_u64;
    let mut closed_views = 0_u64;

    for cycle in 0..cycles {
        let mut documents = DocumentRegistry::new();
        let mut views = DocumentViewRegistry::new();
        let first_view = view(&mut documents, &mut views, cycle, "one")?;
        let second_view = view(&mut documents, &mut views, cycle, "two")?;
        let third_view = view(&mut documents, &mut views, cycle, "three")?;
        let fourth_view = view(&mut documents, &mut views, cycle, "four")?;
        maximum_views = maximum_views.max(usize_to_u64(views.views().len()));

        let mut panes = PaneTree::new(first_view);
        let first_pane = panes.focused_pane();
        panes.add_view(first_pane, second_view)?;
        panes.add_view(first_pane, third_view)?;
        panes.pin_view(first_pane, third_view)?;
        require(
            WORKLOAD,
            panes.set_preview_view(first_pane, second_view)?.is_none(),
            "first preview assignment unexpectedly replaced a view",
        )?;
        panes.reorder_view(first_pane, second_view, 0)?;
        panes.promote_preview(first_pane, second_view)?;

        let second_pane = panes.split_focused(PaneAxis::Horizontal, fourth_view);
        panes.focus(first_pane)?;
        panes.activate_view(first_pane, second_view)?;
        let moved_to = panes.move_active_tab_to_next_pane()?;
        require(
            WORKLOAD,
            moved_to == second_pane,
            "active tab did not move to the adjacent pane",
        )?;
        require(
            WORKLOAD,
            panes.pane_for_view(second_view) == Some(second_pane),
            "moved tab ownership did not update",
        )?;

        let split_id = panes
            .splits()
            .first()
            .map(|split| split.id())
            .ok_or(invariant(WORKLOAD, "split tree contains no splitter"))?;
        panes.set_split_ratio_from_point(
            split_id,
            RectI::new(0, 0, 1_200, 800),
            PointI::new(780, 400),
        )?;

        let snapshot = panes.snapshot_with(DocumentViewId::value);
        let restored = PaneTree::restore_with(&snapshot, |key| {
            views
                .views()
                .iter()
                .find(|view| view.id().value() == key)
                .map(|view| view.id())
        })?;
        require(
            WORKLOAD,
            restored == panes,
            "pane snapshot did not round-trip exactly",
        )?;

        let width = 960_u32.saturating_add(cycle % 4 * 160);
        let height = 540_u32.saturating_add(cycle % 3 * 120);
        let layout = panes.layout(
            RectI::new(0, 0, width, height),
            PaneLayoutMetrics::default(),
        );
        maximum_leaves = maximum_leaves.max(usize_to_u64(layout.leaves.len()));
        maximum_splitters = maximum_splitters.max(usize_to_u64(layout.splitters.len()));

        panes.focus(first_pane)?;
        let close = panes.close_focused_pane()?;
        closed_views = closed_views.saturating_add(usize_to_u64(close.removed_views.len()));
        require(
            WORKLOAD,
            panes.leaves().len() == 1,
            "closing one side of the split must collapse to one leaf",
        )?;
        require(
            WORKLOAD,
            panes.splits().is_empty(),
            "collapsed pane tree retained an orphan splitter",
        )?;
    }

    require(
        WORKLOAD,
        maximum_views <= MAXIMUM_VIEWS,
        "view high-water mark exceeded the fixed fixture",
    )?;
    require(
        WORKLOAD,
        maximum_leaves <= MAXIMUM_LEAVES,
        "pane-leaf high-water mark exceeded the fixed fixture",
    )?;
    require(
        WORKLOAD,
        maximum_splitters <= MAXIMUM_SPLITTERS,
        "splitter high-water mark exceeded the fixed fixture",
    )?;

    let mut report = WorkloadReport::new(WORKLOAD);
    report.record("cycles", u64::from(cycles));
    report.record("views_created", u64::from(cycles).saturating_mul(4));
    report.record("tab_moves", u64::from(cycles));
    report.record("snapshot_round_trips", u64::from(cycles));
    report.record("pane_closes", u64::from(cycles));
    report.record("closed_views", closed_views);
    report.record("maximum_views", maximum_views);
    report.record("maximum_leaves", maximum_leaves);
    report.record("maximum_splitters", maximum_splitters);
    report.limit("maximum_views", MAXIMUM_VIEWS);
    report.limit("maximum_leaves", MAXIMUM_LEAVES);
    report.limit("maximum_splitters", MAXIMUM_SPLITTERS);
    Ok(report)
}

fn view(
    documents: &mut DocumentRegistry,
    views: &mut DocumentViewRegistry,
    cycle: u32,
    label: &str,
) -> WorkloadResult<DocumentViewId> {
    let document = documents.register_virtual(
        format!("m8.3.panes.{cycle}.{label}"),
        format!("{label}-{cycle}"),
        0,
    )?;
    Ok(views.create_view(document))
}
