#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo fmt --all -- --check
cargo test -p luna-integration
cargo clippy -p luna-integration --all-targets -- -D warnings

cargo check \
  --manifest-path downstream/luna-reference-consumer/Cargo.toml \
  --all-targets

echo "m9_3_downstream_product_boundary=passed"
