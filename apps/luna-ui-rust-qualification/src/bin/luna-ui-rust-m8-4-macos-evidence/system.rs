// SPDX-License-Identifier: MPL-2.0

//! macOS environment capture and application-bundle validation.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, Output};

pub(crate) fn capture_environment() -> Result<BTreeMap<String, String>, io::Error> {
    let mut fields = BTreeMap::new();
    insert_command(
        &mut fields,
        "macos_product_version",
        "sw_vers",
        &["-productVersion"],
    )?;
    insert_command(
        &mut fields,
        "macos_build_version",
        "sw_vers",
        &["-buildVersion"],
    )?;
    insert_command(&mut fields, "architecture", "uname", &["-m"])?;
    insert_command(&mut fields, "kernel", "uname", &["-srv"])?;
    insert_command(&mut fields, "hardware_model", "sysctl", &["-n", "hw.model"])?;
    insert_command(
        &mut fields,
        "hardware_memory_bytes",
        "sysctl",
        &["-n", "hw.memsize"],
    )?;
    insert_command(
        &mut fields,
        "hardware_logical_cpu_count",
        "sysctl",
        &["-n", "hw.logicalcpu"],
    )?;
    insert_optional_command(
        &mut fields,
        "hardware_chip",
        "sysctl",
        &["-n", "machdep.cpu.brand_string"],
    );
    insert_command(&mut fields, "rustc", "rustc", &["--version"])?;
    insert_command(&mut fields, "cargo", "cargo", &["--version"])?;
    insert_command(&mut fields, "git_commit", "git", &["rev-parse", "HEAD"])?;
    insert_command(
        &mut fields,
        "display_profile",
        "system_profiler",
        &["SPDisplaysDataType", "-detailLevel", "mini"],
    )?;
    Ok(fields)
}

pub(crate) fn captured_utc() -> Result<String, io::Error> {
    command_text("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"])
}

pub(crate) fn git_dirty() -> Result<bool, io::Error> {
    Ok(!command_text(
        "git",
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?
    .trim()
    .is_empty())
}

pub(crate) fn validate_bundle(
    bundle: &Path,
    expected_backend: &str,
) -> Result<BTreeMap<String, String>, io::Error> {
    let info_plist = bundle.join("Contents/Info.plist");
    let launcher = bundle.join("Contents/MacOS/LunaUIRustEditorDemo");
    let executable = bundle.join("Contents/Resources/bin/luna-ui-rust-editor-demo");
    let manifest = bundle.join("Contents/Resources/M8_4_BUNDLE_MANIFEST.txt");

    require_path(bundle, true, "application bundle")?;
    require_path(&info_plist, false, "Info.plist")?;
    require_path(&launcher, false, "bundle launcher")?;
    require_executable(&launcher, "bundle launcher")?;
    require_path(&executable, false, "Rust executable")?;
    require_executable(&executable, "Rust executable")?;
    require_path(&manifest, false, "bundle manifest")?;

    command_path("plutil", &["-lint"], &info_plist)?;
    command_bundle(
        "codesign",
        &["--verify", "--deep", "--strict", "--verbose=2"],
        bundle,
    )?;
    let architectures = command_path_text("lipo", &["-archs"], &executable)?;
    if !architectures
        .split_whitespace()
        .any(|value| value == "arm64")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "bundle executable {} does not contain arm64: {architectures}",
                executable.display()
            ),
        ));
    }

    let manifest_text = fs::read_to_string(&manifest)?;
    let manifest_fields = parse_manifest(&manifest_text)?;
    if manifest_fields.get("backend").map(String::as_str) != Some(expected_backend) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "bundle {} reports backend {:?}; expected {expected_backend}",
                bundle.display(),
                manifest_fields.get("backend")
            ),
        ));
    }

    let mut fields = BTreeMap::new();
    fields.insert("path".to_owned(), bundle.display().to_string());
    fields.insert("backend".to_owned(), expected_backend.to_owned());
    fields.insert("architectures".to_owned(), architectures);
    fields.insert(
        "executable_bytes".to_owned(),
        fs::metadata(&executable)?.len().to_string(),
    );
    fields.insert(
        "codesign".to_owned(),
        command_bundle_text("codesign", &["-dv", "--verbose=4"], bundle)?,
    );
    fields.insert(
        "file".to_owned(),
        command_path_text("file", &[], &executable)?,
    );
    for (key, value) in manifest_fields {
        fields.insert(format!("manifest.{key}"), value);
    }
    Ok(fields)
}

