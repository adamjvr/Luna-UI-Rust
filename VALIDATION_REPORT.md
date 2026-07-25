# M3.2a Validation Report

## Verified input baseline

The project owner confirmed that the cumulative M3.1d repair plus compiler and Clippy hotfixes build
and run correctly. Native testing confirmed that all top-level headings open real dropdowns and that
the command palette remains a separate Control-P surface.

## M3.2a change set

- `crates/luna-documents`
  - stable document IDs;
  - canonical file identity contract;
  - storage revision and external-change state;
  - save and close requirements;
  - document registry, duplicate indexes, untitled numbering, Save As reassignment, and removal;
  - lifecycle and duplicate-prevention tests.
- `apps/luna-ui-rust-editor-demo`
  - registry-backed metadata and stable IDs;
  - monotonic clean untitled documents;
  - honest Save behavior without simulated writes;
  - dirty-close protection and status feedback;
  - open-document sidebar projection;
  - lifecycle integration tests.
- workspace and documentation
  - new member/dependency and lockfile entry;
  - architecture, roadmap, porting map, parity, current-status, and milestone documentation.

## Structural checks performed in the generation environment

- all 23 TOML files parse;
- the workspace contains 19 members after adding `luna-documents`;
- all local path dependencies resolve to present directories;
- all Rust files retain MPL-2.0 SPDX identifiers;
- delimiter and lexical scans pass across all Rust files;
- no unsafe blocks or declarations were introduced;
- no `.unwrap()`, `.expect()`, `panic!`, `todo!`, or `unimplemented!` calls were introduced;
- Cargo.lock contains the new local package and editor dependency;
- the editor no longer owns duplicate title, saved-revision, or untitled-sequence fields;
- file identity requires an absolute path with no current/parent components;
- duplicate file registration returns the existing document;
- Save As cannot steal a file identity from another open document;
- dirty close cannot remove the document or registry record;
- clean close removes both view and lifecycle state;
- virtual documents cannot report a false successful save.

## Toolchain requirement

The generation environment does not contain Rust tooling. Run the complete local quality gate:

```bash
cargo fmt --all
cargo check --workspace --all-targets
./scripts/validate.sh
cargo run --release -p luna-ui-rust-editor-demo
```

## Runtime acceptance

```text
New File creates an empty clean Untitled-1 tab:
Closing clean Untitled-1 succeeds:
Creating another new file produces Untitled-2:
Typing marks the active tab modified:
Closing a dirty tab is blocked with Save/Discard/Cancel status:
Control-S on an untitled document reports Save As required:
Virtual demo documents do not report a successful filesystem save:
Tabs and Open Documents sidebar activate the same stable document:
File/Edit/Find/View/Help dropdown behavior remains correct:
Control-P command palette behavior remains correct:
M3.1b text-cache behavior remains correct:
M3.1c proof-gallery behavior remains correct:
```

Any compiler, strict-Clippy, test, rustdoc, lifecycle, menu, text-cache, accessibility, or native
runtime regression blocks M3.2b.
