#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

step() {
    printf '==> %s\n' "$1"
}

baseline="${LUNA_SEMVER_BASELINE:-89da6a786357d84a1be4e32f46749fc3666b9f1c}"
qualification_dir="${TMPDIR:-/tmp}/luna-m7-qualification"
package_dir="${TMPDIR:-/tmp}/luna-m7-linux-package"

step "Formatting workspace"
cargo fmt --all

step "Verifying formatting"
cargo fmt --all -- --check

step "Validating public API contracts and package metadata"
python3 scripts/check-public-api.py

step "Checking all targets and features"
cargo check --workspace --all-targets --all-features

step "Running focused M7 API, resource, GPU, and qualification tests"
cargo test -p luna-core -p luna-qualification
cargo test -p luna-integration --all-targets
cargo test -p luna-render-wgpu -p luna-host-wgpu
cargo test -p luna-ui-rust-qualification
cargo test -p luna-ui-rust-editor-demo

step "Running packaged-resource discovery example"
LUNA_RESOURCE_ROOT=docs cargo run -p luna-integration --example resource_loading

step "Running strict Clippy"
cargo clippy --workspace --all-targets --all-features -- -D warnings

step "Running complete workspace tests"
cargo test --workspace --all-targets --all-features

step "Running deterministic structural qualification"
rm -rf "$qualification_dir"
mkdir -p "$qualification_dir"
cargo run --release -p luna-ui-rust-qualification -- \
    --output "$qualification_dir/report.json"

step "Checking rustdoc"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

if command -v cargo-semver-checks >/dev/null 2>&1; then
    step "Running advisory per-crate semver comparison against $baseline"
    repo_root="$(pwd -P)"
    semver_metadata="$(mktemp)"
    cargo metadata --no-deps --format-version 1 > "$semver_metadata"
    semver_failed=0
    while IFS=$'\t' read -r package manifest_path; do
        manifest_relative="$(realpath --relative-to="$repo_root" "$manifest_path")"
        if ! git cat-file -e "$baseline:$manifest_relative" 2>/dev/null; then
            printf 'Skipping new crate %s; it has no baseline API.\n' "$package"
            continue
        fi
        printf 'Checking %s against %s...\n' "$package" "$baseline"
        if ! cargo semver-checks --package "$package" --baseline-rev "$baseline"; then
            semver_failed=1
        fi
    done < <(
        python3 - "$semver_metadata" <<'PYTHON'
import json
import pathlib
import subprocess
import sys

metadata = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
public_crates = set(
    subprocess.check_output(
        [sys.executable, "scripts/check-public-api.py", "--list-crates"],
        text=True,
    ).splitlines()
)
for package in sorted(metadata["packages"], key=lambda item: item["name"]):
    if package["name"] in public_crates:
        print(f"{package['name']}\t{package['manifest_path']}")
PYTHON
    )
    rm -f "$semver_metadata"
    if (( semver_failed != 0 )); then
        printf 'Advisory semver comparison reported differences; review before release.\n' >&2
    fi
else
    printf '==> cargo-semver-checks unavailable; advisory semver comparison skipped.\n'
fi

step "Building Linux development package"
cargo build --release -p luna-ui-rust-editor-demo
rm -rf "$package_dir"
./scripts/package-linux.sh --skip-build --output "$package_dir"

cat > "$qualification_dir/README.txt" <<'CHECKLIST'
Luna-UI-Rust M7 runtime acceptance

1. Follow docs/EDITOR_DEMO_COMMANDS.md on the CPU editor.
2. Repeat the same command, mouse, document, pane, workspace, IME, and accessibility pass through
   Vulkan/wgpu. Geometry and behavior must match the CPU oracle.
3. Run CPU and Vulkan/wgpu proof galleries and cycle all five themes.
4. Leave the editor active through an extended workspace/editing session and inspect GPU retained
   resource diagnostics. Capacities may grow geometrically but must stay within policy and trim after
   a memory warning or device reconstruction.
5. Unpack the Linux development tarball and launch its binary. Confirm the Desktop Entry and bundled
   editor command reference are present.
6. On macOS, run scripts/test-macos.sh and the real-hardware checklist. Advisory CI alone does not
   promote macOS to a blocking release lane.
CHECKLIST

step "M7 automated gate complete"
printf 'Qualification report: %s\n' "$qualification_dir/report.json"
printf 'Runtime checklist: %s\n' "$qualification_dir/README.txt"
printf 'Linux package: %s\n' "$package_dir/Luna-UI-Rust-EditorDemo.tar.gz"
