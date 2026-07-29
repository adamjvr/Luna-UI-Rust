// SPDX-License-Identifier: MPL-2.0

mod documents;
mod panes;
mod render;
mod resources;
mod session;
mod text;
mod workspace;

use crate::report::WorkloadReport;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::time::Instant;

/// Error type shared by private M8.3 workload modules.
pub(crate) type WorkloadResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

/// Immutable inputs shared by every M8.3 workload.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkloadInputs<'a> {
    /// Number of complete deterministic workload cycles.
    pub(crate) cycles: u32,
    /// Explicit source-tree and extracted-package resource roots.
    pub(crate) resource_roots: &'a [PathBuf],
}

/// Runs every M8.3 workload in stable report order.
pub(crate) fn run_all(inputs: WorkloadInputs<'_>) -> WorkloadResult<Vec<WorkloadReport>> {
    Ok(vec![
        timed(|| documents::run(inputs.cycles))?,
        timed(|| text::run(inputs.cycles))?,
        timed(|| panes::run(inputs.cycles))?,
        timed(|| workspace::run(inputs.cycles))?,
        timed(|| session::run(inputs.cycles))?,
        timed(|| render::run(inputs.cycles))?,
        timed(|| resources::run(inputs.cycles, inputs.resource_roots))?,
    ])
}

fn timed<F>(workload: F) -> WorkloadResult<WorkloadReport>
where
    F: FnOnce() -> WorkloadResult<WorkloadReport>,
{
    let started = Instant::now();
    let mut report = workload()?;
    report.set_diagnostic_micros(duration_micros(started));
    Ok(report)
}

/// Converts an allocation or collection length to a saturating report value.
#[must_use]
pub(crate) fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// Converts elapsed wall-clock time to a diagnostic-only report value.
#[must_use]
pub(crate) fn duration_micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

/// Requires one deterministic workload invariant.
pub(crate) fn require(
    workload: &'static str,
    condition: bool,
    message: impl Into<String>,
) -> Result<(), InvariantFailure> {
    if condition {
        Ok(())
    } else {
        Err(InvariantFailure {
            workload,
            message: message.into(),
        })
    }
}

/// Creates a deterministic invariant error for use with `Option::ok_or_else`.
#[must_use]
pub(crate) fn invariant(workload: &'static str, message: impl Into<String>) -> InvariantFailure {
    InvariantFailure {
        workload,
        message: message.into(),
    }
}

/// One blocking structural invariant failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InvariantFailure {
    workload: &'static str,
    message: String,
}

impl Display for InvariantFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} workload invariant failed: {}",
            self.workload, self.message
        )
    }
}

impl Error for InvariantFailure {}

#[cfg(test)]
mod tests {
    use super::{WorkloadInputs, run_all};
    use std::error::Error;
    use std::fs;

    #[test]
    #[ignore = "release-mode M8.3 gate is authoritative; debug cosmic-text shaping is intentionally expensive"]
    fn one_cycle_exercises_every_workload() -> Result<(), Box<dyn Error + Send + Sync>> {
        let root =
            std::env::temp_dir().join(format!("luna-m8-3-resource-test-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::create_dir_all(&root)?;
        fs::write(
            root.join("welcome.txt"),
            "Luna Reference Consumer — deterministic M8.3 test\n",
        )?;

        let reports = run_all(WorkloadInputs {
            cycles: 1,
            resource_roots: std::slice::from_ref(&root),
        })?;
        fs::remove_dir_all(&root)?;

        assert_eq!(reports.len(), 7);
        Ok(())
    }
}
