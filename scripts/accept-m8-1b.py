#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Record confirmed M8.1b symbol-level API acceptance after report review."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SNAPSHOTS = ROOT / "api" / "snapshots"
DEFAULT_EVIDENCE = ROOT / "release" / "evidence" / "m8.1b-symbol-api"


def fail(message: str) -> None:
    print(f"[FAIL] {message}", file=sys.stderr)
    raise SystemExit(1)


def load_passing_json(path: Path, label: str) -> dict[str, object]:
    if not path.is_file():
        fail(f"missing {label}: {path}")
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        fail(f"invalid {label}: {error}")
    if not isinstance(document, dict) or document.get("passed") is not True:
        fail(f"{label} did not record passed=true: {path}")
    return document


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    if new in text:
        print(f"[ OK ] already accepted {path.relative_to(ROOT)}")
        return
    count = text.count(old)
    if count != 1:
        fail(f"{path.relative_to(ROOT)}: expected one acceptance block, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")
    print(f"[ OK ] updated {path.relative_to(ROOT)}")


def replace_or_append_section(path: Path, old: str, new: str) -> None:
    # Replace a candidate section, or append acceptance when the candidate was never checked in.
    text = path.read_text(encoding="utf-8")
    if new in text:
        print(f"[ OK ] already accepted {path.relative_to(ROOT)}")
        return
    count = text.count(old)
    if count == 1:
        path.write_text(text.replace(old, new, 1), encoding="utf-8")
        print(f"[ OK ] updated {path.relative_to(ROOT)}")
        return
    if count > 1:
        fail(f"{path.relative_to(ROOT)}: duplicate candidate blocks: {count}")
    heading = new.splitlines()[0]
    if heading in text:
        fail(f"{path.relative_to(ROOT)}: acceptance heading exists with unexpected content")
    path.write_text(text.rstrip() + "\n\n" + new.strip() + "\n", encoding="utf-8")
    print(f"[ OK ] appended missing acceptance section to {path.relative_to(ROOT)}")


def replace_or_append_line(path: Path, old: str, new: str) -> None:
    # Replace one roadmap line, or append completion when the active line was absent.
    text = path.read_text(encoding="utf-8")
    if new in text:
        print(f"[ OK ] already accepted {path.relative_to(ROOT)}")
        return
    count = text.count(old)
    if count == 1:
        path.write_text(text.replace(old, new, 1), encoding="utf-8")
        print(f"[ OK ] updated {path.relative_to(ROOT)}")
        return
    if count > 1:
        fail(f"{path.relative_to(ROOT)}: duplicate roadmap lines: {count}")
    path.write_text(text.rstrip() + "\n" + new + "\n", encoding="utf-8")
    print(f"[ OK ] appended missing roadmap completion to {path.relative_to(ROOT)}")


