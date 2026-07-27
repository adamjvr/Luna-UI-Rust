#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Compare Luna's current crate-contract inventory with the accepted M7 baseline."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BASELINE = ROOT / "api" / "baselines" / "m7.0.1.toml"
DEFAULT_CURRENT = ROOT / "api" / "public-api.toml"
DEFAULT_CLASSIFICATION = ROOT / "api" / "compatibility" / "m8.1.json"
VALID_TIERS = {"stable", "provisional", "internal"}
VALID_CLASSIFICATIONS = {"compatible", "intentionally-breaking", "accidental"}


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


def load_simple_toml(path: Path) -> dict[str, object]:
    if not path.is_file():
        fail(f"missing contract file: {path}")
    document: dict[str, object] = {}
    section: dict[str, object] = document
    for line_number, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            section_name = line[1:-1].strip()
            if not section_name or "." in section_name:
                fail(f"{path}:{line_number}: unsupported table {line!r}")
            existing = document.get(section_name)
            if existing is None:
                existing = {}
                document[section_name] = existing
            if not isinstance(existing, dict):
                fail(f"{path}:{line_number}: duplicate scalar/table name")
            section = existing
            continue
        key, separator, value = line.partition("=")
        key = key.strip()
        value = value.strip()
        if not separator or not key or not value:
            fail(f"{path}:{line_number}: expected key = value")
        if key in section:
            fail(f"{path}:{line_number}: duplicate key {key!r}")
        section[key] = parse_scalar(value, path, line_number)
    return document


def crate_inventory(path: Path) -> dict[str, str]:
    document = load_simple_toml(path)
    crates = document.get("crates")
    if not isinstance(crates, dict):
        fail(f"{path} is missing [crates]")
    inventory: dict[str, str] = {}
    for package, tier in crates.items():
        if not isinstance(package, str) or not isinstance(tier, str):
            fail(f"{path}: crate inventory must contain string tiers")
        if tier not in VALID_TIERS:
            fail(f"{path}: invalid tier {tier!r} for {package}")
        inventory[package] = tier
    return inventory


def compute_changes(baseline: dict[str, str], current: dict[str, str]) -> list[dict[str, Any]]:
    changes: list[dict[str, Any]] = []
    for package in sorted(current.keys() - baseline.keys()):
        changes.append(
            {
                "kind": "crate-added",
                "package": package,
                "before": None,
                "after": current[package],
            }
        )
    for package in sorted(baseline.keys() - current.keys()):
        changes.append(
            {
                "kind": "crate-removed",
                "package": package,
                "before": baseline[package],
                "after": None,
            }
        )
    for package in sorted(baseline.keys() & current.keys()):
        if baseline[package] != current[package]:
            changes.append(
                {
                    "kind": "tier-changed",
                    "package": package,
                    "before": baseline[package],
                    "after": current[package],
                }
            )
    return changes


def change_key(change: dict[str, Any]) -> tuple[str, str]:
    kind = change.get("kind")
    package = change.get("package")
    if not isinstance(kind, str) or not isinstance(package, str):
        fail("every compatibility change requires string kind and package fields")
    return kind, package


def load_classifications(path: Path) -> dict[tuple[str, str], dict[str, Any]]:
    if not path.is_file():
        fail(f"missing compatibility classification: {path}")
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        fail(f"{path}: invalid JSON: {error}")
    if not isinstance(document, dict) or document.get("version") != 1:
        fail(f"{path}: expected a version 1 object")
    changes = document.get("changes")
    if not isinstance(changes, list):
        fail(f"{path}: changes must be an array")
    result: dict[tuple[str, str], dict[str, Any]] = {}
    for change in changes:
        if not isinstance(change, dict):
            fail(f"{path}: every change must be an object")
        key = change_key(change)
        if key in result:
            fail(f"{path}: duplicate classification for {key[0]} {key[1]}")
        classification = change.get("classification")
        rationale = change.get("rationale")
        if classification not in VALID_CLASSIFICATIONS:
            fail(f"{path}: invalid classification {classification!r} for {key[1]}")
        if not isinstance(rationale, str) or not rationale.strip():
            fail(f"{path}: missing rationale for {key[1]}")
        result[key] = change
    return result


def evaluate(
    changes: list[dict[str, Any]],
    classifications: dict[tuple[str, str], dict[str, Any]],
) -> list[dict[str, Any]]:
    actual_keys = {change_key(change) for change in changes}
    classified_keys = set(classifications)
    if actual_keys != classified_keys:
        missing = sorted(actual_keys - classified_keys)
        stale = sorted(classified_keys - actual_keys)
        fail(f"API contract changes are not fully classified; missing={missing}, stale={stale}")

    report_changes: list[dict[str, Any]] = []
    for change in changes:
        key = change_key(change)
        classification = classifications[key]
        combined = dict(change)
        combined["classification"] = classification["classification"]
        combined["rationale"] = classification["rationale"]
        report_changes.append(combined)

        if combined["classification"] == "accidental":
            fail(f"accidental API contract change: {key[0]} {key[1]}")
        if combined["kind"] == "crate-removed" and combined["before"] == "stable":
            if combined["classification"] != "intentionally-breaking":
                fail(f"stable crate removal must be intentionally-breaking: {key[1]}")
        if (
            combined["kind"] == "tier-changed"
            and combined["before"] == "stable"
            and combined["after"] != "stable"
            and combined["classification"] != "intentionally-breaking"
        ):
            fail(f"stable tier downgrade must be intentionally-breaking: {key[1]}")
    return report_changes


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, default=DEFAULT_BASELINE)
    parser.add_argument("--current", type=Path, default=DEFAULT_CURRENT)
    parser.add_argument("--classification", type=Path, default=DEFAULT_CLASSIFICATION)
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def main() -> None:
    arguments = parse_arguments()
    baseline = crate_inventory(arguments.baseline)
    current = crate_inventory(arguments.current)
    changes = compute_changes(baseline, current)
    classifications = load_classifications(arguments.classification)
    report_changes = evaluate(changes, classifications)
    report = {
        "schema_version": 1,
        "baseline": str(arguments.baseline),
        "current": str(arguments.current),
        "classification": str(arguments.classification),
        "baseline_crate_count": len(baseline),
        "current_crate_count": len(current),
        "changes": report_changes,
        "breaking_change_count": sum(
            change["classification"] == "intentionally-breaking" for change in report_changes
        ),
        "passed": True,
    }
    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if arguments.output is not None:
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(encoded, encoding="utf-8")
        print(f"[ OK ] Wrote API contract diff: {arguments.output}")
    else:
        sys.stdout.write(encoded)
    print(
        f"[ OK ] Classified {len(report_changes)} API contract change(s); "
        f"{report['breaking_change_count']} intentionally breaking."
    )


if __name__ == "__main__":
    main()
