#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Validate Luna's checked-in public-crate and stable-error contracts."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "api" / "public-api.toml"


def fail(message: str) -> None:
    print(f"[FAIL] {message}", file=sys.stderr)
    raise SystemExit(1)


def cargo_metadata() -> dict[str, object]:
    command = ["cargo", "metadata", "--format-version", "1", "--no-deps"]
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError:
        fail("cargo is required for the public API audit")
    except subprocess.CalledProcessError as error:
        sys.stderr.write(error.stdout)
        sys.stderr.write(error.stderr)
        fail("cargo metadata failed")
    return json.loads(completed.stdout)


def workspace_packages(metadata: dict[str, object]) -> dict[str, dict[str, object]]:
    packages = metadata.get("packages")
    if not isinstance(packages, list):
        fail("cargo metadata did not return packages")
    result: dict[str, dict[str, object]] = {}
    for package in packages:
        if not isinstance(package, dict):
            continue
        name = package.get("name")
        if isinstance(name, str):
            result[name] = package
    return result


def library_packages(packages: dict[str, dict[str, object]]) -> dict[str, dict[str, object]]:
    return {
        name: package
        for name, package in packages.items()
        if any(
            isinstance(target, dict)
            and any(kind in {"lib", "rlib"} for kind in target.get("kind", []))
            for target in package.get("targets", [])
        )
    }


def validate_package_metadata(packages: dict[str, dict[str, object]]) -> None:
    for name, package in sorted(packages.items()):
        if not package.get("description"):
            fail(f"{name} is missing a package description")
        if package.get("license") != "MPL-2.0":
            fail(f"{name} does not inherit MPL-2.0")
        if package.get("rust_version") != "1.97.1":
            fail(f"{name} does not inherit the project MSRV")
        if package.get("authors") != ["Adam Vadala-Roth"]:
            fail(f"{name} does not inherit the project author metadata")
        if package.get("repository") != "https://github.com/adamjvr/Luna-UI-Rust":
            fail(f"{name} does not inherit the repository URL")
        if not package.get("readme"):
            fail(f"{name} does not inherit the workspace README")
        if package.get("publish") != []:
            fail(f"{name} must remain non-publishable during pre-release qualification")


def parse_scalar(value: str, line_number: int) -> object:
    if value in {"true", "false"}:
        return value == "true"
    if value.startswith('"') and value.endswith('"'):
        try:
            parsed = json.loads(value)
        except json.JSONDecodeError as error:
            fail(f"api/public-api.toml:{line_number}: invalid string: {error}")
        if not isinstance(parsed, str):
            fail(f"api/public-api.toml:{line_number}: expected a string")
        return parsed
    try:
        return int(value)
    except ValueError:
        fail(f"api/public-api.toml:{line_number}: unsupported value {value!r}")


def load_contract_manifest() -> dict[str, object]:
    contract: dict[str, object] = {}
    section: dict[str, object] = contract
    for line_number, raw_line in enumerate(MANIFEST.read_text(encoding="utf-8").splitlines(), 1):
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            section_name = line[1:-1].strip()
            if not section_name or "." in section_name:
                fail(f"api/public-api.toml:{line_number}: unsupported table {line!r}")
            existing = contract.get(section_name)
            if existing is None:
                existing = {}
                contract[section_name] = existing
            if not isinstance(existing, dict):
                fail(f"api/public-api.toml:{line_number}: duplicate scalar/table name")
            section = existing
            continue
        key, separator, value = line.partition("=")
        key = key.strip()
        value = value.strip()
        if not separator or not key or not value:
            fail(f"api/public-api.toml:{line_number}: expected key = value")
        if key in section:
            fail(f"api/public-api.toml:{line_number}: duplicate key {key!r}")
        section[key] = parse_scalar(value, line_number)
    return contract


def validate_contract_inventory(
    packages: dict[str, dict[str, object]], contract: dict[str, object]
) -> None:
    configured = contract.get("crates")
    if not isinstance(configured, dict):
        fail("api/public-api.toml is missing [crates]")
    configured_names = set(configured)
    package_names = set(packages)
    if configured_names != package_names:
        missing = sorted(package_names - configured_names)
        stale = sorted(configured_names - package_names)
        fail(f"public crate inventory mismatch; missing={missing}, stale={stale}")
    invalid_tiers = sorted(
        name for name, tier in configured.items() if tier not in {"stable", "provisional", "internal"}
    )
    if invalid_tiers:
        fail(f"invalid API stability tiers for: {invalid_tiers}")

    source = (ROOT / "crates" / "luna-qualification" / "src" / "lib.rs").read_text()
    source_names = set(re.findall(r'package: "([^"]+)"', source))
    if source_names != configured_names:
        fail("luna-qualification CRATE_CONTRACTS differs from api/public-api.toml")


def validate_error_codes() -> None:
    error_impl = re.compile(r"impl\s+(?:std::error::)?Error\s+for\s+(\w+)")
    for source_path in sorted((ROOT / "crates").rglob("*.rs")):
        source = source_path.read_text()
        for error_type in error_impl.findall(source):
            if not re.search(rf"impl\s+CodedError\s+for\s+{re.escape(error_type)}\b", source):
                fail(f"{source_path.relative_to(ROOT)}: {error_type} lacks CodedError")


def main() -> None:
    contract = load_contract_manifest()
    arguments = sys.argv[1:]
    if arguments == ["--list-crates"]:
        configured = contract.get("crates")
        if not isinstance(configured, dict):
            fail("api/public-api.toml is missing [crates]")
        for name in sorted(configured):
            print(name)
        return
    if arguments:
        fail(f"unsupported arguments: {arguments}")

    packages = workspace_packages(cargo_metadata())
    libraries = library_packages(packages)
    validate_package_metadata(packages)
    validate_contract_inventory(libraries, contract)
    validate_error_codes()
    print(
        f"[ OK ] Package metadata validated for {len(packages)} workspace packages; "
        f"public API contracts validated for {len(libraries)} library crates."
    )


if __name__ == "__main__":
    main()
