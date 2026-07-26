#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

step() {
    printf '==> %s\n' "$1"
}

step "Formatting workspace"
cargo fmt --all

step "Verifying formatting"
cargo fmt --all -- --check

step "Checking all targets and features"
cargo check --workspace --all-targets --all-features

step "Running focused M5 editor-mechanics tests"
cargo test -p luna-editor
cargo test -p luna-text-cosmic
cargo test -p luna-accessibility -p luna-accessibility-accesskit
cargo test -p luna-commands
cargo test -p luna-ui-rust-editor-demo

step "Running strict Clippy"
cargo clippy --workspace --all-targets --all-features -- -D warnings

step "Running complete workspace tests"
cargo test --workspace --all-targets --all-features

step "Checking rustdoc"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

fixture_dir="${TMPDIR:-/tmp}/luna-m5-parity"
rm -rf "$fixture_dir"
mkdir -p "$fixture_dir"
cat > "$fixture_dir/README.txt" <<'CHECKLIST'
Luna-UI-Rust M5 runtime acceptance

1. Run the editor through the CPU and wgpu backends.
2. Verify Rust-like keywords, comments, strings, numbers, and type-like identifiers use imported
   Sublime color-scheme foregrounds without changing layout or hit-test geometry.
3. Type several characters, then verify Undo coalesces the typing run and Redo restores it.
4. Add cursors with Control-Shift-Up/Down. Type, Backspace, Delete, Enter, completion acceptance,
   Replace Current, and Replace All; verify simultaneous edits and undo/redo are deterministic.
5. Exercise an IME: begin pre-edit, move within its selected range, commit, cancel, and switch focus.
   Verify candidate UI anchors to the caret and pre-edit never enters history before commit.
6. Use a screen reader to focus the editor and request replacement/value actions. Verify the host
   delivers UTF-8 payloads through the product-neutral accessibility action contract.
7. Repeat with Luna Dark, Luna Light, Amber Monitor, and Green Terminal.
CHECKLIST

step "M5 automated gate complete"
printf 'Runtime checklist: %s\n' "$fixture_dir/README.txt"
