// SPDX-License-Identifier: MPL-2.0

//! Deterministic M8.3 repeated long-session qualification executable.
//!
//! Blocking decisions use counts, capacities, and high-water marks. Wall-clock measurements are
//! emitted only as diagnostics and are intentionally excluded from pass/fail policy.

mod report;
mod workloads;

use report::LongSessionReport;
use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;
use workloads::{WorkloadInputs, duration_micros, run_all};

const DEFAULT_CYCLES: u32 = 64;
const MAXIMUM_CYCLES: u32 = 512;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("M8.3 long-session qualification failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let action = parse_arguments(env::args_os().skip(1))?;
    let CliAction::Run(config) = action else {
        print_help();
        return Ok(());
    };

    let started = Instant::now();
    let workloads = run_all(WorkloadInputs {
        cycles: config.cycles,
        resource_roots: &config.resource_roots,
    })?;
    let report = LongSessionReport::new(config.cycles, duration_micros(started), workloads);
    let json = report.to_json();

    if let Some(path) = config.output {
        write_report(&path, &json)?;
        println!("M8.3 long-session report: {}", path.display());
    } else {
        print!("{json}");
    }
    println!("m8_3_long_session=passed");
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Config {
    cycles: u32,
    output: Option<PathBuf>,
    resource_roots: Vec<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CliAction {
    Run(Config),
    Help,
}

fn parse_arguments(mut arguments: impl Iterator<Item = OsString>) -> Result<CliAction, io::Error> {
    let mut cycles = DEFAULT_CYCLES;
    let mut output = None;
    let mut resource_roots = Vec::new();

    while let Some(argument) = arguments.next() {
        match argument.to_string_lossy().as_ref() {
            "--cycles" => {
                let value = required_value(&mut arguments, "--cycles")?;
                cycles = parse_cycles(&value)?;
            }
            "--output" => {
                let value = required_value(&mut arguments, "--output")?;
                output = Some(PathBuf::from(value));
            }
            "--resource-root" => {
                let value = required_value(&mut arguments, "--resource-root")?;
                resource_roots.push(PathBuf::from(value));
            }
            "-h" | "--help" => return Ok(CliAction::Help),
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown M8.3 argument: {other}"),
                ));
            }
        }
    }

    if resource_roots.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "at least one --resource-root is required",
        ));
    }

    Ok(CliAction::Run(Config {
        cycles,
        output,
        resource_roots,
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

fn parse_cycles(value: &OsString) -> Result<u32, io::Error> {
    let value = value.to_string_lossy();
    let cycles = value.parse::<u32>().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid --cycles value {value:?}: {error}"),
        )
    })?;
    if !(1..=MAXIMUM_CYCLES).contains(&cycles) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("--cycles must be between 1 and {MAXIMUM_CYCLES}"),
        ));
    }
    Ok(cycles)
}

fn write_report(path: &Path, json: &str) -> Result<(), io::Error> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, json)
}

fn print_help() {
    println!(
        "Luna UI Rust M8.3 long-session qualification\n\
         \n\
         Usage:\n\
           cargo run --release -p luna-ui-rust-qualification \\\n             --bin luna-ui-rust-m8-3-long-session -- [options]\n\
         \n\
         Options:\n\
           --cycles N             Complete workload cycles (1..={MAXIMUM_CYCLES}; default {DEFAULT_CYCLES})\n\
           --output PATH          Write deterministic JSON to PATH\n\
           --resource-root PATH   Resource root containing welcome.txt; repeatable\n\
           -h, --help             Show this help"
    );
}

#[cfg(test)]
mod tests {
    use super::{CliAction, DEFAULT_CYCLES, parse_arguments};
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn arguments_require_an_explicit_resource_root() {
        let error = parse_arguments(Vec::<OsString>::new().into_iter());
        assert!(error.is_err());
    }

    #[test]
    fn arguments_accept_repeated_resource_roots() -> Result<(), std::io::Error> {
        let action = parse_arguments(
            [
                "--cycles",
                "8",
                "--resource-root",
                "/source",
                "--resource-root",
                "/package",
            ]
            .into_iter()
            .map(OsString::from),
        )?;
        let CliAction::Run(config) = action else {
            return Err(std::io::Error::other("expected run configuration"));
        };

        assert_eq!(config.cycles, 8);
        assert_eq!(
            config.resource_roots,
            vec![PathBuf::from("/source"), PathBuf::from("/package")]
        );
        assert_ne!(config.cycles, DEFAULT_CYCLES);
        Ok(())
    }
}
