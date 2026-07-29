#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# M8.4 Apple-Silicon automated qualification and evidence-campaign preparation.
set -Eeuo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
output_root="${LUNA_M8_4_OUTPUT_ROOT:-$repo_root/dist/m8.4}"
on_exit(){ status=$?; [[ $status -eq 0 ]] || printf '[FAIL] M8.4 macOS qualification stopped with status %s.\n' "$status" >&2; printf '[SAFE] Your terminal remains open.\n'; }
trap on_exit EXIT
step(){ printf '\n============================================================\n==> %s\n============================================================\n' "$1"; }
if [[ "$(uname -s)" != Darwin ]]; then printf 'M8.4 macOS qualification must run on macOS.\n' >&2; exit 2; fi
if [[ "$(uname -m)" != arm64 ]]; then printf 'M8.4 requires Apple-Silicon arm64 hardware.\n' >&2; exit 2; fi
cd -- "$repo_root"
step "Reporting Apple-Silicon environment"
sw_vers
uname -a
rustc --version
cargo --version
xcodebuild -version
step "Running the M8.3.1 native dialog repair gate"
bash ./scripts/test-m8-3-1.sh
step "Running complete workspace quality gates"
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
step "Running deterministic release qualification"
mkdir -p -- "$output_root"
cargo run --release -p luna-ui-rust-qualification -- --output "$output_root/m8-release-qualification.json"
step "Building graphical applications and private evidence recorder"
cargo build --release \
  -p luna-ui-rust-proof-gallery \
  -p luna-ui-rust-editor-demo
cargo build --release \
  -p luna-ui-rust-qualification \
  --bin luna-ui-rust-m8-4-macos-evidence
step "Building signed CPU and Metal application bundles"
bash ./scripts/package-macos.sh --skip-build --backend cpu \
  --display-name "Luna UI Rust Editor Demo CPU" \
  --bundle-id org.lunaui.rust.editor-demo.cpu \
  --output "$output_root/packages"
bash ./scripts/package-macos.sh --skip-build --backend wgpu \
  --display-name "Luna UI Rust Editor Demo Metal" \
  --bundle-id org.lunaui.rust.editor-demo.metal \
  --output "$output_root/packages"
step "Generating the operator status template"
cargo run --release -p luna-ui-rust-qualification \
  --bin luna-ui-rust-m8-4-macos-evidence -- \
  template --output "$output_root/operator-status.template"
step "Checking source whitespace"
git diff --check
step "M8.4 automated macOS candidate qualification passed"
printf 'CPU bundle:      %s\n' "$output_root/packages/Luna UI Rust Editor Demo CPU.app"
printf 'Metal bundle:    %s\n' "$output_root/packages/Luna UI Rust Editor Demo Metal.app"
printf 'Status template: %s\n' "$output_root/operator-status.template"
printf '[PASS] automated Apple-Silicon build, tests, qualification, signing, and bundle validation passed\n'
