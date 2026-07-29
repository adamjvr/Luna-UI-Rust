#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Compatibility entry point for the current macOS advisory lane.
set -Eeuo pipefail
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec "$script_dir/test-m8-4-macos.sh" "$@"
