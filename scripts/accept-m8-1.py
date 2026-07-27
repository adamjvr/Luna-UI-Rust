#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Record confirmed M8.1a Linux clipboard acceptance after operator testing."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_EVIDENCE = ROOT / "release" / "evidence" / "m8.1-clipboard-api"


def fail(message: str) -> None:
    print(f"[FAIL] {message}", file=sys.stderr)
    raise SystemExit(1)


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


def load_passing_json(path: Path, label: str) -> dict[str, object]:
    if not path.is_file():
        fail(f"missing {label}: {path}")
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        fail(f"invalid {label} JSON: {error}")
    if not isinstance(document, dict) or document.get("passed") is not True:
        fail(f"{label} did not record passed=true: {path}")
    return document


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
    parser.add_argument("--evidence", type=Path, default=DEFAULT_EVIDENCE)
    parser.add_argument("--confirm-cpu", action="store_true")
    parser.add_argument("--confirm-wgpu", action="store_true")
    parser.add_argument("--confirm-cross-application", action="store_true")
    parser.add_argument("--confirm-multi-selection", action="store_true")
    parser.add_argument("--confirm-undo-redo", action="store_true")
    parser.add_argument("--confirm-package", action="store_true")
    return parser.parse_args()


def main() -> None:
    arguments = parse_arguments()
    confirmations = {
        "CPU editor clipboard": arguments.confirm_cpu,
        "wgpu editor clipboard": arguments.confirm_wgpu,
        "cross-application transfer": arguments.confirm_cross_application,
        "multi-selection and multi-caret behavior": arguments.confirm_multi_selection,
        "Cut/Paste undo and redo": arguments.confirm_undo_redo,
        "extracted Linux package": arguments.confirm_package,
    }
    missing = [name for name, confirmed in confirmations.items() if not confirmed]
    if missing:
        fail(f"manual acceptance confirmations are incomplete: {missing}")

    evidence = arguments.evidence.expanduser().resolve()
    load_passing_json(evidence / "qualification.json", "qualification report")
    load_passing_json(evidence / "api-contract-diff.json", "API contract report")

    replace_once(
        ROOT / "VALIDATION_REPORT.md",
        """## M8.1a Clipboard Candidate

- A provisional `luna-clipboard` service boundary now supplies native and deterministic memory adapters.
- Edit > Cut, Copy, and Paste plus Ctrl+X/C/V share one editor command path across CPU and `wgpu` hosts.
- The candidate remains unaccepted until `scripts/test-m8-1.sh`, cross-application clipboard transfer,
  multi-selection behavior, undo, and extracted-package operation pass locally.
""",
        """## M8.1a Clipboard Acceptance

- A provisional `luna-clipboard` service boundary supplies native and deterministic memory adapters.
- Edit > Cut, Copy, and Paste plus Ctrl+X/C/V share one editor command path across CPU and `wgpu` hosts.
- The complete M8.1a gate, CPU and `wgpu` operator passes, cross-application transfer,
  multi-selection behavior, undo/redo, and extracted-package operation passed on Linux/Pop!_OS.
""",
    )
    replace_once(
        ROOT / "docs" / "CURRENT_STATUS.md",
        """## M8.1a clipboard candidate

The next acceptance candidate adds the provisional `luna-clipboard` crate, native and memory adapters,
Cut/Copy/Paste menu and shortcut routing, deterministic editor tests, and explicit classification of
the additive M7-to-M8.1 crate-contract difference. It is pending the complete M8.1a automated gate and
CPU/`wgpu` cross-application clipboard operator pass.
""",
        """## M8.1a clipboard acceptance

The provisional `luna-clipboard` crate, native and memory adapters, Cut/Copy/Paste menu and shortcut
routing, deterministic editor tests, and additive M7-to-M8.1 crate-contract classification are accepted
on the blocking Linux/Pop!_OS lane. CPU and `wgpu` cross-application transfer, multi-selection,
undo/redo, and extracted-package operation completed locally.
""",
    )
    replace_once(
        ROOT / "docs" / "ROADMAP.md",
        "- M8.1a introduces the provisional clipboard boundary, enables Cut/Copy/Paste, and classifies the additive crate-contract change.",
        "- M8.1a clipboard integration and additive crate-contract classification — complete after Linux acceptance.",
    )

    checklist = ROOT / "docs" / "RELEASE_CHECKLIST.md"
    checklist_text = checklist.read_text(encoding="utf-8")
    heading = "## M8.1a clipboard acceptance"
    if heading not in checklist_text:
        fail("M8.1a checklist section is missing")
    before, section = checklist_text.split(heading, 1)
    if "\n## " in section:
        body, after = section.split("\n## ", 1)
        suffix = "\n## " + after
    else:
        body, suffix = section, ""
    body = body.replace("- [ ]", "- [x]")
    checklist.write_text(before + heading + body + suffix, encoding="utf-8")
    print("[ OK ] checked M8.1a release checklist")

    evidence.mkdir(parents=True, exist_ok=True)
    acceptance_path = evidence / "MANUAL_ACCEPTANCE.md"
    timestamp = datetime.now(timezone.utc).replace(microsecond=0).isoformat()
    head = git_output("rev-parse", "HEAD")
    acceptance_path.write_text(
        "\n".join(
            [
                "# M8.1a Manual Acceptance",
                "",
                f"- Recorded UTC: `{timestamp}`",
                f"- Source HEAD: `{head}`",
                "- Platform: Linux/Pop!_OS primary lane",
                "- CPU editor clipboard: passed",
                "- `wgpu` editor clipboard: passed",
                "- Cross-application transfer: passed",
                "- Multi-selection and multi-caret behavior: passed",
                "- Cut/Paste undo and redo: passed",
                "- Extracted Linux package clipboard: passed",
                "",
                "This record confirms the operator checks requested by the M8.1a release checklist.",
                "",
            ]
        ),
        encoding="utf-8",
    )
    print(f"[ OK ] wrote {acceptance_path.relative_to(ROOT)}")

    checksum_lines = []
    for path in sorted(evidence.iterdir()):
        if not path.is_file() or path.name == "SHA256SUMS":
            continue
        checksum_lines.append(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}")
    (evidence / "SHA256SUMS").write_text(
        "\n".join(checksum_lines) + "\n",
        encoding="utf-8",
    )
    print(f"[ OK ] refreshed {evidence.relative_to(ROOT) / 'SHA256SUMS'}")
    print("M8.1a acceptance recorded. Review the documentation diff before committing.")


if __name__ == "__main__":
    main()
