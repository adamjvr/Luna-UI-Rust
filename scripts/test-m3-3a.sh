#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

echo "==> Formatting workspace"
cargo fmt --all

echo "==> Verifying formatting"
cargo fmt --all -- --check

echo "==> Checking all targets and features"
cargo check --workspace --all-targets --all-features

echo "==> Running strict Clippy"
cargo clippy --workspace --all-targets --all-features -- -D warnings

echo "==> Running workspace tests"
cargo test --workspace --all-targets --all-features

echo "==> Building rustdoc with warnings denied"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

FIXTURE_ROOT="/tmp/luna-m3-3a-workspace"
rm -rf "$FIXTURE_ROOT"
mkdir -p "$FIXTURE_ROOT/src/nested" "$FIXTURE_ROOT/docs"

cat > "$FIXTURE_ROOT/src/main.rs" <<'RUST'
fn main() {
    println!("M3.3a split-pane fixture");
}
RUST

cat > "$FIXTURE_ROOT/src/shared.rs" <<'RUST'
pub fn shared_buffer_message() -> &'static str {
    "Edit this file in two panes."
}
RUST

cat > "$FIXTURE_ROOT/src/nested/notes.txt" <<'TEXT'
Nested pane test notes.
Line two keeps scrolling and selection checks visible.
Line three exists for independent caret movement.
TEXT

cat > "$FIXTURE_ROOT/docs/panes.md" <<'TEXT'
# Pane Runtime Fixture

Use this file to test pane-local tabs and moving focus between nested splits.
TEXT

cat > "$FIXTURE_ROOT/README.md" <<'TEXT'
# Luna M3.3a Runtime Workspace

Open src/shared.rs, split it right, edit either view, then create a nested downward split.
TEXT

printf '\nM3.3a automated validation passed.\n'
printf 'Runtime fixture: %s\n' "$FIXTURE_ROOT"
printf 'Run: cargo run --release -p luna-ui-rust-editor-demo\n'
