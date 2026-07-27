#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

step() {
    printf '==> %s\n' "$1"
}

snapshot_root="${LUNA_API_SNAPSHOT_ROOT:-api/snapshots}"
report_root="${LUNA_API_REPORT_ROOT:-release/evidence/m8.1b-symbol-api}"
nightly="${LUNA_API_NIGHTLY:-nightly-2026-07-10}"

step "Running the complete accepted M8.1a gate"
./scripts/test-m8-1.sh

step "Verifying pinned API review tools"
cargo +"$nightly" public-api --version
cargo +stable semver-checks --version

step "Capturing accepted M7 and current symbol-level public API snapshots"
python3 scripts/capture-public-api-snapshots.py \
    --nightly "$nightly" \
    --output-root "$snapshot_root"

step "Verifying snapshot checksums"
(
    cd "$snapshot_root/m7.0.1"
    sha256sum -c SHA256SUMS
)
(
    cd "$snapshot_root/m8.1b"
    sha256sum -c SHA256SUMS
)

step "Classifying every symbol-level API difference"
python3 scripts/compare-public-api-snapshots.py \
    --baseline-dir "$snapshot_root/m7.0.1" \
    --current-dir "$snapshot_root/m8.1b" \
    --output-dir "$report_root"

step "Running cargo-semver-checks across accepted stable crates"
python3 scripts/run-semver-review.py \
    --output-dir "$report_root"

step "Re-validating checked-in crate contracts and source whitespace"
python3 scripts/check-public-api.py
git diff --check

step "M8.1b symbol-level API qualification complete"
printf 'Snapshots: %s\n' "$snapshot_root"
printf 'Reports:   %s\n' "$report_root"
printf '%s\n' \
    "Human review remains: inspect API_SYMBOL_DIFF.md, every generated .diff," \
    "SEMVER_REVIEW.md, and the snapshot manifests before recording acceptance."
