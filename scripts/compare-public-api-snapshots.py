#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Compare retained symbol snapshots and require an explicit classification for every difference."""

from __future__ import annotations

import argparse
import collections
import difflib
import json
import shutil
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_POLICY = ROOT / "api" / "compatibility" / "m8.1-symbol-policy.json"
VALID_CLASSIFICATIONS = {"compatible", "intentionally-breaking", "accidental"}


def fail(message: str) -> None:
    print(f"[FAIL] {message}", file=sys.stderr)
    raise SystemExit(1)


def load_json(path: Path, label: str) -> dict[str, Any]:
    if not path.is_file():
        fail(f"missing {label}: {path}")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        fail(f"invalid {label}: {error}")
    if not isinstance(value, dict):
        fail(f"{label} must be a JSON object: {path}")
    return value


def load_manifest(directory: Path) -> dict[str, Any]:
    manifest = load_json(directory / "MANIFEST.json", "snapshot manifest")
    crates = manifest.get("crates")
    if not isinstance(crates, dict) or not all(
        isinstance(package, str) and isinstance(tier, str)
        for package, tier in crates.items()
    ):
        fail(f"invalid crate inventory in {directory / 'MANIFEST.json'}")
    return manifest


def load_policy(path: Path) -> tuple[dict[tuple[str, str], dict[str, Any]], dict[str, Any]]:
    policy = load_json(path, "symbol compatibility policy")
    if policy.get("version") != 1:
        fail(f"{path}: expected version 1")
    changes = policy.get("changes")
    if not isinstance(changes, list):
        fail(f"{path}: changes must be an array")
    result: dict[tuple[str, str], dict[str, Any]] = {}
    for entry in changes:
        if not isinstance(entry, dict):
            fail(f"{path}: every change must be an object")
        kind = entry.get("kind")
        package = entry.get("package")
        classification = entry.get("classification")
        rationale = entry.get("rationale")
        if not isinstance(kind, str) or not isinstance(package, str):
            fail(f"{path}: each change requires string kind and package")
        if classification not in VALID_CLASSIFICATIONS:
            fail(f"{path}: invalid classification for {kind} {package}")
        if not isinstance(rationale, str) or not rationale.strip():
            fail(f"{path}: missing rationale for {kind} {package}")
        key = (kind, package)
        if key in result:
            fail(f"{path}: duplicate policy entry for {kind} {package}")
        result[key] = entry
    return result, policy


def lines_for(directory: Path, package: str) -> list[str]:
    path = directory / f"{package}.txt"
    if not path.is_file():
        fail(f"missing snapshot for {package}: {path}")
    return path.read_text(encoding="utf-8").splitlines()


def line_delta(before: list[str], after: list[str]) -> tuple[list[str], list[str]]:
    before_counts = collections.Counter(before)
    after_counts = collections.Counter(after)
    removed: list[str] = []
    added: list[str] = []
    for line in sorted(before_counts.keys() | after_counts.keys()):
        removed.extend([line] * max(0, before_counts[line] - after_counts[line]))
        added.extend([line] * max(0, after_counts[line] - before_counts[line]))
    return removed, added


def classify_change(
    *,
    change: dict[str, Any],
    policies: dict[tuple[str, str], dict[str, Any]],
) -> dict[str, Any]:
    key = (change["kind"], change["package"])
    policy = policies.get(key)
    if policy is None:
        fail(f"unclassified symbol API change: {key[0]} {key[1]}")
    combined = dict(change)
    combined["classification"] = policy["classification"]
    combined["rationale"] = policy["rationale"]
    if combined["classification"] == "accidental":
        fail(f"accidental symbol API change: {key[0]} {key[1]}")
    if (
        combined.get("before_tier") == "stable"
        and combined["removed_symbol_count"] > 0
        and combined["classification"] != "intentionally-breaking"
    ):
        fail(f"stable symbol removal is not intentionally breaking: {key[1]}")
    return combined


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-dir", type=Path, required=True)
    parser.add_argument("--current-dir", type=Path, required=True)
    parser.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    parser.add_argument("--output-dir", type=Path, required=True)
    return parser.parse_args()