def git_output(*arguments: str) -> str:
    completed = subprocess.run(
        ["git", *arguments],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        fail(completed.stderr.strip() or f"git {' '.join(arguments)} failed")
    return completed.stdout.strip()


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--snapshots", type=Path, default=DEFAULT_SNAPSHOTS)
    parser.add_argument("--evidence", type=Path, default=DEFAULT_EVIDENCE)
    parser.add_argument("--confirm-stable-review", action="store_true")
    parser.add_argument("--confirm-provisional-review", action="store_true")
    parser.add_argument("--confirm-semver-review", action="store_true")
    parser.add_argument("--confirm-snapshot-retention", action="store_true")
    return parser.parse_args()


def refresh_checksums(directory: Path) -> None:
    lines = []
    for path in sorted(directory.rglob("*")):
        if not path.is_file() or path.name == "SHA256SUMS":
            continue
        relative = path.relative_to(directory)
        lines.append(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {relative}")
    (directory / "SHA256SUMS").write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> None:
    arguments = parse_arguments()
    confirmations = {
        "stable-crate symbol diff review": arguments.confirm_stable_review,
        "provisional-crate symbol diff review": arguments.confirm_provisional_review,
        "cargo-semver-checks review": arguments.confirm_semver_review,
        "baseline/current snapshot retention": arguments.confirm_snapshot_retention,
    }
    missing = [name for name, confirmed in confirmations.items() if not confirmed]
    if missing:
        fail(f"M8.1b confirmations are incomplete: {missing}")

    snapshots = arguments.snapshots.expanduser().resolve()
    evidence = arguments.evidence.expanduser().resolve()
    baseline_manifest = load_passing_json(
        evidence / "api-symbol-diff.json", "symbol API comparison"
    )
    semver = load_passing_json(evidence / "semver-summary.json", "semver summary")
    for directory in [snapshots / "m7.0.1", snapshots / "m8.1b"]:
        if not (directory / "MANIFEST.json").is_file() or not (directory / "SHA256SUMS").is_file():
            fail(f"incomplete retained snapshot directory: {directory}")

    candidate = """## M8.1b symbol-level API candidate

M8.1b pins the API review toolchain, captures accepted M7 and current public-symbol snapshots for every
public library crate, rejects unclassified drift, and runs `cargo-semver-checks` across every stable
crate shared with the baseline. Acceptance remains pending until the generated diffs, snapshot
manifests, checksums, and semver logs are reviewed and retained from the exact implementation commit.
"""
    accepted = """## M8.1b symbol-level API acceptance

M8.1b is accepted on the blocking Linux/Pop!_OS lane. Pinned symbol snapshots were retained for every
accepted M7 and current public crate, every difference was explicitly classified, snapshot checksums
passed, and `cargo-semver-checks` completed across every stable crate shared with the baseline. The
accepted comparison contains no unclassified or accidental change.
"""
    replace_or_append_section(ROOT / "docs" / "CURRENT_STATUS.md", candidate, accepted)
    replace_or_append_section(ROOT / "VALIDATION_REPORT.md", candidate, accepted)
    replace_or_append_line(
        ROOT / "docs" / "ROADMAP.md",
        "- M8.1b captures symbol-level snapshots for every public crate, classifies all drift, and runs stable-crate semver review — active.",
        "- M8.1b symbol-level snapshots, complete drift classification, and stable-crate semver review — complete after Linux acceptance.",
    )

    checklist = ROOT / "docs" / "RELEASE_CHECKLIST.md"
    text = checklist.read_text(encoding="utf-8")
    heading = "## M8.1b symbol-level API acceptance"
    if heading not in text:
        fail("M8.1b release-checklist section is missing")
    before, section = text.split(heading, 1)
    if "\n## " in section:
        body, after = section.split("\n## ", 1)
        suffix = "\n## " + after
    else:
        body, suffix = section, ""
    body = body.replace("- [ ]", "- [x]")
    checklist.write_text(before + heading + body + suffix, encoding="utf-8")
    print("[ OK ] checked M8.1b release checklist")

    evidence.mkdir(parents=True, exist_ok=True)
    acceptance_path = evidence / "M8_1B_ACCEPTANCE.md"
    acceptance_path.write_text(
        "\n".join(
            [
                "# M8.1b Symbol-Level API Acceptance",
                "",
                f"- Recorded UTC: `{datetime.now(timezone.utc).replace(microsecond=0).isoformat()}`",
                f"- Source HEAD: `{git_output('rev-parse', 'HEAD')}`",
                f"- Baseline commit: `{baseline_manifest.get('baseline_commit')}`",
                f"- Current snapshot commit: `{baseline_manifest.get('current_commit')}`",
                f"- Changed crates: {baseline_manifest.get('changed_crate_count')}",
                f"- Added symbols: {baseline_manifest.get('added_symbol_count')}",
                f"- Removed symbols: {baseline_manifest.get('removed_symbol_count')}",
                f"- Stable crates semver-reviewed: {semver.get('stable_crate_count')}",
                "- Stable-crate symbol diff review: passed",
                "- Provisional-crate symbol diff review: passed",
                "- Snapshot checksum and retention review: passed",
                "- Unclassified changes: none",
                "- Accidental changes: none",
                "",
            ]
        ),
        encoding="utf-8",
    )
    refresh_checksums(evidence)
    print(f"[ OK ] wrote {acceptance_path.relative_to(ROOT)}")
    print("M8.1b acceptance recorded. Review the documentation and evidence diff before committing.")


if __name__ == "__main__":
    main()
