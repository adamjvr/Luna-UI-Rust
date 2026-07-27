#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
    printf 'macOS application packaging must run on macOS.\n' >&2
    exit 2
fi

profile="release"
output_dir="${PWD}/dist/macos"
bundle_id="org.lunaui.rust.editor-demo"
display_name="Luna UI Rust Editor Demo"
identity="-"
skip_build=false

usage() {
    cat <<'USAGE'
Usage: scripts/package-macos.sh [options]

Options:
  --debug                 Package target/debug instead of target/release.
  --output DIR            Place the .app bundle below DIR.
  --bundle-id ID          Override the reverse-DNS bundle identifier.
  --display-name NAME     Override the Finder-visible application name.
  --identity IDENTITY     codesign identity; default '-' performs ad-hoc signing.
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

if [[ "$skip_build" == false ]]; then
    if [[ "$profile" == "release" ]]; then
        cargo build --release -p luna-ui-rust-editor-demo
    else
        cargo build -p luna-ui-rust-editor-demo
    fi
fi

binary="${PWD}/target/${profile}/luna-ui-rust-editor-demo"
if [[ ! -x "$binary" ]]; then
    printf 'Expected executable is missing: %s\n' "$binary" >&2
    exit 1
fi

version="$(awk -F'"' '/^version = / { print $2; exit }' Cargo.toml)"
if [[ -z "$version" ]]; then
    version="0.1.0"
fi
build_version="${LUNA_BUILD_VERSION:-$(git rev-list --count HEAD 2>/dev/null || printf '1')}"
executable="LunaUIRustEditorDemo"
bundle="${output_dir}/${display_name}.app"
contents="${bundle}/Contents"

escape_sed_replacement() {
    printf '%s' "$1" | sed 's/[&|\]/\\&/g'
}

display_name_sed="$(escape_sed_replacement "$display_name")"
executable_sed="$(escape_sed_replacement "$executable")"
bundle_id_sed="$(escape_sed_replacement "$bundle_id")"
version_sed="$(escape_sed_replacement "$version")"
build_version_sed="$(escape_sed_replacement "$build_version")"

rm -rf "$bundle"
mkdir -p "${contents}/MacOS" "${contents}/Resources"
cp "$binary" "${contents}/MacOS/${executable}"
chmod +x "${contents}/MacOS/${executable}"

install -m 0644 docs/EDITOR_DEMO_COMMANDS.md "${contents}/Resources/EDITOR_DEMO_COMMANDS.md"

sed \
    -e "s|@DISPLAY_NAME@|${display_name_sed}|g" \
    -e "s|@EXECUTABLE@|${executable_sed}|g" \
    -e "s|@BUNDLE_ID@|${bundle_id_sed}|g" \
    -e "s|@VERSION@|${version_sed}|g" \
    -e "s|@BUILD_VERSION@|${build_version_sed}|g" \
    resources/macos/Info.plist.in > "${contents}/Info.plist"

cat > "${contents}/Resources/Luna-UI-Rust.txt" <<'NOTICE'
Luna UI Rust editor integration proof.
Linux is the primary target; macOS is the supported secondary target.
The CPU renderer is the reference path. Set LUNA_RENDER_BACKEND=wgpu before launch to use Metal.
See EDITOR_DEMO_COMMANDS.md in this Resources directory for the complete operator guide.
NOTICE

plutil -lint "${contents}/Info.plist"
codesign --force --deep --sign "$identity" "$bundle"
codesign --verify --deep --strict --verbose=2 "$bundle"

printf 'Created %s\n' "$bundle"
