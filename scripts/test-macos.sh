#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
    printf 'This validation lane must run on macOS.\n' >&2
    exit 2
fi

step() {
    printf '==> %s\n' "$1"
}

step "Reporting macOS toolchain"
sw_vers
uname -m
rustc --version
cargo --version

step "Verifying formatting"
cargo fmt --all -- --check

step "Checking all targets and features"
cargo check --workspace --all-targets --all-features

step "Running strict Clippy"
cargo clippy --workspace --all-targets --all-features -- -D warnings

step "Running complete workspace tests"
cargo test --workspace --all-targets --all-features

step "Checking rustdoc"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

step "Building native CPU and Metal-backed wgpu proof applications"
cargo build --release -p luna-ui-rust-proof-gallery -p luna-ui-rust-editor-demo

checklist="${TMPDIR:-/tmp}/luna-macos-runtime-checklist.txt"
cat > "$checklist" <<'CHECKLIST'
Luna-UI-Rust macOS manual acceptance

CPU:
  cargo run --release -p luna-ui-rust-proof-gallery
  cargo run --release -p luna-ui-rust-editor-demo

GPU / Metal through wgpu:
  WGPU_BACKEND=metal LUNA_RENDER_BACKEND=wgpu cargo run --release -p luna-ui-rust-proof-gallery
  WGPU_BACKEND=metal LUNA_RENDER_BACKEND=wgpu cargo run --release -p luna-ui-rust-editor-demo

Check Retina scaling, resize/full-screen behavior, all four themes, menus and shortcuts, file dialogs,
UTF-8 file operations, session restore, VoiceOver focus/actions, dead-key composition, emoji input,
and at least one multi-stage CJK IME. Test both CPU and GPU presentation before macOS is promoted
from advisory to a release gate.
CHECKLIST

printf 'macOS automated gate complete. Manual checklist: %s\n' "$checklist"
