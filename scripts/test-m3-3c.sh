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

printf '%s\n' '==> Running strict Clippy'
cargo clippy --workspace --all-targets --all-features -- -D warnings

printf '%s\n' '==> Running all tests'
cargo test --workspace --all-targets --all-features

printf '%s\n' '==> Building rustdoc with warnings denied'
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

FIXTURE_ROOT="/tmp/luna-m3-3c-workspace"
printf '%s\n' "==> Creating runtime fixture at $FIXTURE_ROOT"
rm -rf "$FIXTURE_ROOT"
mkdir -p "$FIXTURE_ROOT/tabs" "$FIXTURE_ROOT/src/nested" "$FIXTURE_ROOT/docs" \
    "$FIXTURE_ROOT/watch/subtree" "$FIXTURE_ROOT/.state"

cat > "$FIXTURE_ROOT/README.md" <<'DOC'
# Luna M3.3c Runtime Fixture

Use this workspace to verify recursive pane/session restoration, asynchronous completion, deep menus,
search history/options, scrollbar paging, native watcher delivery, polling fallback, and incremental
subtree refresh.
DOC

cat > "$FIXTURE_ROOT/src/completion.rs" <<'DOC'
fn completion_demo() {
    let topology = Pan;
    let first = "alpha Alpha alphabet alpha_ alpha";
    let second = "cat Cat concatenate cat";
    println!("{topology:?} {first} {second}");
}
DOC

cat > "$FIXTURE_ROOT/src/nested/session.rs" <<'DOC'
pub struct SessionRuntime {
    pub pane_count: usize,
    pub active_view: u64,
}
DOC

cat > "$FIXTURE_ROOT/docs/find.md" <<'DOC'
Luna luna LUNAR luna_ui luna
cat Cat concatenate cat
alpha beta alpha gamma alpha
DOC

: > "$FIXTURE_ROOT/docs/long-scroll.txt"
for index in $(seq -w 1 320); do
    printf 'scrollbar validation line %s — Luna UI Rust M3.3c\n' "$index" \
        >> "$FIXTURE_ROOT/docs/long-scroll.txt"
done

for index in $(seq -w 1 18); do
    printf 'pub const TAB_%s: &str = "tab-%s";\n' "$index" "$index" \
        > "$FIXTURE_ROOT/tabs/tab-$index.rs"
done

printf '%s\n' '==> M3.3c automated gate passed'
printf '%s\n' "Runtime workspace: $FIXTURE_ROOT"
printf '%s\n' "Run with: XDG_STATE_HOME=$FIXTURE_ROOT/.state cargo run --release -p luna-ui-rust-editor-demo"
printf '%s\n' 'For polling fallback, temporarily run with inotifywait absent from PATH.'
