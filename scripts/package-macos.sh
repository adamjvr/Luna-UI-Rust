#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# Build a signed Apple-Silicon development .app for one Luna editor backend.

set -Eeuo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"

profile="release"
output_dir="$repo_root/dist/m8.4/macos-package"
bundle_id="org.lunaui.rust.editor-demo"
display_name="Luna UI Rust Editor Demo"
identity="-"
backend="cpu"
skip_build=false

usage() {
    cat <<'USAGE'
Usage: scripts/package-macos.sh [options]

Options:
  --debug                 Package target/debug instead of target/release.
  --output DIR            Place the .app, ZIP, checksum, and manifest below DIR.
  --bundle-id ID          Override the reverse-DNS bundle identifier.
  --display-name NAME     Override the Finder-visible application name.
  --identity IDENTITY     codesign identity; default '-' performs ad-hoc signing.
  --backend cpu|wgpu      Embed the CPU or Metal/wgpu launch policy.
  --skip-build            Reuse an already-built editor-demo executable.
  -h, --help              Show this help.
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
        --bundle-id)
            bundle_id="${2:?--bundle-id requires a value}"
            shift 2
            ;;
        --display-name)
            display_name="${2:?--display-name requires a value}"
            shift 2
            ;;
        --identity)
            identity="${2:?--identity requires a value}"
            shift 2
            ;;
        --backend)
            backend="${2:?--backend requires cpu or wgpu}"
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

if [[ "$(uname -s)" != "Darwin" ]]; then
    printf 'macOS application packaging must run on macOS.\n' >&2
    exit 2
fi
if [[ "$(uname -m)" != "arm64" ]]; then
    printf 'M8.4 packaging requires Apple-Silicon arm64 hardware.\n' >&2
    exit 2
fi
if [[ "$backend" != "cpu" && "$backend" != "wgpu" ]]; then
    printf 'Unsupported backend %q; expected cpu or wgpu.\n' "$backend" >&2
    exit 2
fi

for command_name in awk cargo codesign ditto file git grep install lipo plutil python3 rustc shasum tr; do
    command -v "$command_name" >/dev/null 2>&1 || {
        printf 'Required command is missing: %s\n' "$command_name" >&2
        exit 1
    }
done

cd -- "$repo_root"

if [[ "$skip_build" == false ]]; then
    if [[ "$profile" == "release" ]]; then
        cargo build --release -p luna-ui-rust-editor-demo
    else
        cargo build -p luna-ui-rust-editor-demo
    fi
fi

binary="$repo_root/target/$profile/luna-ui-rust-editor-demo"
if [[ ! -x "$binary" ]]; then
    printf 'Expected executable is missing: %s\n' "$binary" >&2
    exit 1
fi
if ! lipo -archs "$binary" | tr ' ' '\n' | grep -qx arm64; then
    printf 'Expected an arm64 executable: %s\n' "$binary" >&2
    exit 1
fi

version="$(awk -F'"' '/^version = / { print $2; exit }' Cargo.toml)"
[[ -n "$version" ]] || version="0.1.0"
build_version="${LUNA_BUILD_VERSION:-$(git rev-list --count HEAD 2>/dev/null || printf '1')}"
source_commit="$(git rev-parse HEAD)"
source_tree_dirty=false
if [[ -n "$(git status --porcelain=v1 --untracked-files=all)" ]]; then
    source_tree_dirty=true
fi

executable="LunaUIRustEditorDemo"
bundle="$output_dir/$display_name.app"
contents="$bundle/Contents"
launcher="$contents/MacOS/$executable"
resource_binary="$contents/Resources/bin/luna-ui-rust-editor-demo"
archive="$output_dir/$display_name-macos-arm64.zip"
checksum="$archive.sha256"
manifest="$contents/Resources/M8_4_BUNDLE_MANIFEST.txt"

rm -rf -- "$bundle" "$archive" "$checksum"
install -d "$contents/MacOS" "$contents/Resources/bin"
install -m 0755 "$binary" "$resource_binary"
install -m 0644 docs/EDITOR_DEMO_COMMANDS.md "$contents/Resources/EDITOR_DEMO_COMMANDS.md"
install -m 0644 docs/M8_4_MACOS_EVIDENCE.md "$contents/Resources/M8_4_MACOS_EVIDENCE.md"

python3 - \
    resources/macos/Info.plist.in \
    "$contents/Info.plist" \
    "$display_name" \
    "$executable" \
    "$bundle_id" \
    "$version" \
    "$build_version" <<'PY'
import html
import pathlib
import re
import sys

template_path, output_path, display_name, executable, bundle_id, version, build_version = sys.argv[1:]
text = pathlib.Path(template_path).read_text(encoding="utf-8")
values = {
    "@DISPLAY_NAME@": display_name,
    "@EXECUTABLE@": executable,
    "@BUNDLE_ID@": bundle_id,
    "@VERSION@": version,
    "@BUILD_VERSION@": build_version,
}
for token, value in values.items():
    if token not in text:
        raise SystemExit(f"Info.plist template is missing required token {token}")
    text = text.replace(token, html.escape(value, quote=False))

unresolved = sorted(set(re.findall(r"@[A-Z0-9_]+@", text)))
if unresolved:
    raise SystemExit(
        "Info.plist template contains unresolved tokens: " + ", ".join(unresolved)
    )

pathlib.Path(output_path).write_text(text, encoding="utf-8")
PY

cat > "$launcher" <<LAUNCHER
#!/bin/sh
set -eu
contents_dir="\$(CDPATH= cd -- "\$(dirname -- "\$0")/.." && pwd)"
export LUNA_RENDER_BACKEND="$backend"
LAUNCHER
if [[ "$backend" == "wgpu" ]]; then
    cat >> "$launcher" <<'LAUNCHER'
export WGPU_BACKEND="metal"
LAUNCHER
fi
cat >> "$launcher" <<'LAUNCHER'
exec "$contents_dir/Resources/bin/luna-ui-rust-editor-demo" "$@"
LAUNCHER
chmod 0755 "$launcher"

cat > "$manifest" <<MANIFEST
schema=luna-m8.4-macos-bundle-v1
backend=$backend
profile=$profile
bundle_id=$bundle_id
display_name=$display_name
source_commit=$source_commit
source_tree_dirty=$source_tree_dirty
architecture=arm64
rustc=$(rustc --version)
MANIFEST

cat > "$contents/Resources/Luna-UI-Rust.txt" <<NOTICE
Luna UI Rust M8.4 Apple-Silicon qualification bundle.
Embedded backend: $backend
This is a product-neutral development proof, not a stable end-user release.
See M8_4_MACOS_EVIDENCE.md and EDITOR_DEMO_COMMANDS.md in this Resources directory.
NOTICE

plutil -lint "$contents/Info.plist"
codesign --force --timestamp=none --sign "$identity" "$resource_binary"
codesign --force --deep --timestamp=none --sign "$identity" "$bundle"
codesign --verify --deep --strict --verbose=2 "$bundle"
file "$resource_binary"
lipo -archs "$resource_binary"

mkdir -p -- "$output_dir"
ditto -c -k --sequesterRsrc --keepParent "$bundle" "$archive"
(
    cd -- "$output_dir"
    shasum -a 256 "$(basename -- "$archive")" > "$(basename -- "$checksum")"
)

printf 'Created bundle:   %s\n' "$bundle"
printf 'Created ZIP:      %s\n' "$archive"
printf 'Created checksum: %s\n' "$checksum"
printf 'Embedded backend: %s\n' "$backend"
