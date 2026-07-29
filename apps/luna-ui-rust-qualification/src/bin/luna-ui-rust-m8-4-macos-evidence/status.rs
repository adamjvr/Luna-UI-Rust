// SPDX-License-Identifier: MPL-2.0

//! Operator-result parsing and policy for the private M8.4 evidence recorder.

use std::collections::BTreeMap;
use std::io;

pub(crate) const STATUS_SCHEMA: &str = "luna-m8.4-operator-status-v1";

const CHECK_POLICIES: &[CheckPolicy] = &[
    CheckPolicy::required("cpu_editor"),
    CheckPolicy::required("metal_editor"),
    CheckPolicy::required("cpu_gallery"),
    CheckPolicy::required("metal_gallery"),
    CheckPolicy::required("cpu_app_bundle"),
    CheckPolicy::required("metal_app_bundle"),
    CheckPolicy::required("retina_geometry"),
    CheckPolicy::allow_not_applicable("external_display_geometry"),
    CheckPolicy::required("dialogs"),
    CheckPolicy::required("application_support_session"),
    CheckPolicy::required("fsevents"),
    CheckPolicy::required("sleep_wake"),
    CheckPolicy::required("memory_pressure"),
    CheckPolicy::required("dead_keys"),
    CheckPolicy::required("emoji"),
    CheckPolicy::required("cjk_ime"),
    CheckPolicy::allow_exception("voiceover"),
    CheckPolicy::required("document_edited_indicator"),
    CheckPolicy::required("dirty_close_discard"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckState {
    Pass,
    Fail,
    Pending,
    NotApplicable,
    Exception,
}

impl CheckState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Pending => "pending",
            Self::NotApplicable => "not_applicable",
            Self::Exception => "exception",
        }
    }

    fn parse(value: &str) -> Result<Self, io::Error> {
        match value {
            "pass" => Ok(Self::Pass),
            "fail" => Ok(Self::Fail),
            "pending" => Ok(Self::Pending),
            "not_applicable" => Ok(Self::NotApplicable),
            "exception" => Ok(Self::Exception),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported operator check state: {other}"),
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OperatorStatus {
    checks: BTreeMap<String, CheckState>,
    notes: String,
}

impl OperatorStatus {
    pub(crate) fn parse(text: &str) -> Result<Self, io::Error> {
        let mut schema = None;
        let mut checks = BTreeMap::new();
        let mut notes = String::new();

        for (line_index, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "operator status line {} has no '=' separator",
                        line_index + 1
                    ),
                ));
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "schema" => schema = Some(value.to_owned()),
                "notes" => notes = unescape_value(value)?,
                _ if key.starts_with("check.") => {
                    let name = key.trim_start_matches("check.");
                    if checks
                        .insert(name.to_owned(), CheckState::parse(value)?)
                        .is_some()
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("duplicate operator check: {name}"),
                        ));
                    }
                }
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("unknown operator status field: {other}"),
                    ));
                }
            }
        }

        if schema.as_deref() != Some(STATUS_SCHEMA) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("operator status schema must be {STATUS_SCHEMA}"),
            ));
        }

        let status = Self { checks, notes };
        status.validate_complete()?;
        Ok(status)
    }

    pub(crate) fn template() -> String {
        let mut output = format!("schema={STATUS_SCHEMA}\n");
        for policy in CHECK_POLICIES {
            output.push_str("check.");
            output.push_str(policy.name);
            output.push_str("=pending\n");
        }
        output.push_str("notes=\n");
        output
    }

    pub(crate) fn checks(&self) -> &BTreeMap<String, CheckState> {
        &self.checks
    }

    pub(crate) fn notes(&self) -> &str {
        &self.notes
    }

    pub(crate) fn validate_for_capture(&self) -> Result<(), io::Error> {
        self.validate_complete()?;
        for policy in CHECK_POLICIES {
            let state = self.checks.get(policy.name).copied().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("missing operator check: {}", policy.name),
                )
            })?;
            let accepted = state == CheckState::Pass
                || (state == CheckState::NotApplicable && policy.not_applicable)
                || (state == CheckState::Exception && policy.exception);
            if !accepted {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "operator check {} is {}; expected an accepted result",
                        policy.name,
                        state.as_str()
                    ),
                ));
            }
        }
        if self
            .checks
            .values()
            .any(|state| matches!(state, CheckState::NotApplicable | CheckState::Exception))
            && self.notes.trim().is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not_applicable or exception results require explanatory notes",
            ));
        }
        Ok(())
    }

    fn validate_complete(&self) -> Result<(), io::Error> {
        for policy in CHECK_POLICIES {
            if !self.checks.contains_key(policy.name) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("missing operator check: {}", policy.name),
                ));
            }
        }
        for name in self.checks.keys() {
            if !CHECK_POLICIES.iter().any(|policy| policy.name == name) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unknown operator check: {name}"),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct CheckPolicy {
    name: &'static str,
    not_applicable: bool,
    exception: bool,
}

impl CheckPolicy {
    const fn required(name: &'static str) -> Self {
        Self {
            name,
            not_applicable: false,
            exception: false,
        }
    }

    const fn allow_not_applicable(name: &'static str) -> Self {
        Self {
            name,
            not_applicable: true,
            exception: false,
        }
    }

    const fn allow_exception(name: &'static str) -> Self {
        Self {
            name,
            not_applicable: false,
            exception: true,
        }
    }
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
            io::Error::new(io::ErrorKind::InvalidData, "trailing escape in notes")
        })?;
        match escaped {
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            '\\' => output.push('\\'),
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unsupported notes escape: \\{other}"),
                ));
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{CheckState, OperatorStatus};

    #[test]
    fn template_round_trips_but_remains_pending() -> Result<(), std::io::Error> {
        let status = OperatorStatus::parse(&OperatorStatus::template())?;
        assert!(
            status
                .checks()
                .values()
                .all(|state| *state == CheckState::Pending)
        );
        assert!(status.validate_for_capture().is_err());
        Ok(())
    }

    #[test]
    fn exception_requires_notes() -> Result<(), std::io::Error> {
        let mut template = OperatorStatus::template().replace("=pending", "=pass");
        template = template.replace("check.voiceover=pass", "check.voiceover=exception");
        assert!(
            OperatorStatus::parse(&template)?
                .validate_for_capture()
                .is_err()
        );
        template = template.replace(
            "notes=",
            "notes=VoiceOver issue recorded as M8.4 advisory exception",
        );
        assert!(
            OperatorStatus::parse(&template)?
                .validate_for_capture()
                .is_ok()
        );
        Ok(())
    }
}
