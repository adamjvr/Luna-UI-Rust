#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo fmt --all -- --check
cargo test -p luna-document-services --test downstream_product_file_lifecycle -- --nocapture

test -f docs/M9_4_DOWNSTREAM_FILE_LIFECYCLE.md
grep -q "downstream product" docs/M9_4_DOWNSTREAM_FILE_LIFECYCLE.md
grep -q "resolve_save_conflict_for_product" crates/luna-document-services/src/lib.rs
grep -q "outside {application_name}" crates/luna-document-services/src/lib.rs

echo "m9_4_downstream_file_lifecycle=passed"
