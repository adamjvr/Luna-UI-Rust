// SPDX-License-Identifier: MPL-2.0

//! Private M8.4 Apple-Silicon evidence capture and campaign verification executable.
//!
//! The binary adds no public Luna API. It records environment and bundle facts, validates an
//! explicit operator-status file, emits deterministic evidence records, and verifies that a repeated
//! campaign used one clean arm64 commit. It does not attempt to fake graphical, IME, or VoiceOver
//! acceptance in a headless process.

mod record;
mod status;
mod system;

use record::{EvidenceRecord, load_records, verify_campaign};
use status::OperatorStatus;
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const DEFAULT_MINIMUM_RUNS: usize = 3;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("M8.4 macOS evidence failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    match parse_arguments(env::args_os().skip(1))? {
        Action::Template { output } => {
            write_text(&output, &OperatorStatus::template())?;
            println!("M8.4 operator template: {}", output.display());
        }
        Action::Capture(config) => capture(config)?,
        Action::Verify(config) => verify(config)?,
        Action::Help => print_help(),
    }
    Ok(())
}

fn capture(config: CaptureConfig) -> Result<(), Box<dyn Error + Send + Sync>> {
    if env::consts::OS != "macos" {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "M8.4 evidence capture must run on macOS",
        )
        .into());
    }
    validate_run_id(&config.run_id)?;

    let status = OperatorStatus::parse(&fs::read_to_string(&config.operator_status)?)?;
    status.validate_for_capture()?;
    let mut fields = system::capture_environment()?;
    let architecture = fields
        .get("architecture")
        .cloned()
        .ok_or_else(|| io::Error::other("captured architecture was unavailable"))?;
    if architecture != "arm64" {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("M8.4 requires Apple Silicon arm64 hardware; observed {architecture}"),
        )
        .into());
    }

    let git_commit = fields
        .get("git_commit")
        .cloned()
        .ok_or_else(|| io::Error::other("captured git commit was unavailable"))?;
    let git_dirty = system::git_dirty()?;
    if git_dirty && !config.allow_dirty {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "repository is dirty; commit the candidate or pass --allow-dirty for provisional evidence",
        )
        .into());
    }

    let cpu_bundle = system::validate_bundle(&config.cpu_bundle, "cpu")?;
    let metal_bundle = system::validate_bundle(&config.metal_bundle, "wgpu")?;
    validate_bundle_source(&cpu_bundle, &git_commit, config.allow_dirty, "CPU")?;
    validate_bundle_source(&metal_bundle, &git_commit, config.allow_dirty, "Metal")?;
    merge_prefixed(&mut fields, "bundle.cpu", cpu_bundle);
    merge_prefixed(&mut fields, "bundle.metal", metal_bundle);

    let record = EvidenceRecord {
        run_id: config.run_id.clone(),
        captured_utc: system::captured_utc()?,
        passed: true,
        git_commit,
        git_dirty,
        architecture,
        fields,
        checks: status.checks().clone(),
        notes: status.notes().to_owned(),
    };
    fs::create_dir_all(&config.output_dir)?;
    let evidence_path = config
        .output_dir
        .join(format!("{}.evidence", config.run_id));
    let json_path = config.output_dir.join(format!("{}.json", config.run_id));
    write_text(&evidence_path, &record.to_evidence_text())?;
    write_text(&json_path, &record.to_json())?;

    println!("M8.4 evidence record: {}", evidence_path.display());
    println!("M8.4 JSON report:     {}", json_path.display());
    println!("m8_4_evidence=passed");
    Ok(())
}

fn verify(config: VerifyConfig) -> Result<(), Box<dyn Error + Send + Sync>> {
    let records = load_records(&config.evidence_dir)?;
    let summary = verify_campaign(&records, config.minimum_runs, config.allow_dirty)?;
    let summary_path = config.evidence_dir.join("campaign-summary.json");
    write_text(&summary_path, &summary.to_json())?;
    println!("M8.4 campaign summary: {}", summary_path.display());
    println!("m8_4_campaign=passed");
    Ok(())
}

