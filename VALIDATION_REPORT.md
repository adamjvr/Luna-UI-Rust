# M3.2b Validation Report

## Verified input baseline

The project owner locally validated and committed M3.2a. The accepted baseline includes stable
document IDs, monotonic untitled naming, canonical file identity, duplicate prevention, dirty-state
evaluation, and protected close requirements.

## M3.2b change set

- `crates/luna-document-services`
  - product-neutral `TextFileService` and `DocumentDialogService` traits;
  - strict UTF-8 load results;
  - canonical existing/new-destination identities;
  - deterministic content revisions;
  - Any/Missing/Matches write preconditions;
  - same-directory temporary writes, existing-permission preservation, and atomic replacement;
  - typed I/O, path, encoding, and conflict failures;
  - Zenity/KDialog native Linux dialog adapter;
  - in-memory file and scripted dialog adapters;
  - service and conflict tests.
- `apps/luna-ui-rust-editor-demo`
  - injected file/dialog services;
  - enabled Open and Save As menu/palette commands;
  - Control-O and Control-Shift-S routing;
  - strict UTF-8 Open and duplicate-open activation;
  - Save As writing plus file-identity assignment;
  - optimistic ordinary Save;
  - Save/Discard/Cancel dirty close;
  - Overwrite/Reload/Cancel conflict resolution;
  - retained-text invalidation after reload;
  - integration tests using deterministic adapters.
- workspace and documentation
  - new workspace member and editor dependency;
  - lockfile entry;
  - architecture, roadmap, porting map, parity, current-status, README, and milestone docs.

## Structural checks performed in the generation environment

- all 24 TOML files parse;
- the workspace contains 20 members after adding `luna-document-services`;
- all local path dependencies resolve to present crate directories;
- all 40 Rust files retain MPL-2.0 SPDX identifiers;
- the source tree contains 17,119 Rust lines and 128 declared tests;
- delimiter and lexical scans pass across all Rust files;
- no unsafe blocks or declarations were introduced;
- no `.unwrap()`, `.expect()`, `panic!`, `todo!`, or `unimplemented!` calls were introduced;
- Cargo.lock contains the new local package and editor dependency;
- invalid UTF-8 cannot be loaded through either real or memory services;
- an optimistic revision mismatch cannot silently overwrite storage;
- Save As checks duplicate open identity before writing;
- canceled dialogs preserve content, dirty state, and registry identity;
- conflict Reload resets the text-layout cache and saved baseline;
- dirty-close Save closes only after a successful write;
- dirty-close Discard removes without writing;
- dirty-close Cancel leaves the document open;
- Open, Save, and Save As remain projected through the shared command catalog.

## Toolchain requirement

The generation environment does not contain Rust tooling. Run the complete local quality gate:

```bash
cargo fmt --all
cargo check --workspace --all-targets
./scripts/validate.sh
cargo run --release -p luna-ui-rust-editor-demo
```

The native Linux runtime expects either `zenity` or `kdialog`. When neither helper is available, the
editor reports the missing helper without changing document state.

## Runtime acceptance

```text
Control-O opens a native file chooser:
Selected UTF-8 file opens in a new tab:
Opening the same canonical file activates the existing tab:
Invalid UTF-8 reports an error and opens no tab:
Control-S saves an existing file and clears dirty state:
Control-S on untitled/virtual content opens Save As:
Control-Shift-S opens Save As directly:
Save As updates the tab title and file identity:
Dirty close Save writes before closing:
Dirty close Discard closes without writing:
Dirty close Cancel leaves the tab open:
External content change produces Overwrite/Reload/Cancel:
Reload replaces editor content and clears dirty state:
Canceled or failed operations preserve editor content:
File/Edit/Find/View/Help dropdown behavior remains correct:
Control-P command palette behavior remains correct:
M3.1b text-cache behavior remains correct:
M3.1c proof-gallery behavior remains correct:
```

Any compiler, strict-Clippy, test, rustdoc, file-integrity, lifecycle, menu, text-cache,
accessibility, or native runtime regression blocks M3.2c.
