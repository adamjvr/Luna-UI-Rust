// SPDX-License-Identifier: MPL-2.0

//! Stable M8.4 evidence-record serialization and campaign verification.

use crate::status::{CheckState, OperatorStatus};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;

pub(crate) const EVIDENCE_SCHEMA: &str = "luna-m8.4-macos-evidence-v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRecord {
    pub(crate) run_id: String,
    pub(crate) captured_utc: String,
    pub(crate) passed: bool,
    pub(crate) git_commit: String,
    pub(crate) git_dirty: bool,
    pub(crate) architecture: String,
    pub(crate) fields: BTreeMap<String, String>,
    pub(crate) checks: BTreeMap<String, CheckState>,
    pub(crate) notes: String,
}

impl EvidenceRecord {
    pub(crate) fn to_evidence_text(&self) -> String {
        let mut output = String::new();
        push_field(&mut output, "schema", EVIDENCE_SCHEMA);
        push_field(&mut output, "run_id", &self.run_id);
        push_field(&mut output, "captured_utc", &self.captured_utc);
        push_field(&mut output, "passed", bool_text(self.passed));
        push_field(&mut output, "git_commit", &self.git_commit);
        push_field(&mut output, "git_dirty", bool_text(self.git_dirty));
        push_field(&mut output, "architecture", &self.architecture);
        for (key, value) in &self.fields {
            push_field(&mut output, &format!("field.{key}"), value);
        }
        for (key, state) in &self.checks {
            push_field(&mut output, &format!("check.{key}"), state.as_str());
        }
        push_field(&mut output, "notes", &self.notes);
        output
    }

    pub(crate) fn from_evidence_text(text: &str) -> Result<Self, io::Error> {
        let fields = parse_fields(text)?;
        let required = |key: &str| -> Result<String, io::Error> {
            fields.get(key).cloned().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("missing evidence field: {key}"),
                )
            })
        };
        if required("schema")? != EVIDENCE_SCHEMA {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("evidence schema must be {EVIDENCE_SCHEMA}"),
            ));
        }

        let mut metadata = BTreeMap::new();
        let mut checks = BTreeMap::new();
        for (key, value) in &fields {
            if let Some(name) = key.strip_prefix("field.") {
                metadata.insert(name.to_owned(), value.clone());
            }
            if let Some(name) = key.strip_prefix("check.") {
                checks.insert(name.to_owned(), parse_check_state(value)?);
            }
        }

        let status_text =
            status_text_from_record(&checks, fields.get("notes").map_or("", String::as_str));
        let status = OperatorStatus::parse(&status_text)?;
        status.validate_for_capture()?;

        Ok(Self {
            run_id: required("run_id")?,
            captured_utc: required("captured_utc")?,
            passed: parse_bool(&required("passed")?)?,
            git_commit: required("git_commit")?,
            git_dirty: parse_bool(&required("git_dirty")?)?,
            architecture: required("architecture")?,
            fields: metadata,
            checks,
            notes: fields.get("notes").cloned().unwrap_or_default(),
        })
    }

    pub(crate) fn to_json(&self) -> String {
        let mut output = String::new();
        output.push_str("{\n");
        json_field(&mut output, 1, "schema", EVIDENCE_SCHEMA, true);
        json_field(&mut output, 1, "run_id", &self.run_id, true);
        json_field(&mut output, 1, "captured_utc", &self.captured_utc, true);
        json_bool(&mut output, 1, "passed", self.passed, true);
        json_field(&mut output, 1, "git_commit", &self.git_commit, true);
        json_bool(&mut output, 1, "git_dirty", self.git_dirty, true);
        json_field(&mut output, 1, "architecture", &self.architecture, true);
        output.push_str("  \"fields\": {\n");
        json_map(&mut output, &self.fields, 2);
        output.push_str("  },\n  \"checks\": {\n");
        let check_strings = self
            .checks
            .iter()
            .map(|(key, value)| (key.clone(), value.as_str().to_owned()))
            .collect::<BTreeMap<_, _>>();
        json_map(&mut output, &check_strings, 2);
        output.push_str("  },\n");
        json_field(&mut output, 1, "notes", &self.notes, false);
        output.push_str("}\n");
        output
    }
}

pub(crate) fn load_records(directory: &Path) -> Result<Vec<EvidenceRecord>, io::Error> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == OsStr::new("evidence"))
        {
            paths.push(path);
        }
    }
    paths.sort();
    paths
        .into_iter()
        .map(|path| EvidenceRecord::from_evidence_text(&fs::read_to_string(path)?))
        .collect()
}