fn validate_bundle_source(
    fields: &BTreeMap<String, String>,
    expected_commit: &str,
    allow_dirty: bool,
    label: &str,
) -> Result<(), io::Error> {
    let commit = fields
        .get("manifest.source_commit")
        .ok_or_else(|| io::Error::other(format!("{label} bundle source commit is missing")))?;
    if commit != expected_commit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{label} bundle was built from commit {commit}; current evidence commit is {expected_commit}"
            ),
        ));
    }
    let dirty = fields
        .get("manifest.source_tree_dirty")
        .map(String::as_str)
        .ok_or_else(|| io::Error::other(format!("{label} bundle dirty-state field is missing")))?;
    if dirty != "false" && !allow_dirty {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} bundle was built from a dirty source tree"),
        ));
    }
    Ok(())
}

fn merge_prefixed(
    destination: &mut BTreeMap<String, String>,
    prefix: &str,
    source: BTreeMap<String, String>,
) {
    for (key, value) in source {
        destination.insert(format!("{prefix}.{key}"), value);
    }
}

fn write_text(path: &Path, text: &str) -> Result<(), io::Error> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

fn validate_run_id(run_id: &str) -> Result<(), io::Error> {
    let valid = !run_id.is_empty()
        && run_id.len() <= 64
        && run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if valid {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--run-id must contain 1-64 ASCII letters, numbers, '.', '_' or '-'",
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Action {
    Template { output: PathBuf },
    Capture(CaptureConfig),
    Verify(VerifyConfig),
    Help,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CaptureConfig {
    output_dir: PathBuf,
    run_id: String,
    operator_status: PathBuf,
    cpu_bundle: PathBuf,
    metal_bundle: PathBuf,
    allow_dirty: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VerifyConfig {
    evidence_dir: PathBuf,
    minimum_runs: usize,
    allow_dirty: bool,
}

fn parse_arguments(mut arguments: impl Iterator<Item = OsString>) -> Result<Action, io::Error> {
    let Some(command) = arguments.next() else {
        return Ok(Action::Help);
    };
    match command.to_string_lossy().as_ref() {
        "template" => parse_template(arguments),
        "capture" => parse_capture(arguments),
        "verify" => parse_verify(arguments),
        "-h" | "--help" => Ok(Action::Help),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown M8.4 command: {other}"),
        )),
    }
}

fn parse_template(mut arguments: impl Iterator<Item = OsString>) -> Result<Action, io::Error> {
    let mut output = None;
    while let Some(argument) = arguments.next() {
        match argument.to_string_lossy().as_ref() {
            "--output" => output = Some(PathBuf::from(required_value(&mut arguments, "--output")?)),
            other => return Err(unknown_argument("template", other)),
        }
    }
    Ok(Action::Template {
        output: output.ok_or_else(|| missing_argument("template", "--output"))?,
    })
}

fn parse_capture(mut arguments: impl Iterator<Item = OsString>) -> Result<Action, io::Error> {
    let mut output_dir = None;
    let mut run_id = None;
    let mut operator_status = None;
    let mut cpu_bundle = None;
    let mut metal_bundle = None;
    let mut allow_dirty = false;

    while let Some(argument) = arguments.next() {
        match argument.to_string_lossy().as_ref() {
            "--output-dir" => {
                output_dir = Some(PathBuf::from(required_value(
                    &mut arguments,
                    "--output-dir",
                )?));
            }
            "--run-id" => {
                run_id = Some(
                    required_value(&mut arguments, "--run-id")?
                        .to_string_lossy()
                        .into_owned(),
                );
            }
            "--operator-status" => {
                operator_status = Some(PathBuf::from(required_value(
                    &mut arguments,
                    "--operator-status",
                )?));
            }
            "--cpu-bundle" => {
                cpu_bundle = Some(PathBuf::from(required_value(
                    &mut arguments,
                    "--cpu-bundle",
                )?));
            }
            "--metal-bundle" => {
                metal_bundle = Some(PathBuf::from(required_value(
                    &mut arguments,
                    "--metal-bundle",
                )?));
            }
            "--allow-dirty" => allow_dirty = true,
            other => return Err(unknown_argument("capture", other)),
        }
    }

    Ok(Action::Capture(CaptureConfig {
        output_dir: output_dir.ok_or_else(|| missing_argument("capture", "--output-dir"))?,
        run_id: run_id.ok_or_else(|| missing_argument("capture", "--run-id"))?,
        operator_status: operator_status
            .ok_or_else(|| missing_argument("capture", "--operator-status"))?,
        cpu_bundle: cpu_bundle.ok_or_else(|| missing_argument("capture", "--cpu-bundle"))?,
        metal_bundle: metal_bundle.ok_or_else(|| missing_argument("capture", "--metal-bundle"))?,
        allow_dirty,
    }))
}

fn parse_verify(mut arguments: impl Iterator<Item = OsString>) -> Result<Action, io::Error> {
    let mut evidence_dir = None;
    let mut minimum_runs = DEFAULT_MINIMUM_RUNS;
    let mut allow_dirty = false;

    while let Some(argument) = arguments.next() {
        match argument.to_string_lossy().as_ref() {
            "--evidence-dir" => {
                evidence_dir = Some(PathBuf::from(required_value(
                    &mut arguments,
                    "--evidence-dir",
                )?));
            }
            "--minimum-runs" => {
                let value = required_value(&mut arguments, "--minimum-runs")?;
                minimum_runs = value.to_string_lossy().parse::<usize>().map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("invalid --minimum-runs value: {error}"),
                    )
                })?;
                if minimum_runs == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--minimum-runs must be at least one",
                    ));
                }
            }
            "--allow-dirty" => allow_dirty = true,
            other => return Err(unknown_argument("verify", other)),
        }
    }

    Ok(Action::Verify(VerifyConfig {
        evidence_dir: evidence_dir.ok_or_else(|| missing_argument("verify", "--evidence-dir"))?,
        minimum_runs,
        allow_dirty,
    }))
}

