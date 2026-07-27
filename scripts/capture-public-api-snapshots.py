#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Capture deterministic cargo-public-api snapshots for the M7 baseline and current tree."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BASELINE_COMMIT = "e696df0cedaeda7ac5c0892cf8f709f8325eff8b"
DEFAULT_NIGHTLY = "nightly-2026-07-10"
EXPECTED_PUBLIC_API_VERSION = "0.52.0"
VALID_TIERS = {"stable", "provisional", "internal"}


def fail(message: str) -> None:
    print(f"[FAIL] {message}", file=sys.stderr)
    raise SystemExit(1)


def run(
    command: list[str],
    *,
    cwd: Path = ROOT,
    capture: bool = False,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=cwd,
        check=False,
        text=True,
        capture_output=capture,
        env=env,
    )
    if completed.returncode != 0:
        if capture:
            sys.stdout.write(completed.stdout)
            sys.stderr.write(completed.stderr)
        fail(f"command failed ({completed.returncode}): {' '.join(command)}")
    return completed


def git_output(*arguments: str, cwd: Path = ROOT) -> str:
    return run(["git", *arguments], cwd=cwd, capture=True).stdout.strip()


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
        fail(f"missing contract manifest: {path}")
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
    return dict(sorted(inventory.items()))


def command_output(command: list[str], *, cwd: Path = ROOT) -> str:
    return run(command, cwd=cwd, capture=True).stdout.strip()


def verify_toolchain(nightly: str) -> dict[str, str]:
    public_api = command_output(["cargo", f"+{nightly}", "public-api", "--version"])
    if EXPECTED_PUBLIC_API_VERSION not in public_api:
        fail(
            "cargo-public-api version mismatch; "
            f"expected {EXPECTED_PUBLIC_API_VERSION}, observed {public_api!r}"
        )
    return {
        "cargo_public_api": public_api,
        "rustc": command_output(["rustc", f"+{nightly}", "--version"]),
        "cargo": command_output(["cargo", f"+{nightly}", "--version"]),
    }


def working_tree_fingerprint() -> tuple[bool, str]:
    status = run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        capture=True,
    ).stdout
    return bool(status.strip()), hashlib.sha256(status.encode("utf-8")).hexdigest()


def write_checksums(directory: Path) -> None:
    lines = []
    for path in sorted(directory.iterdir()):
        if not path.is_file() or path.name == "SHA256SUMS":
            continue
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        lines.append(f"{digest}  {path.name}")
    (directory / "SHA256SUMS").write_text("\n".join(lines) + "\n", encoding="utf-8")


def capture_one_tree(
    *,
    source_root: Path,
    destination: Path,
    inventory: dict[str, str],
    nightly: str,
    source_commit: str,
    source_kind: str,
    working_tree_dirty: bool,
    working_tree_status_sha256: str,
    tools: dict[str, str],
) -> None:
    stage = Path(tempfile.mkdtemp(prefix="luna-api-snapshot-stage-"))
    target_dir = Path(tempfile.mkdtemp(prefix="luna-api-target-"))
    try:
        for index, package in enumerate(sorted(inventory), 1):
            print(f"==> [{index}/{len(inventory)}] Capturing {package} from {source_kind}")
            output = stage / f"{package}.txt"
            environment = os.environ.copy()
            environment["CARGO_TARGET_DIR"] = str(target_dir / package)
            environment["CARGO_TERM_COLOR"] = "never"
            with output.open("w", encoding="utf-8") as stream:
                completed = subprocess.run(
                    [
                        "cargo",
                        f"+{nightly}",
                        "public-api",
                        "-p",
                        package,
                        "--all-features",
                        "-sss",
                    ],
                    cwd=source_root,
                    check=False,
                    text=True,
                    stdout=stream,
                    env=environment,
                )
            if completed.returncode != 0:
                fail(f"cargo public-api failed for {package} in {source_kind}")

        manifest: dict[str, Any] = {
            "schema_version": 1,
            "source_kind": source_kind,
            "source_commit": source_commit,
            "working_tree_dirty": working_tree_dirty,
            "working_tree_status_sha256": working_tree_status_sha256,
            "generated_utc": datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
            "nightly": nightly,
            "tools": tools,
            "crate_count": len(inventory),
            "crates": inventory,
        }
        (stage / "MANIFEST.json").write_text(
            json.dumps(manifest, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        write_checksums(stage)
        if destination.exists():
            shutil.rmtree(destination)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(stage), str(destination))
    finally:
        shutil.rmtree(stage, ignore_errors=True)
        shutil.rmtree(target_dir, ignore_errors=True)


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-commit", default=DEFAULT_BASELINE_COMMIT)
    parser.add_argument("--baseline-name", default="m7.0.1")
    parser.add_argument("--current-name", default="m8.1b")
    parser.add_argument("--nightly", default=os.environ.get("LUNA_API_NIGHTLY", DEFAULT_NIGHTLY))
    parser.add_argument(
        "--output-root",
        type=Path,
        default=Path(os.environ.get("LUNA_API_SNAPSHOT_ROOT", ROOT / "api" / "snapshots")),
    )
    return parser.parse_args()


def main() -> None:
    arguments = parse_arguments()
    output_root = arguments.output_root.expanduser().resolve()
    tools = verify_toolchain(arguments.nightly)
    head = git_output("rev-parse", "HEAD")
    dirty, status_hash = working_tree_fingerprint()
    current_inventory = crate_inventory(ROOT / "api" / "public-api.toml")

    temp_root = Path(tempfile.mkdtemp(prefix="luna-m8.1b-baseline-worktree-"))
    baseline_root = temp_root / "checkout"
    worktree_added = False
    try:
        run(
            [
                "git",
                "worktree",
                "add",
                "--detach",
                str(baseline_root),
                arguments.baseline_commit,
            ]
        )
        worktree_added = True
        baseline_inventory = crate_inventory(baseline_root / "api" / "public-api.toml")

        capture_one_tree(
            source_root=baseline_root,
            destination=output_root / arguments.baseline_name,
            inventory=baseline_inventory,
            nightly=arguments.nightly,
            source_commit=arguments.baseline_commit,
            source_kind="accepted-baseline",
            working_tree_dirty=False,
            working_tree_status_sha256=hashlib.sha256(b"").hexdigest(),
            tools=tools,
        )
        capture_one_tree(
            source_root=ROOT,
            destination=output_root / arguments.current_name,
            inventory=current_inventory,
            nightly=arguments.nightly,
            source_commit=head,
            source_kind="current-working-tree" if dirty else "current-commit",
            working_tree_dirty=dirty,
            working_tree_status_sha256=status_hash,
            tools=tools,
        )
    finally:
        if worktree_added:
            subprocess.run(
                ["git", "worktree", "remove", "--force", str(baseline_root)],
                cwd=ROOT,
                check=False,
            )
        shutil.rmtree(temp_root, ignore_errors=True)
        subprocess.run(["git", "worktree", "prune"], cwd=ROOT, check=False)

    print(f"[ OK ] Baseline snapshots: {output_root / arguments.baseline_name}")
    print(f"[ OK ] Current snapshots:  {output_root / arguments.current_name}")


if __name__ == "__main__":
    main()
