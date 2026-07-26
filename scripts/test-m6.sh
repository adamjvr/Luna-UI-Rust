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

step "Running focused M6 platform and integration tests"
cargo test -p luna-host-core
cargo test -p luna-host-winit -p luna-host-wgpu
cargo test -p luna-document-services
cargo test -p luna-session
cargo test -p luna-workspaces
cargo test -p luna-integration
cargo test -p luna-theme
cargo test -p luna-ui-rust-editor-demo

step "Running strict Clippy"
cargo clippy --workspace --all-targets --all-features -- -D warnings

step "Running complete workspace tests"
cargo test --workspace --all-targets --all-features

step "Checking rustdoc"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

fixture_dir="${TMPDIR:-/tmp}/luna-m6-acceptance"
rm -rf "$fixture_dir"
mkdir -p "$fixture_dir"
cat > "$fixture_dir/README.txt" <<'CHECKLIST'
Luna-UI-Rust M6 runtime acceptance

Linux primary:
1. Run CPU and Vulkan/wgpu editor and proof-gallery paths.
2. Verify the Different theme through View > Color Scheme and gallery cycling.
3. Verify native Linux dialogs and native-first workspace watching still fall back safely.
4. Dirty a document, confirm the native edited-state contract reports true, close the window, and
   verify the complete dirty session restores on next launch.

macOS secondary:
1. Run scripts/test-macos.sh on Apple Silicon hardware.
2. Exercise CPU and Metal/wgpu rendering at Retina and external-display scale factors.
3. Test AppleScript Open/Save/Folder/confirmation dialogs and Application Support session paths.
4. Exercise FSEvents workspace delivery, sleep/wake, memory pressure, dead keys, emoji, CJK IME,
   VoiceOver actions, and native document-edited indication.
5. Package and launch the ad-hoc signed .app bundle.

All five presets:
Luna Dark, Luna Light, Amber Monitor, Green Terminal, Different.
CHECKLIST

step "M6 automated gate complete"
printf 'Runtime checklist: %s\n' "$fixture_dir/README.txt"