pub(crate) fn command_text(program: &str, arguments: &[&str]) -> Result<String, io::Error> {
    let output = Command::new(program).args(arguments).output()?;
    successful_text(program, output)
}

fn insert_command(
    fields: &mut BTreeMap<String, String>,
    key: &str,
    program: &str,
    arguments: &[&str],
) -> Result<(), io::Error> {
    fields.insert(key.to_owned(), command_text(program, arguments)?);
    Ok(())
}

fn insert_optional_command(
    fields: &mut BTreeMap<String, String>,
    key: &str,
    program: &str,
    arguments: &[&str],
) {
    if let Ok(value) = command_text(program, arguments) {
        fields.insert(key.to_owned(), value);
    }
}

fn require_path(path: &Path, directory: bool, label: &str) -> Result<(), io::Error> {
    let valid = if directory {
        path.is_dir()
    } else {
        path.is_file()
    };
    if valid {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{label} is missing: {}", path.display()),
        ))
    }
}

#[cfg(unix)]
fn require_executable(path: &Path, label: &str) -> Result<(), io::Error> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path)?.permissions().mode();
    if mode & 0o111 != 0 {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{label} is not executable: {}", path.display()),
        ))
    }
}

#[cfg(not(unix))]
fn require_executable(_path: &Path, _label: &str) -> Result<(), io::Error> {
    Ok(())
}

fn command_path(program: &str, arguments: &[&str], path: &Path) -> Result<(), io::Error> {
    let output = Command::new(program).args(arguments).arg(path).output()?;
    successful_text(program, output).map(|_| ())
}

fn command_path_text(program: &str, arguments: &[&str], path: &Path) -> Result<String, io::Error> {
    let output = Command::new(program).args(arguments).arg(path).output()?;
    successful_text(program, output)
}

fn command_bundle(program: &str, arguments: &[&str], bundle: &Path) -> Result<(), io::Error> {
    command_path(program, arguments, bundle)
}

fn command_bundle_text(
    program: &str,
    arguments: &[&str],
    bundle: &Path,
) -> Result<String, io::Error> {
    let output = Command::new(program).args(arguments).arg(bundle).output()?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = stdout.trim();
        let stderr = stderr.trim();
        let combined = match (stdout.is_empty(), stderr.is_empty()) {
            (false, false) => format!("{stdout}\n{stderr}"),
            (false, true) => stdout.to_owned(),
            (true, false) => stderr.to_owned(),
            (true, true) => String::new(),
        };
        return Ok(combined);
    }
    Err(command_error(program, &output))
}

fn successful_text(program: &str, output: Output) -> Result<String, io::Error> {
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
    }
    Err(command_error(program, &output))
}

fn command_error(program: &str, output: &Output) -> io::Error {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    io::Error::other(format!("{program} exited with {}: {detail}", output.status))
}

fn parse_manifest(text: &str) -> Result<BTreeMap<String, String>, io::Error> {
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bundle manifest line has no '=' separator: {line}"),
            ));
        };
        if fields.insert(key.to_owned(), value.to_owned()).is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate bundle manifest field: {key}"),
            ));
        }
    }
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::parse_manifest;

    #[test]
    fn bundle_manifest_parser_rejects_duplicates() -> Result<(), std::io::Error> {
        let parsed = parse_manifest("backend=cpu\ncommit=abc\n")?;
        assert_eq!(parsed.get("backend").map(String::as_str), Some("cpu"));
        assert!(parse_manifest("backend=cpu\nbackend=wgpu\n").is_err());
        Ok(())
    }
}
