// SPDX-License-Identifier: MPL-2.0

//! Deterministic public-API and release-qualification contracts.
//!
//! Luna treats wall-clock measurements as diagnostics because shared CI hosts are noisy. Release
//! gates use deterministic structural counters: replay operations, retained cache entries, pane and
//! menu geometry, display-list size, GPU atlas bytes, and accessibility-node counts. Downstream
//! projects may add time-based observations without making them blocking.

use luna_core::{CodedError, ErrorCode};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Compatibility commitment attached to one public library crate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ApiStability {
    /// Names and behavior are covered by Luna's compatible-release policy.
    Stable,
    /// Public for integration and testing, but still allowed to change before Luna 1.0.
    Provisional,
    /// Workspace implementation detail that downstream applications should not import directly.
    Internal,
}

impl ApiStability {
    /// Returns the stable manifest spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Provisional => "provisional",
            Self::Internal => "internal",
        }
    }
}

/// Checked-in compatibility contract for one Luna library package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrateContract {
    /// Cargo package name.
    pub package: &'static str,
    /// Current compatibility commitment.
    pub stability: ApiStability,
    /// Short responsibility statement used during API review.
    pub responsibility: &'static str,
}

/// M7 public-library contract inventory.
///
/// The Python API audit verifies that this list and the workspace library packages stay in sync.
pub const CRATE_CONTRACTS: &[CrateContract] = &[
    CrateContract {
        package: "luna-accessibility",
        stability: ApiStability::Stable,
        responsibility: "semantic accessibility snapshots",
    },
    CrateContract {
        package: "luna-accessibility-accesskit",
        stability: ApiStability::Provisional,
        responsibility: "AccessKit translation adapter",
    },
    CrateContract {
        package: "luna-commands",
        stability: ApiStability::Stable,
        responsibility: "typed command and binding contracts",
    },
    CrateContract {
        package: "luna-clipboard",
        stability: ApiStability::Provisional,
        responsibility: "product-neutral text clipboard services",
    },
    CrateContract {
        package: "luna-core",
        stability: ApiStability::Stable,
        responsibility: "identity, geometry, diagnostics, and error codes",
    },
    CrateContract {
        package: "luna-document-services",
        stability: ApiStability::Provisional,
        responsibility: "file and native-dialog boundaries",
    },
    CrateContract {
        package: "luna-documents",
        stability: ApiStability::Stable,
        responsibility: "document identity and lifecycle",
    },
    CrateContract {
        package: "luna-editor",
        stability: ApiStability::Provisional,
        responsibility: "editor mechanics and replay fixtures",
    },
    CrateContract {
        package: "luna-host-core",
        stability: ApiStability::Stable,
        responsibility: "platform-neutral host contracts",
    },
    CrateContract {
        package: "luna-host-wgpu",
        stability: ApiStability::Provisional,
        responsibility: "native wgpu host",
    },
    CrateContract {
        package: "luna-host-winit",
        stability: ApiStability::Provisional,
        responsibility: "native winit CPU host",
    },
    CrateContract {
        package: "luna-input",
        stability: ApiStability::Stable,
        responsibility: "platform-neutral input events",
    },
    CrateContract {
        package: "luna-integration",
        stability: ApiStability::Provisional,
        responsibility: "downstream adapter and resource composition",
    },
    CrateContract {
        package: "luna-layout",
        stability: ApiStability::Stable,
        responsibility: "deterministic layout primitives",
    },
    CrateContract {
        package: "luna-panes",
        stability: ApiStability::Provisional,
        responsibility: "recursive pane and tab topology",
    },
    CrateContract {
        package: "luna-render",
        stability: ApiStability::Stable,
        responsibility: "display lists and CPU oracle",
    },
    CrateContract {
        package: "luna-render-wgpu",
        stability: ApiStability::Provisional,
        responsibility: "wgpu display-list backend",
    },
    CrateContract {
        package: "luna-session",
        stability: ApiStability::Provisional,
        responsibility: "persistent product-neutral sessions",
    },
    CrateContract {
        package: "luna-text",
        stability: ApiStability::Stable,
        responsibility: "UTF-8 editing coordinates and state",
    },
    CrateContract {
        package: "luna-text-cosmic",
        stability: ApiStability::Provisional,
        responsibility: "cosmic-text shaping and rasterization",
    },
    CrateContract {
        package: "luna-theme",
        stability: ApiStability::Stable,
        responsibility: "theme tokens and built-in presets",
    },
    CrateContract {
        package: "luna-ui",
        stability: ApiStability::Provisional,
        responsibility: "reusable widgets and editor surfaces",
    },
    CrateContract {
        package: "luna-workspaces",
        stability: ApiStability::Provisional,
        responsibility: "workspace trees and watchers",
    },
    CrateContract {
        package: "luna-qualification",
        stability: ApiStability::Provisional,
        responsibility: "release qualification contracts",
    },
];

