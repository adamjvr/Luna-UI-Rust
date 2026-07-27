#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

profile="release"
output_dir="${PWD}/dist/linux"
prefix="Luna-UI-Rust-EditorDemo"
application_id="org.lunaui.EditorDemo"
skip_build=false

usage() {
    cat <<'USAGE'
Usage: scripts/package-linux.sh [options]

Options:
  --debug          Package target/debug instead of target/release.
  --output DIR     Place the unpacked bundle and tarball below DIR.
  --skip-build     Reuse an already-built editor-demo executable.
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
    printf 'Linux development packaging must run on Linux.\n' >&2
    exit 2
fi

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

bundle="${output_dir}/${prefix}"
tarball="${output_dir}/${prefix}.tar.gz"
checksum="${tarball}.sha256"
source_date_epoch="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct 2>/dev/null || printf '0')}"
rm -rf "$bundle" "$tarball" "$checksum"
install -d \
    "${bundle}/bin" \
    "${bundle}/share/applications" \
    "${bundle}/share/metainfo" \
    "${bundle}/share/${application_id}" \
    "${bundle}/share/doc/${prefix}"
install -m 0755 "$binary" "${bundle}/bin/luna-ui-rust-editor-demo"
install -m 0644 resources/linux/org.lunaui.EditorDemo.desktop \
    "${bundle}/share/applications/org.lunaui.EditorDemo.desktop"
install -m 0644 resources/linux/org.lunaui.EditorDemo.metainfo.xml \
    "${bundle}/share/metainfo/org.lunaui.EditorDemo.metainfo.xml"
install -m 0644 README.md LICENSE \
    "${bundle}/share/doc/${prefix}/"
install -m 0644 docs/EDITOR_DEMO_COMMANDS.md \
    "${bundle}/share/${application_id}/EDITOR_DEMO_COMMANDS.md"

cat > "${bundle}/MANIFEST.txt" <<NOTICE
application_id=${application_id}
profile=${profile}
source_commit=$(git rev-parse HEAD 2>/dev/null || printf 'unknown')
source_date_epoch=${source_date_epoch}
rustc=$(rustc --version)
NOTICE

cat > "${bundle}/README.txt" <<'NOTICE'
Luna UI Rust Editor Demo development bundle

Run from the unpacked directory:
  PATH="$PWD/bin:$PATH" ./bin/luna-ui-rust-editor-demo

Install for one user by copying:
  bin/luna-ui-rust-editor-demo        -> ~/.local/bin/
  share/applications/*.desktop        -> ~/.local/share/applications/
  share/metainfo/*.metainfo.xml       -> ~/.local/share/metainfo/
  share/org.lunaui.EditorDemo/        -> ~/.local/share/org.lunaui.EditorDemo/

This is a development qualification package, not a stable end-user release.
NOTICE

if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "${bundle}/share/applications/org.lunaui.EditorDemo.desktop"
else
    printf 'desktop-file-validate unavailable; desktop-entry validation skipped.\n'
fi

if command -v appstreamcli >/dev/null 2>&1; then
    appstreamcli validate --no-net "${bundle}/share/metainfo/org.lunaui.EditorDemo.metainfo.xml"
else
    printf 'appstreamcli unavailable; AppStream validation skipped.\n'
fi

mkdir -p "$output_dir"
find "$bundle" -exec touch -h -d "@${source_date_epoch}" {} +
tar \
    --sort=name \
    --mtime="@${source_date_epoch}" \
    --owner=0 \
    --group=0 \
    --numeric-owner \
    -C "$output_dir" \
    -czf "$tarball" \
    "$prefix"
(
    cd "$output_dir"
    sha256sum "$(basename "$tarball")" > "$(basename "$checksum")"
)
printf 'Created %s\nCreated %s\nCreated %s\n' "$bundle" "$tarball" "$checksum"
