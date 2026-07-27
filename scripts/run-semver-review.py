#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Run cargo-semver-checks for every stable crate shared with the accepted M7 baseline."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BASELINE = ROOT / "api" / "baselines" / "m7.0.1.toml"
DEFAULT_CURRENT = ROOT / "api" / "public-api.toml"
DEFAULT_BASELINE_COMMIT = "e696df0cedaeda7ac5c0892cf8f709f8325eff8b"
EXPECTED_VERSION = "0.49.0"


def fail(message: str) -> None:
    print(f"[FAIL] {message}", file=sys.stderr)
    raise SystemExit(1)


def parse_scalar(value: str, path: Path, line_number: int) -> object:
    if value in {"true", "false"}:
        return value == "true"
    if value.startswith('"') and value.endswith('"'):
        try:
            parsed = json.loads(value)
        except json.JSONDecodeError as error:
            fail(f"{path}:{line_number}: invalid string: {error}")
        if not isinstance(parsed, str):
            fail(f"{path}:{line_number}: expected a string")
        return parsed
    try:
        return int(value)
    except ValueError:
        fail(f"{path}:{line_number}: unsupported value {value!r}")


def inventory(path: Path) -> dict[str, str]:
    if not path.is_file():
        fail(f"missing API contract: {path}")
    document: dict[str, object] = {}
    section: dict[str, object] = document
    for line_number, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            name = line[1:-1].strip()
            existing = document.get(name)
            if existing is None:
                existing = {}
                document[name] = existing
            if not isinstance(existing, dict):
                fail(f"{path}:{line_number}: invalid table {name}")
            section = existing
            continue
        key, separator, value = line.partition("=")
        if not separator:
            fail(f"{path}:{line_number}: expected key = value")
        section[key.strip()] = parse_scalar(value.strip(), path, line_number)
    crates = document.get("crates")
    if not isinstance(crates, dict):
        fail(f"{path}: missing [crates]")
    result: dict[str, str] = {}
    for package, tier in crates.items():
        if isinstance(package, str) and isinstance(tier, str):
            result[package] = tier
    return result


def command_output(command: list[str]) -> str:
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        fail(completed.stderr.strip() or f"command failed: {' '.join(command)}")
    return completed.stdout.strip()


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, default=DEFAULT_BASELINE)
    parser.add_argument("--current", type=Path, default=DEFAULT_CURRENT)
    parser.add_argument("--baseline-commit", default=DEFAULT_BASELINE_COMMIT)
    parser.add_argument("--output-dir", type=Path, required=True)
    return parser.parse_args()


def main() -> None:
    arguments = parse_arguments()
    output_dir = arguments.output_dir.expanduser().resolve()
    log_dir = output_dir / "semver-logs"
    if log_dir.exists():
        import shutil
        shutil.rmtree(log_dir)
    log_dir.mkdir(parents=True, exist_ok=True)

    tool_version = command_output(["cargo", "+stable", "semver-checks", "--version"])
    if EXPECTED_VERSION not in tool_version:
        fail(
            "cargo-semver-checks version mismatch; "
            f"expected {EXPECTED_VERSION}, observed {tool_version!r}"
        )

    baseline = inventory(arguments.baseline)
    current = inventory(arguments.current)
    packages = sorted(
        package
        for package in baseline.keys() & current.keys()
        if baseline[package] == "stable" and current[package] == "stable"
    )
    environment = os.environ.copy()
    environment["CARGO_TERM_COLOR"] = "never"
    environment["NO_COLOR"] = "1"

    results: list[dict[str, Any]] = []
    for index, package in enumerate(packages, 1):
        print(f"==> [{index}/{len(packages)}] Semver review for {package}")
        command = [
            "cargo",
            "+stable",
            "semver-checks",
            "--package",
            package,
            "--baseline-rev",
            arguments.baseline_commit,
            "--all-features",
        ]
        completed = subprocess.run(
            command,
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
            env=environment,
        )
        log_path = log_dir / f"{package}.log"
        log_path.write_text(
            "$ " + " ".join(command) + "\n\n" + completed.stdout + completed.stderr,
            encoding="utf-8",
        )
        results.append(
            {
                "package": package,
                "exit_code": completed.returncode,
                "status": "passed" if completed.returncode == 0 else "failed",
                "log": f"semver-logs/{package}.log",
            }
        )

    failed = [result for result in results if result["exit_code"] != 0]
    summary = {
        "schema_version": 1,
        "phase": "M8.1b",
        "generated_utc": datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
        "baseline_commit": arguments.baseline_commit,
        "tool": tool_version,
        "stable_crate_count": len(packages),
        "failed_crate_count": len(failed),
        "results": results,
        "passed": not failed,
    }
    (output_dir / "semver-summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    markdown = [
        "# M8.1b Stable-Crate Semver Review",
        "",
        f"- Tool: `{tool_version}`",
        f"- Baseline commit: `{arguments.baseline_commit}`",
        f"- Stable crates reviewed: {len(packages)}",
        f"- Failures: {len(failed)}",
        "",
        "| Crate | Status | Log |",
        "|---|---:|---|",
    ]
    markdown.extend(
        f"| `{result['package']}` | {result['status']} | `{result['log']}` |"
        for result in results
    )
    (output_dir / "SEMVER_REVIEW.md").write_text("\n".join(markdown) + "\n", encoding="utf-8")

    if failed:
        names = [result["package"] for result in failed]
        fail(f"semver review failed for: {names}")
    print(f"[ OK ] Semver review passed for {len(packages)} stable crate(s).")


if __name__ == "__main__":
    main()
