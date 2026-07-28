// SPDX-License-Identifier: MPL-2.0

use super::{WorkloadResult, invariant, require, usize_to_u64};
use crate::report::WorkloadReport;
use luna_workspaces::{
    MemoryWorkspaceService, MemoryWorkspaceWatchService, WorkspaceCollisionPolicy, WorkspaceModel,
    WorkspaceMutationService, WorkspaceRefreshScope, WorkspaceScanOptions, WorkspaceService,
    WorkspaceWatchEvent, WorkspaceWatchKind, WorkspaceWatchService, coalesce_watch_events,
    refresh_scope_for_events,
};
use std::path::{Path, PathBuf};

const WORKLOAD: &str = "workspace_watcher_bursts";
const BASELINE_NODES: u64 = 3;
const MAXIMUM_NODES: u64 = 4;
const RAW_EVENTS_PER_CYCLE: u64 = 6;
const COALESCED_EVENTS_PER_CYCLE: u64 = 3;

pub(super) fn run(cycles: u32) -> WorkloadResult<WorkloadReport> {
    let root = PathBuf::from("/luna-m8-3/workspace");
    let source = root.join("src");
    let service = MemoryWorkspaceService::new(root.clone())?;
    service.insert_directory(Path::new("src"))?;
    service.insert_directory(Path::new("docs"))?;
    let options = WorkspaceScanOptions::default();
    let mut model = WorkspaceModel::new(service.scan(&root, options)?);
    let mut watcher = MemoryWorkspaceWatchService::default();
    watcher.watch(&root)?;

    let mut raw_events = 0_u64;
    let mut coalesced_events = 0_u64;
    let mut full_refreshes = 0_u64;
    let mut subtree_refreshes = 0_u64;
    let mut maximum_nodes = usize_to_u64(model.snapshot().nodes().len());

    for cycle in 0..cycles {
        let initial_name = format!("cycle-{cycle:04}.rs");
        let renamed_name = format!("cycle-{cycle:04}-renamed.rs");
        let created = service.create_file(
            &source,
            &initial_name,
            WorkspaceCollisionPolicy::FailIfExists,
        )?;
        let created_path = created.path().to_path_buf();
        let created_burst = [
            event(&created_path, WorkspaceWatchKind::Created),
            event(&created_path, WorkspaceWatchKind::Modified),
            event(&created_path, WorkspaceWatchKind::Modified),
        ];
        process_burst(
            &service,
            &mut watcher,
            &mut model,
            options,
            &root,
            &created_burst,
            &mut raw_events,
            &mut coalesced_events,
            &mut full_refreshes,
            &mut subtree_refreshes,
        )?;
        maximum_nodes = maximum_nodes.max(usize_to_u64(model.snapshot().nodes().len()));

        let renamed = service.rename(
            &created_path,
            &renamed_name,
            WorkspaceCollisionPolicy::FailIfExists,
        )?;
        let renamed_path = renamed.path().to_path_buf();
        let renamed_burst = [
            event(&renamed_path, WorkspaceWatchKind::Renamed),
            event(&renamed_path, WorkspaceWatchKind::Modified),
        ];
        process_burst(
            &service,
            &mut watcher,
            &mut model,
            options,
            &root,
            &renamed_burst,
            &mut raw_events,
            &mut coalesced_events,
            &mut full_refreshes,
            &mut subtree_refreshes,
        )?;
        maximum_nodes = maximum_nodes.max(usize_to_u64(model.snapshot().nodes().len()));

        service.delete(&renamed_path)?;
        let removed_burst = [event(&renamed_path, WorkspaceWatchKind::Removed)];
        process_burst(
            &service,
            &mut watcher,
            &mut model,
            options,
            &root,
            &removed_burst,
            &mut raw_events,
            &mut coalesced_events,
            &mut full_refreshes,
            &mut subtree_refreshes,
        )?;
        maximum_nodes = maximum_nodes.max(usize_to_u64(model.snapshot().nodes().len()));
    }

    let final_nodes = usize_to_u64(model.snapshot().nodes().len());
    require(
        WORKLOAD,
        final_nodes == BASELINE_NODES,
        format!("workspace did not return to {BASELINE_NODES} baseline nodes"),
    )?;
    require(
        WORKLOAD,
        maximum_nodes <= MAXIMUM_NODES,
        "workspace node high-water mark exceeded the fixed fixture",
    )?;
    require(
        WORKLOAD,
        raw_events == u64::from(cycles).saturating_mul(RAW_EVENTS_PER_CYCLE),
        "raw watcher event count changed",
    )?;
    require(
        WORKLOAD,
        coalesced_events == u64::from(cycles).saturating_mul(COALESCED_EVENTS_PER_CYCLE),
        "watcher coalescing no longer produces one event per mutation stage",
    )?;
    require(
        WORKLOAD,
        full_refreshes == 0,
        "path-local mutation bursts unexpectedly required full refreshes",
    )?;
    require(
        WORKLOAD,
        subtree_refreshes == u64::from(cycles).saturating_mul(3),
        "path-local mutation bursts no longer select subtree refreshes",
    )?;

    let mut report = WorkloadReport::new(WORKLOAD);
    report.record("cycles", u64::from(cycles));
    report.record("mutations", u64::from(cycles).saturating_mul(3));
    report.record("refreshes", u64::from(cycles).saturating_mul(3));
    report.record("raw_events", raw_events);
    report.record("raw_events_per_cycle", RAW_EVENTS_PER_CYCLE);
    report.record("coalesced_events", coalesced_events);
    report.record("coalesced_events_per_cycle", COALESCED_EVENTS_PER_CYCLE);
    report.record("full_refreshes", full_refreshes);
    report.record("subtree_refreshes", subtree_refreshes);
    report.record("maximum_nodes", maximum_nodes);
    report.record("final_nodes", final_nodes);
    report.limit("maximum_nodes", MAXIMUM_NODES);
    report.limit("raw_events_per_cycle", RAW_EVENTS_PER_CYCLE);
    report.limit("coalesced_events_per_cycle", COALESCED_EVENTS_PER_CYCLE);
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn process_burst(
    service: &MemoryWorkspaceService,
    watcher: &mut MemoryWorkspaceWatchService,
    model: &mut WorkspaceModel,
    options: WorkspaceScanOptions,
    root: &Path,
    burst: &[WorkspaceWatchEvent],
    raw_total: &mut u64,
    coalesced_total: &mut u64,
    full_refreshes: &mut u64,
    subtree_refreshes: &mut u64,
) -> WorkloadResult<()> {
    for event in burst {
        watcher.push(event.clone());
    }
    let raw = watcher.drain_events()?;
    *raw_total = (*raw_total).saturating_add(usize_to_u64(raw.len()));
    let coalesced = coalesce_watch_events(root, &raw)?;
    *coalesced_total = (*coalesced_total).saturating_add(usize_to_u64(coalesced.len()));
    match refresh_scope_for_events(root, &coalesced)? {
        Some(WorkspaceRefreshScope::Full) => {
            *full_refreshes = (*full_refreshes).saturating_add(1);
        }
        Some(WorkspaceRefreshScope::Subtree(_)) => {
            *subtree_refreshes = (*subtree_refreshes).saturating_add(1);
        }
        None => {
            return Err(Box::new(invariant(
                WORKLOAD,
                "non-empty watcher burst produced no refresh scope",
            )));
        }
    }
    require(
        WORKLOAD,
        model.refresh_from_events(service, &raw, options)?,
        "workspace model suppressed a real create, rename, or remove mutation",
    )?;
    Ok(())
}

fn event(path: &Path, kind: WorkspaceWatchKind) -> WorkspaceWatchEvent {
    WorkspaceWatchEvent {
        path: path.to_path_buf(),
        kind,
    }
}
