#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# Complete M8.2 external downstream-consumer qualification.
# This child script never closes the terminal that launches it.

set -Eeuo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
consumer_root="$repo_root/downstream/luna-reference-consumer"
manifest="$consumer_root/Cargo.toml"
temporary_root="$(mktemp -d -t luna-m8.2-XXXXXXXX)"

on_exit() {
    status=$?
    rm -rf -- "$temporary_root"
    if [[ "$status" -ne 0 ]]; then
        printf '[FAIL] M8.2 automated qualification stopped with status %s.\n' "$status" >&2
    fi
    printf '[SAFE] Your terminal remains open.\n'
}
trap on_exit EXIT

step() {
    printf '\n============================================================\n'
    printf '==> %s\n' "$1"
    printf '============================================================\n'
}

fail() {
    printf '[FAIL] %s\n' "$1" >&2
    return 1
}

for command_name in cargo rustc python3 unzip zip sha256sum; do
    command -v "$command_name" >/dev/null 2>&1 || fail "Required command is missing: $command_name"
done

cd -- "$repo_root"

step "Verifying the M8.2 candidate documentation"
grep -qF "M8.2 external downstream reference consumer" CHANGELOG.md
grep -qF "## M8.2 external downstream consumer candidate" docs/CURRENT_STATUS.md
grep -qF "## M8.2 external downstream consumer acceptance" docs/RELEASE_CHECKLIST.md
grep -qF "# M8.2 External Downstream Consumer Proof" docs/M8_2_DOWNSTREAM_CONSUMER.md

step "Verifying the external Cargo workspace boundary"
metadata_path="$temporary_root/consumer-metadata.json"
cargo metadata \
    --manifest-path "$manifest" \
    --format-version 1 \
    --no-deps \
    > "$metadata_path"
python3 - "$metadata_path" "$consumer_root" "$repo_root" <<'PY'
import json
import pathlib
import sys

metadata_path = pathlib.Path(sys.argv[1])
expected_consumer = pathlib.Path(sys.argv[2]).resolve()
repository = pathlib.Path(sys.argv[3]).resolve()
metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
workspace_root = pathlib.Path(metadata["workspace_root"]).resolve()
packages = metadata.get("packages", [])

if workspace_root != expected_consumer:
    raise SystemExit(
        f"consumer workspace root {workspace_root} does not equal {expected_consumer}"
    )
if workspace_root == repository:
    raise SystemExit("consumer was absorbed into the Luna repository workspace")
if [package.get("name") for package in packages] != ["luna-reference-consumer"]:
    raise SystemExit(f"unexpected external workspace packages: {packages!r}")

print(f"[ OK ] external workspace root: {workspace_root}")
print("[ OK ] the consumer is outside the main workspace dependency graph")
PY

step "Checking the accepted Luna workspace"
cargo fmt --all -- --check
python3 scripts/check-public-api.py
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

step "Building every Luna demo application in release mode"
cargo build --release \
    -p luna-ui-rust-demo \
    -p luna-ui-rust-native-demo \
    -p luna-ui-rust-text-demo \
    -p luna-ui-rust-proof-gallery \
    -p luna-ui-rust-editor-demo \
    -p luna-ui-rust-qualification

step "Formatting and checking the external reference consumer"
export CARGO_TARGET_DIR="$consumer_root/target"
cargo fmt --manifest-path "$manifest" --all
cargo fmt --manifest-path "$manifest" --all -- --check
cargo check --manifest-path "$manifest" --all-targets --all-features
cargo clippy --manifest-path "$manifest" --all-targets --all-features -- -D warnings
cargo test --manifest-path "$manifest" --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc \
    --manifest-path "$manifest" \
    --all-features \
    --no-deps
cargo build --manifest-path "$manifest" --release
[[ -f "$consumer_root/Cargo.lock" ]] || fail "The external consumer Cargo.lock was not generated."

step "Running the source-tree public-API self-test"
workspace="$temporary_root/workspace"
mkdir -p -- "$workspace/src"
printf 'fn main() { println!("m8.2"); }\n' > "$workspace/src/main.rs"
printf '# External consumer workspace\n' > "$workspace/README.md"
LUNA_RESOURCE_ROOT="$consumer_root/resources" \
XDG_STATE_HOME="$temporary_root/source-state" \
cargo run \
    --manifest-path "$manifest" \
    --release -- \
    --self-test \
    --workspace "$workspace" |
tee "$temporary_root/source-self-test.txt"
grep -q '^m8_2_self_test=passed$' "$temporary_root/source-self-test.txt"

step "Building and extracting the relocatable Linux ZIP"
package_output="$temporary_root/package"
"$consumer_root/scripts/package-linux.sh" \
    --output "$package_output" \
    --skip-build
archive="$(find "$package_output" -maxdepth 1 -type f -name 'Luna-Reference-Consumer-linux-*.zip' -print -quit)"
[[ -n "$archive" ]] || fail "The downstream package ZIP was not created."
(
    cd -- "$package_output"
    sha256sum -c "$(basename -- "$archive").sha256"
)
extract_root="$temporary_root/extracted"
unzip -q "$archive" -d "$extract_root"
extracted="$extract_root/Luna-Reference-Consumer"
[[ -x "$extracted/bin/luna-reference-consumer" ]] || fail "Extracted executable is missing."
[[ -f "$extracted/share/org.lunaui.ReferenceConsumer/welcome.txt" ]] || fail "Extracted resource is missing."

step "Launching the extracted package from an unrelated directory"
unrelated="$temporary_root/unrelated-working-directory"
mkdir -p -- "$unrelated" "$temporary_root/extracted-home"
(
    cd -- "$unrelated"
    HOME="$temporary_root/extracted-home" \
    XDG_STATE_HOME="$temporary_root/extracted-state" \
    "$extracted/bin/luna-reference-consumer" \
        --self-test \
        --workspace "$workspace"
) | tee "$temporary_root/extracted-self-test.txt"
grep -q '^m8_2_self_test=passed$' "$temporary_root/extracted-self-test.txt"

git diff --check

step "M8.2 automated downstream qualification passed"
printf 'Consumer manifest: %s\n' "$manifest"
printf 'Package ZIP:      %s\n' "$archive"
printf '[PASS] source and extracted-package self-tests passed\n'
