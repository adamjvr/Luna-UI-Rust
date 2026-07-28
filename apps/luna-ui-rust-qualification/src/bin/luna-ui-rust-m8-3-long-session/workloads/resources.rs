// SPDX-License-Identifier: MPL-2.0

use super::{WorkloadResult, invariant, require, usize_to_u64};
use crate::report::WorkloadReport;
use luna_integration::ResourceLocator;
use std::path::{Path, PathBuf};

const WORKLOAD: &str = "resource_loading";
const RESOURCE_NAME: &str = "welcome.txt";
const REQUIRED_MARKER: &str = "Luna Reference Consumer";
const MAXIMUM_RESOURCE_BYTES: u64 = 64 * 1_024;

pub(super) fn run(cycles: u32, roots: &[PathBuf]) -> WorkloadResult<WorkloadReport> {
    require(
        WORKLOAD,
        !roots.is_empty(),
        "at least one explicit source or package resource root is required",
    )?;

    let mut loads = 0_u64;
    let mut maximum_resource_bytes = 0_u64;
    let mut reference_content = None::<String>;

    for cycle in 0..cycles {
        for root in roots {
            let mut locator = ResourceLocator::new("org.lunaui.ReferenceConsumer")?;
            locator.push_root(root.clone());
            let content = locator.read_utf8(Path::new(RESOURCE_NAME))?;
            require(
                WORKLOAD,
                content.contains(REQUIRED_MARKER),
                format!(
                    "resource root {} did not contain the expected consumer marker",
                    root.display()
                ),
            )?;

            if let Some(reference) = &reference_content {
                require(
                    WORKLOAD,
                    content.as_str() == reference.as_str(),
                    format!("resource content diverged between explicit roots at cycle {cycle}"),
                )?;
            } else {
                reference_content = Some(content.clone());
            }

            maximum_resource_bytes = maximum_resource_bytes.max(usize_to_u64(content.len()));
            loads = loads.saturating_add(1);
        }
    }

    let root_count = usize_to_u64(roots.len());
    let expected_loads = u64::from(cycles).saturating_mul(root_count);
    require(
        WORKLOAD,
        loads == expected_loads,
        "resource-load count changed",
    )?;
    require(
        WORKLOAD,
        maximum_resource_bytes > 0,
        "resource workload loaded an empty file",
    )?;
    require(
        WORKLOAD,
        maximum_resource_bytes <= MAXIMUM_RESOURCE_BYTES,
        "resource file exceeded its deterministic size limit",
    )?;
    let reference_bytes = reference_content
        .as_ref()
        .map(|content| usize_to_u64(content.len()))
        .ok_or(invariant(
            WORKLOAD,
            "resource workload produced no reference content",
        ))?;

    let mut report = WorkloadReport::new(WORKLOAD);
    report.record("cycles", u64::from(cycles));
    report.record("resource_roots", root_count);
    report.record("loads", loads);
    report.record("reference_resource_bytes", reference_bytes);
    report.record("maximum_resource_bytes", maximum_resource_bytes);
    report.limit("maximum_resource_bytes", MAXIMUM_RESOURCE_BYTES);
    Ok(report)
}
