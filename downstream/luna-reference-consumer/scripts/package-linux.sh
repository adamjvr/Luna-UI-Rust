#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# Build a relocatable Linux ZIP for the external M8.2 reference consumer.
# This child script never closes the terminal that launches it.

set -Eeuo pipefail

on_exit() {
    status=$?
    if [[ "$status" -ne 0 ]]; then
        printf '[FAIL] M8.2 Linux packaging stopped with status %s.\n' "$status" >&2
    fi
    printf '[SAFE] Your terminal remains open.\n'
}
trap on_exit EXIT

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
consumer_root="$(cd -- "$script_dir/.." && pwd)"
repo_root="$(cd -- "$consumer_root/../.." && pwd)"
manifest="$consumer_root/Cargo.toml"

profile="release"
output_dir="$repo_root/dist/m8.2"
skip_build=false
application_id="org.lunaui.ReferenceConsumer"
bundle_name="Luna-Reference-Consumer"

usage() {
    cat <<'USAGE'
Usage: downstream/luna-reference-consumer/scripts/package-linux.sh [options]

Options:
  --debug          Package a debug executable instead of release.
  --output DIR     Write the bundle and ZIP below DIR.
  --skip-build     Reuse the already-built executable.
  -h, --help       Show this help.
USAGE
}

while (($#)); do
    case "$1" in
        --debug)
            profile="debug"
            shift
            ;;
        --output)
            output_dir="${2:?--output requires a directory}"
            shift 2
            ;;
        --skip-build)
            skip_build=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            printf 'Unknown argument: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [[ "$(uname -s)" != "Linux" ]]; then
    printf 'The M8.2 Linux package must be built on Linux.\n' >&2
    exit 2
fi

for command_name in cargo rustc git install zip sha256sum find touch; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
        printf 'Required command is missing: %s\n' "$command_name" >&2
        exit 1
    fi
done

target_dir="$consumer_root/target"
if [[ "$skip_build" == false ]]; then
    if [[ "$profile" == "release" ]]; then
        CARGO_TARGET_DIR="$target_dir" cargo build \
            --manifest-path "$manifest" \
            --release
    else
        CARGO_TARGET_DIR="$target_dir" cargo build \
            --manifest-path "$manifest"
    fi
fi

binary="$target_dir/$profile/luna-reference-consumer"
if [[ ! -x "$binary" ]]; then
    printf 'Expected executable is missing: %s\n' "$binary" >&2
    exit 1
fi

architecture="$(uname -m)"
bundle="$output_dir/$bundle_name"
archive="$output_dir/${bundle_name}-linux-${architecture}.zip"
checksum="${archive}.sha256"
source_date_epoch="${SOURCE_DATE_EPOCH:-$(git -C "$repo_root" log -1 --format=%ct)}"
source_tree_dirty=false
if [[ -n "$(git -C "$repo_root" status --porcelain=v1 --untracked-files=all)" ]]; then
    source_tree_dirty=true
fi

rm -rf -- "$bundle" "$archive" "$checksum"
install -d \
    "$bundle/bin" \
    "$bundle/share/$application_id" \
    "$bundle/share/doc/$bundle_name"
install -m 0755 "$binary" "$bundle/bin/luna-reference-consumer"
install -m 0644 \
    "$consumer_root/resources/welcome.txt" \
    "$bundle/share/$application_id/welcome.txt"
install -m 0644 \
    "$consumer_root/README.md" \
    "$repo_root/LICENSE" \
    "$repo_root/docs/M8_2_DOWNSTREAM_CONSUMER.md" \
    "$bundle/share/doc/$bundle_name/"

cat > "$bundle/MANIFEST.txt" <<MANIFEST
application_id=$application_id
profile=$profile
source_commit=$(git -C "$repo_root" rev-parse HEAD)
source_date_epoch=$source_date_epoch
source_tree_dirty=$source_tree_dirty
architecture=$architecture
rustc=$(rustc --version)
MANIFEST

cat > "$bundle/README.txt" <<'NOTICE'
Luna Reference Consumer — M8.2 downstream proof

Run from any working directory:
  /absolute/path/to/Luna-Reference-Consumer/bin/luna-reference-consumer --backend cpu

Run through the GPU backend:
  /absolute/path/to/Luna-Reference-Consumer/bin/luna-reference-consumer --backend wgpu

Run the deterministic extracted-package proof:
  /absolute/path/to/Luna-Reference-Consumer/bin/luna-reference-consumer --self-test --workspace /path/to/workspace

The executable discovers its resource below:
  share/org.lunaui.ReferenceConsumer/welcome.txt

This is a product-neutral development qualification package, not a stable product release.
NOTICE

mkdir -p -- "$output_dir"
find "$bundle" -exec touch -h -d "@$source_date_epoch" {} +
(
    cd -- "$output_dir"
    zip -X -q -r "$(basename -- "$archive")" "$bundle_name"
    sha256sum "$(basename -- "$archive")" > "$(basename -- "$checksum")"
)

printf 'Created bundle:   %s\n' "$bundle"
printf 'Created ZIP:      %s\n' "$archive"
printf 'Created checksum: %s\n' "$checksum"