pub(crate) fn verify_campaign(
    records: &[EvidenceRecord],
    minimum_runs: usize,
    allow_dirty: bool,
) -> Result<CampaignSummary, io::Error> {
    if records.len() < minimum_runs {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "M8.4 campaign requires at least {minimum_runs} evidence runs; found {}",
                records.len()
            ),
        ));
    }
    let mut run_ids = BTreeSet::new();
    let mut commits = BTreeSet::new();
    for record in records {
        if !record.passed {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("evidence run {} is not marked passed", record.run_id),
            ));
        }
        if record.architecture != "arm64" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "evidence run {} used architecture {}; M8.4 requires arm64",
                    record.run_id, record.architecture
                ),
            ));
        }
        if record.git_dirty && !allow_dirty {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "evidence run {} was captured from a dirty tree",
                    record.run_id
                ),
            ));
        }
        if !run_ids.insert(record.run_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate evidence run id: {}", record.run_id),
            ));
        }
        commits.insert(record.git_commit.clone());
    }
    if commits.len() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "all accepted M8.4 evidence runs must reference the same commit",
        ));
    }
    let commit = commits.into_iter().next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "campaign commit was unavailable",
        )
    })?;
    Ok(CampaignSummary {
        schema: "luna-m8.4-macos-campaign-v1",
        passed: true,
        minimum_runs,
        run_ids: run_ids.into_iter().collect(),
        commit,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CampaignSummary {
    schema: &'static str,
    passed: bool,
    minimum_runs: usize,
    run_ids: Vec<String>,
    commit: String,
}

impl CampaignSummary {
    pub(crate) fn to_json(&self) -> String {
        let mut output = String::new();
        output.push_str("{\n");
        json_field(&mut output, 1, "schema", self.schema, true);
        json_bool(&mut output, 1, "passed", self.passed, true);
        output.push_str(&format!("  \"minimum_runs\": {},\n", self.minimum_runs));
        json_field(&mut output, 1, "git_commit", &self.commit, true);
        output.push_str("  \"run_ids\": [");
        for (index, run_id) in self.run_ids.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            output.push('"');
            output.push_str(&escape_json(run_id));
            output.push('"');
        }
        output.push_str("]\n}\n");
        output
    }
}

fn status_text_from_record(checks: &BTreeMap<String, CheckState>, notes: &str) -> String {
    let mut text = format!("schema={}\n", crate::status::STATUS_SCHEMA);
    for (name, state) in checks {
        text.push_str(&format!("check.{name}={}\n", state.as_str()));
    }
    text.push_str(&format!("notes={}\n", escape_value(notes)));
    text
}

fn parse_check_state(value: &str) -> Result<CheckState, io::Error> {
    match value {
        "pass" => Ok(CheckState::Pass),
        "fail" => Ok(CheckState::Fail),
        "pending" => Ok(CheckState::Pending),
        "not_applicable" => Ok(CheckState::NotApplicable),
        "exception" => Ok(CheckState::Exception),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported evidence check state: {other}"),
        )),
    }
}

fn push_field(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push('=');
    output.push_str(&escape_value(value));
    output.push('\n');
}

fn escape_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn unescape_value(value: &str) -> Result<String, io::Error> {
    let mut output = String::new();
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        let escaped = characters.next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "trailing evidence escape")
        })?;
        match escaped {
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            '\\' => output.push('\\'),
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unsupported evidence escape: \\{other}"),
                ));
            }
        }
    }
    Ok(output)
}

fn parse_fields(text: &str) -> Result<BTreeMap<String, String>, io::Error> {
    let mut fields = BTreeMap::new();
    for (line_index, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("evidence line {} has no '=' separator", line_index + 1),
            ));
        };
        if fields
            .insert(key.to_owned(), unescape_value(value)?)
            .is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate evidence field: {key}"),
            ));
        }
    }
    Ok(fields)
}

fn parse_bool(value: &str) -> Result<bool, io::Error> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid boolean value: {other}"),
        )),
    }
}

const fn bool_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn json_map(output: &mut String, values: &BTreeMap<String, String>, indent: usize) {
    for (index, (key, value)) in values.iter().enumerate() {
        let is_last = index + 1 == values.len();
        json_field(output, indent, key, value, !is_last);
    }
}

fn json_field(output: &mut String, indent: usize, key: &str, value: &str, comma: bool) {
    output.push_str(&"  ".repeat(indent));
    output.push('"');
    output.push_str(&escape_json(key));
    output.push_str("\": \"");
    output.push_str(&escape_json(value));
    output.push('"');
    if comma {
        output.push(',');
    }
    output.push('\n');
}

fn json_bool(output: &mut String, indent: usize, key: &str, value: bool, comma: bool) {
    output.push_str(&"  ".repeat(indent));
    output.push('"');
    output.push_str(&escape_json(key));
    output.push_str("\": ");
    output.push_str(bool_text(value));
    if comma {
        output.push(',');
    }
    output.push('\n');
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            control if control.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", u32::from(control)));
            }
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{EvidenceRecord, verify_campaign};
    use crate::status::{CheckState, OperatorStatus};
    use std::collections::BTreeMap;

    fn record(run_id: &str) -> Result<EvidenceRecord, std::io::Error> {
        let status_text = OperatorStatus::template().replace("=pending", "=pass");
        let status = OperatorStatus::parse(&status_text)?;
        Ok(EvidenceRecord {
            run_id: run_id.to_owned(),
            captured_utc: "2026-07-28T12:00:00Z".to_owned(),
            passed: true,
            git_commit: "abc123".to_owned(),
            git_dirty: false,
            architecture: "arm64".to_owned(),
            fields: BTreeMap::from([("hardware_model".to_owned(), "Mac14,3".to_owned())]),
            checks: status.checks().clone(),
            notes: String::new(),
        })
    }

    #[test]
    fn evidence_text_round_trips() -> Result<(), std::io::Error> {
        let original = record("run-1")?;
        let restored = EvidenceRecord::from_evidence_text(&original.to_evidence_text())?;
        assert_eq!(restored, original);
        assert_eq!(restored.checks.get("cpu_editor"), Some(&CheckState::Pass));
        Ok(())
    }

    #[test]
    fn campaign_requires_unique_runs_on_one_clean_commit() -> Result<(), std::io::Error> {
        let first = record("run-1")?;
        let second = record("run-2")?;
        assert!(verify_campaign(&[first.clone(), second], 2, false).is_ok());
        assert!(verify_campaign(&[first.clone(), first], 2, false).is_err());
        Ok(())
    }
}
