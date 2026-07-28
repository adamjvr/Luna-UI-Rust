// SPDX-License-Identifier: MPL-2.0

use std::collections::BTreeMap;

/// One deterministic workload result plus non-blocking timing diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkloadReport {
    name: &'static str,
    diagnostic_micros: u64,
    metrics: BTreeMap<&'static str, u64>,
    limits: BTreeMap<&'static str, u64>,
}

impl WorkloadReport {
    /// Creates an empty report for one named workload.
    #[must_use]
    pub(crate) fn new(name: &'static str) -> Self {
        Self {
            name,
            diagnostic_micros: 0,
            metrics: BTreeMap::new(),
            limits: BTreeMap::new(),
        }
    }

    /// Records one deterministic measurement.
    pub(crate) fn record(&mut self, key: &'static str, value: u64) {
        let _previous = self.metrics.insert(key, value);
    }

    /// Records one inclusive upper bound used by the workload.
    pub(crate) fn limit(&mut self, key: &'static str, value: u64) {
        let _previous = self.limits.insert(key, value);
    }

    /// Records diagnostic-only wall-clock time.
    pub(crate) fn set_diagnostic_micros(&mut self, value: u64) {
        self.diagnostic_micros = value;
    }

    fn write_json(&self, output: &mut String, indent: &str) {
        output.push_str(indent);
        output.push_str("{\n");
        write_json_field(output, indent, "name", self.name, true);
        write_json_number_field(
            output,
            indent,
            "diagnostic_micros",
            self.diagnostic_micros,
            true,
        );
        write_json_map(output, indent, "metrics", &self.metrics, true);
        write_json_map(output, indent, "limits", &self.limits, false);
        output.push_str(indent);
        output.push('}');
    }
}

/// Aggregate M8.3 report emitted by the private qualification binary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LongSessionReport {
    cycles: u32,
    diagnostic_total_micros: u64,
    workloads: Vec<WorkloadReport>,
}

impl LongSessionReport {
    /// Creates one passing aggregate report.
    #[must_use]
    pub(crate) fn new(
        cycles: u32,
        diagnostic_total_micros: u64,
        workloads: Vec<WorkloadReport>,
    ) -> Self {
        Self {
            cycles,
            diagnostic_total_micros,
            workloads,
        }
    }

    /// Serializes the report in stable workload and metric order.
    #[must_use]
    pub(crate) fn to_json(&self) -> String {
        let mut output = String::new();
        output.push_str("{\n");
        write_json_field(&mut output, "", "schema", "luna-m8.3-long-session-v1", true);
        write_json_bool_field(&mut output, "", "passed", true, true);
        write_json_number_field(&mut output, "", "cycles", u64::from(self.cycles), true);
        write_json_number_field(
            &mut output,
            "",
            "diagnostic_total_micros",
            self.diagnostic_total_micros,
            true,
        );
        output.push_str("  \"workloads\": [\n");
        for (index, workload) in self.workloads.iter().enumerate() {
            workload.write_json(&mut output, "    ");
            if index + 1 != self.workloads.len() {
                output.push(',');
            }
            output.push('\n');
        }
        output.push_str("  ]\n");
        output.push_str("}\n");
        output
    }
}

fn write_json_map(
    output: &mut String,
    indent: &str,
    name: &str,
    values: &BTreeMap<&'static str, u64>,
    trailing_comma: bool,
) {
    output.push_str(indent);
    output.push_str("  ");
    push_json_string(output, name);
    output.push_str(": {");
    if values.is_empty() {
        output.push('}');
    } else {
        output.push('\n');
        for (index, (key, value)) in values.iter().enumerate() {
            output.push_str(indent);
            output.push_str("    ");
            push_json_string(output, key);
            output.push_str(": ");
            output.push_str(&value.to_string());
            if index + 1 != values.len() {
                output.push(',');
            }
            output.push('\n');
        }
        output.push_str(indent);
        output.push_str("  }");
    }
    if trailing_comma {
        output.push(',');
    }
    output.push('\n');
}

fn write_json_field(
    output: &mut String,
    indent: &str,
    name: &str,
    value: &str,
    trailing_comma: bool,
) {
    output.push_str(indent);
    output.push_str("  ");
    push_json_string(output, name);
    output.push_str(": ");
    push_json_string(output, value);
    if trailing_comma {
        output.push(',');
    }
    output.push('\n');
}

fn write_json_number_field(
    output: &mut String,
    indent: &str,
    name: &str,
    value: u64,
    trailing_comma: bool,
) {
    output.push_str(indent);
    output.push_str("  ");
    push_json_string(output, name);
    output.push_str(": ");
    output.push_str(&value.to_string());
    if trailing_comma {
        output.push(',');
    }
    output.push('\n');
}

fn write_json_bool_field(
    output: &mut String,
    indent: &str,
    name: &str,
    value: bool,
    trailing_comma: bool,
) {
    output.push_str(indent);
    output.push_str("  ");
    push_json_string(output, name);
    output.push_str(if value { ": true" } else { ": false" });
    if trailing_comma {
        output.push(',');
    }
    output.push('\n');
}

fn push_json_string(output: &mut String, value: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{1f}' => {
                let code = u32::from(character);
                output.push_str("\\u");
                for shift in [12_u32, 8, 4, 0] {
                    let nibble = usize::try_from((code >> shift) & 0x0f).unwrap_or(0);
                    output.push(char::from(HEX[nibble]));
                }
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

#[cfg(test)]
mod tests {
    use super::{LongSessionReport, WorkloadReport};

    #[test]
    fn json_is_stable_and_escapes_control_characters() {
        let mut workload = WorkloadReport::new("text\ncache");
        workload.record("hits", 7);
        workload.limit("hits", 8);
        let report = LongSessionReport::new(4, 12, vec![workload]);
        let json = report.to_json();

        assert!(json.contains("\"schema\": \"luna-m8.3-long-session-v1\""));
        assert!(json.contains("\"name\": \"text\\ncache\""));
        assert!(json.contains("\"hits\": 7"));
    }
}