/// Deterministic measurement understood by the M7 release gate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum QualificationMetric {
    /// Operations replayed by an editor behavior fixture.
    EditorReplayOperations,
    /// Logical text-layout misses during a fixed scroll fixture.
    TextLayoutMisses,
    /// Glyph-raster misses during a fixed scroll fixture.
    TextRasterMisses,
    /// Visible leaf panes in a recursive layout fixture.
    PaneLeaves,
    /// Visible splitters in a recursive layout fixture.
    PaneSplitters,
    /// Visible menu rows across one popup path.
    MenuRows,
    /// Nodes in a deterministic workspace snapshot.
    WorkspaceNodes,
    /// Display-list commands consumed by the CPU oracle fixture.
    CpuDisplayCommands,
    /// Ordered draw batches produced by the GPU scene compiler.
    GpuDrawBatches,
    /// Bytes occupied by the frame atlas.
    GpuAtlasBytes,
    /// Retained GPU vertex-buffer capacity.
    GpuVertexCapacityBytes,
    /// Retained GPU index-buffer capacity.
    GpuIndexCapacityBytes,
    /// Nodes in a deterministic semantic accessibility tree.
    AccessibilityNodes,
}

impl QualificationMetric {
    /// Returns the stable report key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EditorReplayOperations => "editor.replay_operations",
            Self::TextLayoutMisses => "text.layout_misses",
            Self::TextRasterMisses => "text.raster_misses",
            Self::PaneLeaves => "panes.leaves",
            Self::PaneSplitters => "panes.splitters",
            Self::MenuRows => "menus.rows",
            Self::WorkspaceNodes => "workspaces.nodes",
            Self::CpuDisplayCommands => "cpu.display_commands",
            Self::GpuDrawBatches => "gpu.draw_batches",
            Self::GpuAtlasBytes => "gpu.atlas_bytes",
            Self::GpuVertexCapacityBytes => "gpu.vertex_capacity_bytes",
            Self::GpuIndexCapacityBytes => "gpu.index_capacity_bytes",
            Self::AccessibilityNodes => "accessibility.nodes",
        }
    }
}

/// Inclusive upper bound for one deterministic release measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QualificationBudget {
    /// Measurement governed by this budget.
    pub metric: QualificationMetric,
    /// Largest accepted value.
    pub maximum: u64,
}

/// One observed deterministic measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QualificationMeasurement {
    /// Measurement identity.
    pub metric: QualificationMetric,
    /// Observed value.
    pub value: u64,
}

/// Named collection of release budgets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualificationProfile {
    name: String,
    budgets: Vec<QualificationBudget>,
}

