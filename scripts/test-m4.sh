#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

printf '%s\n' '==> Formatting workspace'
cargo fmt --all

printf '%s\n' '==> Verifying formatting'
cargo fmt --all -- --check

printf '%s\n' '==> Checking all targets and features'
cargo check --workspace --all-targets --all-features

printf '%s\n' '==> Checking focused M4 crates'
cargo check -p luna-render-wgpu -p luna-host-wgpu -p luna-ui-rust-proof-gallery --all-targets

printf '%s\n' '==> Running strict Clippy'
cargo clippy --workspace --all-targets --all-features -- -D warnings

printf '%s\n' '==> Running all tests'
cargo test --workspace --all-targets --all-features

printf '%s\n' '==> Building rustdoc with warnings denied'
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

FIXTURE_ROOT="/tmp/luna-m4-comparison"
printf '%s\n' "==> Creating M4 runtime checklist at $FIXTURE_ROOT"
rm -rf "$FIXTURE_ROOT"
mkdir -p "$FIXTURE_ROOT"
cat > "$FIXTURE_ROOT/README.txt" <<'DOC'
Luna UI Rust M4 runtime acceptance

CPU proof gallery:
  cargo run --release -p luna-ui-rust-proof-gallery

GPU proof gallery:
  LUNA_RENDER_BACKEND=wgpu cargo run --release -p luna-ui-rust-proof-gallery

Optional Vulkan-only GPU run:
  WGPU_BACKEND=vulkan LUNA_RENDER_BACKEND=wgpu cargo run --release -p luna-ui-rust-proof-gallery

For both backends:
  1. Resize repeatedly at 100%, 125%, 150%, and 200% scale where available.
  2. Click the theme card through Luna Dark, Luna Light, Amber Monitor, and Green Terminal.
  3. Activate the button and toggle while animation is running.
  4. Verify multilingual text, image alpha, card clips, hover borders, and accessibility.
  5. Compare layout, clipping, colors, and glyph placement between CPU and GPU.
  6. Inspect stderr metrics and confirm no validation, surface, or device-loss failure.

Editor theme runs:
  cargo run --release -p luna-ui-rust-editor-demo
  LUNA_RENDER_BACKEND=wgpu cargo run --release -p luna-ui-rust-editor-demo
  Open View > Color Scheme and test all four presets through both backends.
DOC

printf '%s\n' '==> M4 automated gate passed'
printf '%s\n' "Runtime checklist: $FIXTURE_ROOT/README.txt"
printf '%s\n' 'The GPU runtime comparison requires a graphical session and compatible driver.'