fn required_value(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<OsString, io::Error> {
    arguments.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{option} requires a value"),
        )
    })
}

fn missing_argument(command: &str, option: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("M8.4 {command} requires {option}"),
    )
}

fn unknown_argument(command: &str, argument: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("unknown M8.4 {command} argument: {argument}"),
    )
}

fn print_help() {
    println!(
        "Luna UI Rust M8.4 macOS evidence\n\
         \n\
         Template:\n\
           cargo run --release -p luna-ui-rust-qualification \\\n             --bin luna-ui-rust-m8-4-macos-evidence -- template --output STATUS\n\
         \n\
         Capture one Apple-Silicon run:\n\
           ... -- capture --output-dir DIR --run-id ID --operator-status STATUS \\\n             --cpu-bundle CPU.app --metal-bundle METAL.app [--allow-dirty]\n\
         \n\
         Verify a repeated campaign:\n\
           ... -- verify --evidence-dir DIR [--minimum-runs 3] [--allow-dirty]"
    );
}

#[cfg(test)]
mod tests {
    use super::{Action, DEFAULT_MINIMUM_RUNS, parse_arguments, validate_bundle_source};
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn template_requires_output() {
        assert!(parse_arguments([OsString::from("template")].into_iter()).is_err());
    }

    #[test]
    fn bundle_source_must_match_the_clean_evidence_commit() -> Result<(), std::io::Error> {
        let clean = BTreeMap::from([
            ("manifest.source_commit".to_owned(), "abc".to_owned()),
            ("manifest.source_tree_dirty".to_owned(), "false".to_owned()),
        ]);
        assert!(validate_bundle_source(&clean, "abc", false, "CPU").is_ok());
        assert!(validate_bundle_source(&clean, "def", false, "CPU").is_err());

        let mut dirty = clean;
        dirty.insert("manifest.source_tree_dirty".to_owned(), "true".to_owned());
        assert!(validate_bundle_source(&dirty, "abc", false, "CPU").is_err());
        assert!(validate_bundle_source(&dirty, "abc", true, "CPU").is_ok());
        Ok(())
    }

    #[test]
    fn verify_defaults_to_three_runs() -> Result<(), std::io::Error> {
        let action = parse_arguments(
            ["verify", "--evidence-dir", "/tmp/evidence"]
                .into_iter()
                .map(OsString::from),
        )?;
        let Action::Verify(config) = action else {
            return Err(std::io::Error::other("expected verify action"));
        };
        assert_eq!(config.evidence_dir, PathBuf::from("/tmp/evidence"));
        assert_eq!(config.minimum_runs, DEFAULT_MINIMUM_RUNS);
        Ok(())
    }
}
