#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURES="${TMPDIR:-/tmp}/luna-m3-2e-workspace"
SESSION_HOME="$FIXTURES/.state"
SESSION_ROOT="$SESSION_HOME/luna-ui-rust"

cd "$ROOT"

cargo fmt --all
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

rm -rf "$FIXTURES"
mkdir -p "$FIXTURES/src/nested" "$FIXTURES/docs"
printf 'fn main() { println!("workspace operations"); }\n' > "$FIXTURES/src/main.rs"
printf 'pub fn library() -> &\x27static str { "luna" }\n' > "$FIXTURES/src/lib.rs"
printf 'nested fixture\n' > "$FIXTURES/src/nested/deep.txt"
printf '# Workspace Operations\n' > "$FIXTURES/docs/guide.md"
printf 'replace me\n' > "$FIXTURES/collision.txt"
printf 'dirty delete baseline\n' > "$FIXTURES/dirty-delete.txt"
printf 'rename baseline\n' > "$FIXTURES/rename-open.txt"

cat <<INSTRUCTIONS

M3.2e automated validation passed.

Workspace fixture: $FIXTURES
Persistent session file: $SESSION_ROOT/editor-session-v1.txt

Manual editor routine:
  1. Run with isolated persistent state:
       XDG_STATE_HOME='$SESSION_HOME' cargo run --release -p luna-ui-rust-editor-demo
  2. Use File -> Open Folder and select: $FIXTURES
  3. Select the workspace root and choose File -> New File in Workspace.
     Create created.txt and verify the row appears and is selected.
  4. Create a folder named created-folder and verify it appears before regular files.
  5. Select created.txt, choose Rename Workspace Entry, and rename it renamed.txt.
     Verify the row identity changes and the old row disappears.
  6. Select collision.txt, rename renamed.txt to collision.txt, and verify replacement requires
     explicit confirmation. Cancel first, then repeat and confirm replacement.
  7. Open rename-open.txt, edit it without saving, select it in the workspace, and rename it.
     Verify the tab follows the new path and remains dirty.
  8. Open dirty-delete.txt, edit it without saving, select it, and choose Delete Workspace Entry.
     Choose Keep Open. Verify storage is deleted and the dirty buffer becomes an Untitled tab.
  9. Repeat a dirty deletion with Discard & Close and verify the tab closes only after deletion.
 10. Repeat with Cancel and verify neither storage nor the editor buffer changes.
 11. Delete created-folder and verify recursive deletion requires confirmation.
 12. Open two recent files, expand src/nested, select src/main.rs, then close Luna normally.
 13. Relaunch Luna. Verify the workspace root, expansion, selection, and recent-file menu restore.
 14. Close Workspace, relaunch, and verify the closed workspace is not restored.
 15. Verify File/Edit/Find/View/Help remain dropdowns and Ctrl+P remains independent.
 16. Run the proof-gallery regression:
       cargo run --release -p luna-ui-rust-proof-gallery

Inspect the persistent session file when needed:
  sed -n '1,120p' '$SESSION_ROOT/editor-session-v1.txt'

Cleanup after runtime testing:
  rm -rf '$FIXTURES'

INSTRUCTIONS
