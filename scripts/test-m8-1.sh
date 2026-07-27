#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

step() {
    printf '==> %s\n' "$1"
}

evidence_name="${LUNA_M8_EVIDENCE_NAME:-m8.1-clipboard-api}"
evidence_dir="${LUNA_EVIDENCE_DIR:-release/evidence/${evidence_name}}"

step "Running complete M8 baseline and evidence gate"
LUNA_M8_EVIDENCE_NAME="$evidence_name" ./scripts/test-m8.sh

step "Classifying the M7-to-M8.1 crate-contract difference"
python3 scripts/check-api-contract-diff.py \
    --output "$evidence_dir/api-contract-diff.json"

step "Running focused clipboard service tests"
cargo test -p luna-clipboard

step "Running editor clipboard integration tests"
cargo test \
    -p luna-ui-rust-editor-demo \
    --bin luna-ui-rust-editor-demo \
    clipboard

step "Building the release editor with native clipboard integration"
cargo build --release -p luna-ui-rust-editor-demo

step "M8.1a clipboard and API-contract gate complete"
printf '%s\n' \
    "Manual acceptance remains: CPU Cut/Copy/Paste, wgpu Cut/Copy/Paste," \
    "cross-application clipboard transfer, multi-selection copy, undo after Cut/Paste," \
    "and extracted-package clipboard operation."