impl QualificationProfile {
    /// Creates a validated profile.
    ///
    /// # Errors
    ///
    /// Returns [`QualificationError::EmptyProfileName`] for a blank name or
    /// [`QualificationError::DuplicateBudget`] when one metric appears more than once.
    pub fn new(
        name: impl Into<String>,
        budgets: impl IntoIterator<Item = QualificationBudget>,
    ) -> Result<Self, QualificationError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(QualificationError::EmptyProfileName);
        }
        let budgets = budgets.into_iter().collect::<Vec<_>>();
        let mut metrics = BTreeSet::new();
        for budget in &budgets {
            if !metrics.insert(budget.metric) {
                return Err(QualificationError::DuplicateBudget(budget.metric));
            }
        }
        Ok(Self { name, budgets })
    }

    /// Returns the profile name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns budgets in declaration order.
    #[must_use]
    pub fn budgets(&self) -> &[QualificationBudget] {
        &self.budgets
    }

    /// Returns Luna's checked-in M7 structural limits.
    #[must_use]
    pub fn m7_release() -> Self {
        Self {
            name: "m7-release".to_owned(),
            budgets: vec![
                QualificationBudget {
                    metric: QualificationMetric::EditorReplayOperations,
                    maximum: 4_096,
                },
                QualificationBudget {
                    metric: QualificationMetric::TextLayoutMisses,
                    maximum: 8,
                },
                QualificationBudget {
                    metric: QualificationMetric::TextRasterMisses,
                    maximum: 64,
                },
                QualificationBudget {
                    metric: QualificationMetric::PaneLeaves,
                    maximum: 32,
                },
                QualificationBudget {
                    metric: QualificationMetric::PaneSplitters,
                    maximum: 31,
                },
                QualificationBudget {
                    metric: QualificationMetric::MenuRows,
                    maximum: 256,
                },
                QualificationBudget {
                    metric: QualificationMetric::WorkspaceNodes,
                    maximum: 10_000,
                },
                QualificationBudget {
                    metric: QualificationMetric::CpuDisplayCommands,
                    maximum: 100_000,
                },
                QualificationBudget {
                    metric: QualificationMetric::GpuDrawBatches,
                    maximum: 8_192,
                },
                QualificationBudget {
                    metric: QualificationMetric::GpuAtlasBytes,
                    maximum: 67_108_864,
                },
                QualificationBudget {
                    metric: QualificationMetric::GpuVertexCapacityBytes,
                    maximum: 33_554_432,
                },
                QualificationBudget {
                    metric: QualificationMetric::GpuIndexCapacityBytes,
                    maximum: 16_777_216,
                },
                QualificationBudget {
                    metric: QualificationMetric::AccessibilityNodes,
                    maximum: 100_000,
                },
            ],
        }
    }

    /// Evaluates a complete measurement set.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate measurements, missing required metrics, or exceeded budgets.
    pub fn evaluate(
        &self,
        measurements: impl IntoIterator<Item = QualificationMeasurement>,
    ) -> Result<QualificationReport, QualificationError> {
        let mut observed = BTreeMap::new();
        for measurement in measurements {
            if observed
                .insert(measurement.metric, measurement.value)
                .is_some()
            {
                return Err(QualificationError::DuplicateMeasurement(measurement.metric));
            }
        }
        let mut results = Vec::with_capacity(self.budgets.len());
        for budget in &self.budgets {
            let value = observed
                .get(&budget.metric)
                .copied()
                .ok_or(QualificationError::MissingMeasurement(budget.metric))?;
            if value > budget.maximum {
                return Err(QualificationError::BudgetExceeded {
                    metric: budget.metric,
                    observed: value,
                    maximum: budget.maximum,
                });
            }
            results.push(QualificationResult {
                metric: budget.metric,
                observed: value,
                maximum: budget.maximum,
            });
        }
        Ok(QualificationReport {
            profile: self.name.clone(),
            results,
        })
    }
}

/// Passing result for one release budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QualificationResult {
    /// Measurement identity.
    pub metric: QualificationMetric,
    /// Observed value.
    pub observed: u64,
    /// Accepted inclusive limit.
    pub maximum: u64,
}

/// Complete passing release-qualification report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualificationReport {
    profile: String,
    results: Vec<QualificationResult>,
}

impl QualificationReport {
    /// Returns the profile that produced this report.
    #[must_use]
    pub fn profile(&self) -> &str {
        &self.profile
    }

    /// Returns passing results in budget order.
    #[must_use]
    pub fn results(&self) -> &[QualificationResult] {
        &self.results
    }

    /// Encodes a small deterministic JSON object without introducing a serialization dependency.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut output = format!(
            "{{\"profile\":\"{}\",\"passed\":true,\"results\":[",
            escape_json(&self.profile)
        );
        for (index, result) in self.results.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            output.push_str(&format!(
                "{{\"metric\":\"{}\",\"observed\":{},\"maximum\":{}}}",
                result.metric.as_str(),
                result.observed,
                result.maximum
            ));
        }
        output.push_str("]}");
        output
    }
}

/// Failure while constructing or evaluating a qualification profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QualificationError {
    /// A profile name contained no visible characters.
    EmptyProfileName,
    /// One profile declared the same metric twice.
    DuplicateBudget(QualificationMetric),
    /// A measurement set supplied one metric twice.
    DuplicateMeasurement(QualificationMetric),
    /// A required measurement was absent.
    MissingMeasurement(QualificationMetric),
    /// An observed value exceeded its inclusive limit.
    BudgetExceeded {
        /// Failed metric.
        metric: QualificationMetric,
        /// Observed value.
        observed: u64,
        /// Accepted inclusive limit.
        maximum: u64,
    },
}

