#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

milestone="${1:-m8-working}"
output_root="${LUNA_EVIDENCE_DIR:-release/evidence/$milestone}"
qualification_dir="${TMPDIR:-/tmp}/luna-m8-evidence-qualification"
package_dir="${TMPDIR:-/tmp}/luna-m8-evidence-package"

rm -rf "$qualification_dir" "$package_dir"
mkdir -p "$qualification_dir" "$package_dir" "$output_root"

cargo metadata --format-version 1 --no-deps > "$output_root/cargo-metadata.json"
cargo run --release -p luna-ui-rust-qualification -- \
    --output "$qualification_dir/qualification.json"
cp "$qualification_dir/qualification.json" "$output_root/qualification.json"

cargo build --release -p luna-ui-rust-editor-demo
./scripts/package-linux.sh --skip-build --output "$package_dir"
cp "$package_dir/Luna-UI-Rust-EditorDemo.tar.gz" "$output_root/"

(
    cd "$output_root"
    sha256sum \
        qualification.json \
        cargo-metadata.json \
        Luna-UI-Rust-EditorDemo.tar.gz \
        > SHA256SUMS
)

{
    printf '# Luna-UI-Rust release evidence\n\n'
    printf -- '- Milestone: `%s`\n' "$milestone"
    printf -- '- Commit: `%s`\n' "$(git rev-parse HEAD)"
    printf -- '- Branch: `%s`\n' "$(git branch --show-current)"
    printf -- '- Rust: `%s`\n' "$(rustc --version)"
    printf -- '- Cargo: `%s`\n' "$(cargo --version)"
    printf -- '- Kernel: `%s`\n' "$(uname -srmo)"
    printf -- '- Generated UTC: `%s`\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    printf '\n## Manual acceptance\n\n'
    printf -- '- [ ] CPU editor operator pass\n'
    printf -- '- [ ] wgpu editor operator pass\n'
    printf -- '- [ ] CPU proof-gallery five-theme pass\n'
    printf -- '- [ ] wgpu proof-gallery five-theme pass\n'
    printf -- '- [ ] extracted package launch pass\n'
    printf -- '- [ ] accessibility observations recorded\n'
    printf -- '- [ ] IME observations recorded\n'
} > "$output_root/ENVIRONMENT.md"

cp api/public-api.toml "$output_root/public-api.toml"
cp docs/EDITOR_DEMO_COMMANDS.md "$output_root/EDITOR_DEMO_COMMANDS.md"

printf 'Release evidence written to %s\n' "$output_root"
