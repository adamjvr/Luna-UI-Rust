#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Focused M8.3.1 native dirty-close response repair qualification.
set -Eeuo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
on_exit(){ status=$?; [[ $status -eq 0 ]] || printf '[FAIL] M8.3.1 qualification stopped with status %s.\n' "$status" >&2; printf '[SAFE] Your terminal remains open.\n'; }
trap on_exit EXIT
step(){ printf '\n============================================================\n==> %s\n============================================================\n' "$1"; }
cd -- "$repo_root"
for command_name in cargo python3; do command -v "$command_name" >/dev/null 2>&1 || { printf '[FAIL] Missing command: %s\n' "$command_name" >&2; exit 1; }; done
step "Verifying the M8.3.1 dialog repair source"
grep -qF 'mod dialog_response;' crates/luna-document-services/src/lib.rs
grep -qF 'zenity_extra_button_selected' crates/luna-document-services/src/lib.rs
grep -qF 'extra_button_on_stderr_is_recognized' crates/luna-document-services/src/dialog_response.rs
step "Formatting and checking the focused repair"
cargo fmt --all
cargo fmt --all -- --check
cargo check -p luna-document-services -p luna-ui-rust-editor-demo --all-targets --all-features
cargo clippy -p luna-document-services -p luna-ui-rust-editor-demo --all-targets --all-features -- -D warnings
step "Running dialog and editor lifecycle regression tests"
cargo test -p luna-document-services
cargo test -p luna-ui-rust-editor-demo dirty_close
cargo test -p luna-ui-rust-editor-demo clipboard_menu_enables_selection_commands_and_pastes_as_one_transaction
step "Checking documentation and source whitespace"
python3 scripts/check-public-api.py
git diff --check
step "M8.3.1 automated repair qualification passed"
printf '[PASS] native three-choice dialog parsing and dirty-close lifecycle tests passed\n'
