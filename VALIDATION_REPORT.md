# M3.1d.1 Validation Report

## Verified baseline

The project owner confirmed that M3.1d compiled and was committed, but native runtime testing found
that top-level menu clicks still presented the command-palette interface rather than reliable anchored
dropdowns. M3.1d.1 is a correction built directly on that reported runtime state.

## M3.1d.1 correction set

- top-level menu pointer hits are resolved before palette/find/editor overlay handling;
- opening any dropdown clears every other transient surface;
- the command palette is removed from dropdown projection and remains available through Ctrl+P;
- runtime diagnostics identify menu open/close state explicitly;
- dropdown presentation is narrower and shadowed, without the palette's backdrop, title, or query box;
- regression coverage clicks File while the palette is already open and verifies one anchored menu,
  no palette, no find panel, and one transient surface.

## Original M3.1d change set

- `crates/luna-ui/src/dropdown_menu.rs`
  - product-neutral command/menu definitions and interaction state;
  - anchored viewport-clamped dropdown geometry;
  - separators, shortcuts, enabled/disabled state, checked state, and selected state;
  - pointer and accessibility command resolution;
  - menu/menu-item semantics;
  - deterministic keyboard-selection, clamping, disabled-command, and accessibility tests.
- `crates/luna-ui/src/editor_shell.rs`
  - application-supplied active menu ID;
  - active-heading paint state;
  - expanded/collapsed and focused accessibility state;
  - regression coverage for selected menu headings.
- `crates/luna-ui/src/lib.rs`
  - public exports for the reusable dropdown-menu model and widget.
- `apps/luna-ui-rust-editor-demo/src/main.rs`
  - one File/Edit/Find/View/Help command catalog;
  - independent menu, palette, and find-panel state;
  - shared dropdown and command-palette projections;
  - real menu pointer, keyboard, and accessibility routing;
  - outside-click dismissal and cross-heading traversal;
  - common command execution for shortcuts, menus, palette, pointer, and accessibility;
  - disabled command omission from the current palette projection;
  - regression tests proving menu/palette separation and command projection.
- project documentation
  - M3.1d status, architecture, roadmap, porting map, Swift parity, runtime instructions, and this
    validation report.

## Structural checks performed in the generation environment

- all workspace manifests and TOML configuration files parse;
- all workspace members and local path dependencies remain present;
- modified Rust files retain MPL-2.0 SPDX identifiers;
- no unsafe blocks or declarations were introduced;
- no `.unwrap()`, `.expect()`, `panic!`, `todo!`, or `unimplemented!` calls were introduced;
- changed Rust source remains within the repository's 100-column policy before rustfmt;
- dropdown keyboard traversal skips separators and disabled commands;
- disabled rows cannot resolve to executable pointer or accessibility commands;
- menu and palette state are independent;
- menu-heading clicks take dispatch priority even while the palette is open;
- top-level menu headings cannot call or project `open_palette`;
- one-transient-surface assertions guard menu, palette, and find construction;
- command shortcuts, menu rows, palette rows, and accessibility converge on one executor;
- menu interaction does not mutate text-layout or raster cache state;
- repository-root overlay and full-source archive round trips are verified during packaging;
- no commit-message text file is included.

## Toolchain limitation

This generation environment does not contain `rustc`, Cargo, rustfmt, or Clippy and cannot resolve
outbound toolchain downloads. It therefore does not claim compiler or native-runtime validation for
the new M3.1d.1 code.

Run the complete local gate before committing:

```bash
cargo fmt --all
./scripts/validate.sh
cargo run --release -p luna-ui-rust-editor-demo
cargo run --release -p luna-ui-rust-proof-gallery
```

## Runtime acceptance record

```text
File heading opens File dropdown:
Edit heading opens Edit dropdown:
Find heading opens Find dropdown:
View heading opens View dropdown:
Help heading opens Help dropdown:
Palette open, then one File click replaces it with anchored dropdown:
Ordinary menu click opens command palette unexpectedly:
Ctrl+P opens searchable command palette:
Menu runtime log reports `palette=false find=false`:
Same-heading click closes dropdown:
Cross-heading click/hover switches dropdown:
Outside click dismisses dropdown:
Disabled command activation attempts:
Checked sidebar/theme state correct:
Up/Down/Home/End navigation correct:
Left/Right menu switching correct:
Enter/Space activation correct:
Escape dismissal correct:
Menu and palette command-result parity:
Text-layout misses while opening/navigating menus:
Text-raster misses while opening/navigating menus:
Editor idle frames after 10 seconds:
Proof-gallery M3.1c regression:
Accessibility heading/dropdown/command behavior:
```

Any formatting, compiler, strict-Clippy, test, rustdoc, painting, hit-test, keyboard, pointer,
accessibility, command-routing, editor-cache, proof-gallery, or native-runtime regression blocks
M3.2.
