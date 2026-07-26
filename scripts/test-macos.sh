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

step "Running focused macOS platform tests"
cargo test -p luna-host-core -p luna-host-winit -p luna-host-wgpu
cargo test -p luna-document-services -p luna-session -p luna-workspaces -p luna-integration
cargo test -p luna-theme -p luna-ui-rust-editor-demo

step "Running strict Clippy"
cargo clippy --workspace --all-targets --all-features -- -D warnings

step "Running complete workspace tests"
cargo test --workspace --all-targets --all-features

step "Checking rustdoc"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

step "Building native CPU and Metal-backed wgpu proof applications"
cargo build --release -p luna-ui-rust-proof-gallery -p luna-ui-rust-editor-demo

step "Building and verifying an ad-hoc signed application bundle"
./scripts/package-macos.sh --skip-build --output "${TMPDIR:-/tmp}/luna-m6-macos-package"

checklist="${TMPDIR:-/tmp}/luna-macos-runtime-checklist.txt"
cat > "$checklist" <<'CHECKLIST'
Luna-UI-Rust macOS M6 manual acceptance

CPU:
  cargo run --release -p luna-ui-rust-proof-gallery
  cargo run --release -p luna-ui-rust-editor-demo

GPU / Metal through wgpu:
  WGPU_BACKEND=metal LUNA_RENDER_BACKEND=wgpu cargo run --release -p luna-ui-rust-proof-gallery
  WGPU_BACKEND=metal LUNA_RENDER_BACKEND=wgpu cargo run --release -p luna-ui-rust-editor-demo

Check Retina scaling, resize/full-screen, all five themes, native document-edited indication,
AppleScript file/folder/confirmation dialogs, Application Support session restoration, FSEvents
workspace refresh, sleep/wake, memory pressure, VoiceOver focus/actions, dead-key composition, emoji,
and at least one multi-stage CJK IME. Test CPU and Metal presentation and launch the packaged .app.
CHECKLIST

printf 'macOS automated gate complete. Manual checklist: %s\n' "$checklist"