def main() -> None:
    arguments = parse_arguments()
    baseline_dir = arguments.baseline_dir.expanduser().resolve()
    current_dir = arguments.current_dir.expanduser().resolve()
    output_dir = arguments.output_dir.expanduser().resolve()
    baseline = load_manifest(baseline_dir)
    current = load_manifest(current_dir)
    policies, policy_document = load_policy(arguments.policy)

    expected_commit = policy_document.get("baseline_commit")
    if baseline.get("source_commit") != expected_commit:
        fail(
            "baseline snapshot commit mismatch; "
            f"expected {expected_commit}, observed {baseline.get('source_commit')}"
        )

    baseline_crates = baseline["crates"]
    current_crates = current["crates"]
    if not isinstance(baseline_crates, dict) or not isinstance(current_crates, dict):
        fail("snapshot manifests contain invalid crate inventories")

    output_dir.mkdir(parents=True, exist_ok=True)
    diff_dir = output_dir / "diffs"
    if diff_dir.exists():
        shutil.rmtree(diff_dir)
    diff_dir.mkdir(parents=True)

    changes: list[dict[str, Any]] = []
    all_packages = sorted(set(baseline_crates) | set(current_crates))
    for package in all_packages:
        before_tier = baseline_crates.get(package)
        after_tier = current_crates.get(package)
        before = lines_for(baseline_dir, package) if before_tier is not None else []
        after = lines_for(current_dir, package) if after_tier is not None else []
        if before == after and before_tier == after_tier:
            continue
        if before_tier is None:
            kind = "crate-added"
        elif after_tier is None:
            kind = "crate-removed"
        else:
            kind = "symbols-changed"
        removed, added = line_delta(before, after)
        diff_lines = list(
            difflib.unified_diff(
                [f"{line}\n" for line in before],
                [f"{line}\n" for line in after],
                fromfile=f"m7.0.1/{package}.txt",
                tofile=f"m8.1b/{package}.txt",
            )
        )
        (diff_dir / f"{package}.diff").write_text("".join(diff_lines), encoding="utf-8")
        changes.append(
            classify_change(
                change={
                    "kind": kind,
                    "package": package,
                    "before_tier": before_tier,
                    "after_tier": after_tier,
                    "before_symbol_count": len(before),
                    "after_symbol_count": len(after),
                    "removed_symbol_count": len(removed),
                    "added_symbol_count": len(added),
                    "removed_symbols": removed,
                    "added_symbols": added,
                    "diff": f"diffs/{package}.diff",
                },
                policies=policies,
            )
        )

    actual_keys = {(change["kind"], change["package"]) for change in changes}
    stale = sorted(set(policies) - actual_keys)
    if stale:
        fail(f"stale symbol API classifications with no matching change: {stale}")

    report = {
        "schema_version": 1,
        "phase": "M8.1b",
        "baseline_manifest": str(baseline_dir / "MANIFEST.json"),
        "current_manifest": str(current_dir / "MANIFEST.json"),
        "policy": str(arguments.policy),
        "baseline_commit": baseline.get("source_commit"),
        "current_commit": current.get("source_commit"),
        "baseline_crate_count": len(baseline_crates),
        "current_crate_count": len(current_crates),
        "changed_crate_count": len(changes),
        "stable_changed_crate_count": sum(
            change.get("before_tier") == "stable" or change.get("after_tier") == "stable"
            for change in changes
        ),
        "removed_symbol_count": sum(change["removed_symbol_count"] for change in changes),
        "added_symbol_count": sum(change["added_symbol_count"] for change in changes),
        "intentionally_breaking_count": sum(
            change["classification"] == "intentionally-breaking" for change in changes
        ),
        "changes": changes,
        "passed": True,
    }
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "api-symbol-diff.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    markdown = [
        "# M8.1b Symbol-Level API Comparison",
        "",
        f"- Baseline commit: `{report['baseline_commit']}`",
        f"- Current commit: `{report['current_commit']}`",
        f"- Baseline crates: {report['baseline_crate_count']}",
        f"- Current crates: {report['current_crate_count']}",
        f"- Changed crates: {report['changed_crate_count']}",
        f"- Added symbols: {report['added_symbol_count']}",
        f"- Removed symbols: {report['removed_symbol_count']}",
        f"- Intentionally breaking changes: {report['intentionally_breaking_count']}",
        "",
        "## Classified differences",
        "",
    ]
    if not changes:
        markdown.append("No symbol-level public API differences were detected.")
    for change in changes:
        markdown.extend(
            [
                f"### `{change['package']}` — {change['classification']}",
                "",
                f"- Kind: `{change['kind']}`",
                f"- Tier: `{change['before_tier']}` → `{change['after_tier']}`",
                f"- Added symbols: {change['added_symbol_count']}",
                f"- Removed symbols: {change['removed_symbol_count']}",
                f"- Diff: `{change['diff']}`",
                f"- Rationale: {change['rationale']}",
                "",
            ]
        )
    (output_dir / "API_SYMBOL_DIFF.md").write_text("\n".join(markdown) + "\n", encoding="utf-8")
    print(
        "[ OK ] Symbol API comparison passed: "
        f"{len(changes)} changed crate(s), {report['removed_symbol_count']} removal(s), "
        f"{report['added_symbol_count']} addition(s)."
    )


if __name__ == "__main__":
    main()