impl Display for QualificationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProfileName => {
                formatter.write_str("qualification profile name cannot be empty")
            }
            Self::DuplicateBudget(metric) => write!(
                formatter,
                "duplicate qualification budget: {}",
                metric.as_str()
            ),
            Self::DuplicateMeasurement(metric) => write!(
                formatter,
                "duplicate qualification measurement: {}",
                metric.as_str()
            ),
            Self::MissingMeasurement(metric) => write!(
                formatter,
                "missing qualification measurement: {}",
                metric.as_str()
            ),
            Self::BudgetExceeded {
                metric,
                observed,
                maximum,
            } => write!(
                formatter,
                "qualification budget exceeded for {}: observed {observed}, maximum {maximum}",
                metric.as_str()
            ),
        }
    }
}

impl Error for QualificationError {}

impl CodedError for QualificationError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::new(match self {
            Self::EmptyProfileName => "qualification.empty_profile_name",
            Self::DuplicateBudget(_) => "qualification.duplicate_budget",
            Self::DuplicateMeasurement(_) => "qualification.duplicate_measurement",
            Self::MissingMeasurement(_) => "qualification.missing_measurement",
            Self::BudgetExceeded { .. } => "qualification.budget_exceeded",
        })
    }
}

/// Bounded long-session high-watermark sampler.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LongSessionWatermark {
    current_bytes: u64,
    peak_bytes: u64,
    samples: u64,
}

impl LongSessionWatermark {
    /// Records one retained-resource sample.
    pub fn record(&mut self, bytes: u64) {
        self.current_bytes = bytes;
        self.peak_bytes = self.peak_bytes.max(bytes);
        self.samples = self.samples.saturating_add(1);
    }

    /// Returns the latest sample.
    #[must_use]
    pub const fn current_bytes(self) -> u64 {
        self.current_bytes
    }

    /// Returns the largest sample observed.
    #[must_use]
    pub const fn peak_bytes(self) -> u64 {
        self.peak_bytes
    }

    /// Returns the number of recorded samples.
    #[must_use]
    pub const fn samples(self) -> u64 {
        self.samples
    }
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other if other.is_control() => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let code = u32::from(other);
                escaped.push_str("\\u");
                for shift in [12, 8, 4, 0] {
                    let index = ((code >> shift) & 0x0f) as usize;
                    escaped.push(char::from(HEX[index]));
                }
            }
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{
        LongSessionWatermark, QualificationError, QualificationMeasurement, QualificationMetric,
        QualificationProfile,
    };
    use luna_core::CodedError;

    #[test]
    fn release_profile_accepts_values_at_the_limit() -> Result<(), QualificationError> {
        let profile = QualificationProfile::m7_release();
        let measurements = profile
            .budgets()
            .iter()
            .map(|budget| QualificationMeasurement {
                metric: budget.metric,
                value: budget.maximum,
            });
        let report = profile.evaluate(measurements)?;
        assert!(report.to_json().contains("\"passed\":true"));
        Ok(())
    }

    #[test]
    fn exceeded_budget_has_stable_code() -> Result<(), QualificationError> {
        let profile = QualificationProfile::new(
            "small",
            [super::QualificationBudget {
                metric: QualificationMetric::PaneLeaves,
                maximum: 1,
            }],
        )?;
        let error = profile
            .evaluate([QualificationMeasurement {
                metric: QualificationMetric::PaneLeaves,
                value: 2,
            }])
            .err()
            .ok_or(QualificationError::MissingMeasurement(
                QualificationMetric::PaneLeaves,
            ))?;
        assert_eq!(error.error_code().as_str(), "qualification.budget_exceeded");
        Ok(())
    }

    #[test]
    fn json_escapes_control_characters() {
        assert_eq!(super::escape_json("a\u{0008}b"), "a\\u0008b");
    }

    #[test]
    fn watermark_tracks_current_and_peak_without_growth() {
        let mut watermark = LongSessionWatermark::default();
        watermark.record(10);
        watermark.record(4);
        assert_eq!(watermark.current_bytes(), 4);
        assert_eq!(watermark.peak_bytes(), 10);
        assert_eq!(watermark.samples(), 2);
    }
}
