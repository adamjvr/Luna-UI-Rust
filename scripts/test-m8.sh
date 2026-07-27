#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

step() {
    printf '==> %s\n' "$1"
}

baseline="${LUNA_M8_API_BASELINE:-api/baselines/m7.0.1.toml}"
evidence_name="${LUNA_M8_EVIDENCE_NAME:-m8-working}"

step "Re-running complete M7 release qualification"
./scripts/test-m7.sh

step "Verifying retained M7.0.1 API baseline"
test -f "$baseline"
python3 - "$baseline" <<'PYTHON'
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
required = [
    'milestone = "M7.0.1"',
    'accepted_commit = "e696df0cedaeda7ac5c0892cf8f709f8325eff8b"',
    '[crates]',
    'luna-core = "stable"',
    'luna-ui = "provisional"',
]
missing = [entry for entry in required if entry not in text]
if missing:
    raise SystemExit(f"retained baseline is incomplete: {missing}")
PYTHON

step "Checking M8 release-candidate documentation"
test -f CHANGELOG.md
test -f docs/M8_RELEASE_CANDIDATE.md
grep -q "M8 Release-Candidate" docs/M8_RELEASE_CANDIDATE.md
grep -q "0.2.0-rc.1" docs/M8_RELEASE_CANDIDATE.md

step "Capturing reproducible M8 evidence"
./scripts/capture-release-evidence.sh "$evidence_name"

step "M8 baseline and evidence gate complete"
