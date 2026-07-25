#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURES="${TMPDIR:-/tmp}/luna-m3-2c-runtime"

cd "$ROOT"

cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

rm -rf "$FIXTURES"
mkdir -p "$FIXTURES"
printf 'baseline\n' > "$FIXTURES/observed.txt"
printf 'recent one\n' > "$FIXTURES/recent-one.txt"
printf 'recent two\n' > "$FIXTURES/recent-two.txt"
printf '\xff\xfe\x00\x80' > "$FIXTURES/invalid-utf8.txt"

cat <<INSTRUCTIONS

M3.2c automated validation passed.

Runtime fixtures: $FIXTURES

Manual editor routine:
  1. Run: cargo run --release -p luna-ui-rust-editor-demo
  2. Open recent-one.txt and recent-two.txt, then verify File shows both in MRU order.
  3. Reopen recent-one.txt and verify its existing tab activates and it moves to MRU position one.
  4. Open observed.txt. In another terminal run:
       printf 'in-place change\n' > '$FIXTURES/observed.txt'
     Wait about one second and verify the status reports Changed on disk.
  5. Activate File -> Reload from Disk and verify the editor adopts the new text.
  6. Replace the file atomically:
       printf 'replacement\n' > '$FIXTURES/replacement.tmp'
       mv '$FIXTURES/replacement.tmp' '$FIXTURES/observed.txt'
     Verify the status reports Replaced on disk.
  7. Delete observed.txt and verify Deleted on disk, then recreate it and verify Recreated on disk.
  8. Open invalid-utf8.txt and verify no tab is created.
  9. Confirm File/Edit/Find/View/Help remain dropdowns and Ctrl+P remains the separate palette.

INSTRUCTIONS
