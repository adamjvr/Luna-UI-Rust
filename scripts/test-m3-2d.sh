#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURES="${TMPDIR:-/tmp}/luna-m3-2d-workspace"

cd "$ROOT"

cargo fmt --all
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

if [[ -e "$FIXTURES/locked" ]]; then
    chmod u+rwx "$FIXTURES/locked" 2>/dev/null || true
fi
rm -rf "$FIXTURES"
mkdir -p "$FIXTURES/src/nested" "$FIXTURES/docs" "$FIXTURES/locked"
printf 'fn main() { println!("workspace"); }\n' > "$FIXTURES/src/main.rs"
printf 'pub fn library() -> &\x27static str { "luna" }\n' > "$FIXTURES/src/lib.rs"
printf 'nested fixture\n' > "$FIXTURES/src/nested/deep.txt"
printf '# Workspace Guide\n' > "$FIXTURES/docs/guide.md"
printf 'hidden fixture\n' > "$FIXTURES/.secret.txt"
printf 'permission fixture\n' > "$FIXTURES/locked/private.txt"
printf '\xff\xfe\x00\x80' > "$FIXTURES/invalid-utf8.txt"
ln -s "src" "$FIXTURES/linked-src"
chmod 000 "$FIXTURES/locked" 2>/dev/null || true

cat <<INSTRUCTIONS

M3.2d automated validation passed.

Workspace fixture: $FIXTURES

Manual editor routine:
  1. Run: cargo run --release -p luna-ui-rust-editor-demo
  2. Use File -> Open Folder and select: $FIXTURES
  3. Verify the sidebar shows the workspace root with folders before files.
  4. Expand src, nested, and docs. Collapse and reopen them.
  5. Click src/main.rs and verify it opens. Click it again and verify no duplicate tab appears.
  6. Verify .secret.txt is hidden and linked-src is shown as a non-followed link leaf.
  7. Verify locked is projected as permission denied when the current user cannot read it.
  8. In another terminal add a file:
       printf 'new file\n' > '$FIXTURES/src/new-file.txt'
     Wait about one second and verify the row appears while expansion stays intact.
  9. Rename it:
       mv '$FIXTURES/src/new-file.txt' '$FIXTURES/src/renamed-file.txt'
     Verify the old row disappears and the new row appears.
 10. Delete it:
       rm '$FIXTURES/src/renamed-file.txt'
     Verify the row disappears without closing unrelated tabs.
 11. Use File -> Refresh Workspace and verify an unchanged refresh reports that it is current.
 12. Use File -> Close Workspace and verify the sidebar returns to Open Documents.
 13. Confirm File/Edit/Find/View/Help remain dropdowns and Ctrl+P remains independent.
 14. Run the proof-gallery regression:
       cargo run --release -p luna-ui-rust-proof-gallery

Cleanup after runtime testing:
  chmod u+rwx '$FIXTURES/locked' 2>/dev/null || true
  rm -rf '$FIXTURES'

INSTRUCTIONS
