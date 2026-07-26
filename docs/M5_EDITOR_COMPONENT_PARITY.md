# M5 Editor Component Parity

## Purpose

M5 fills reusable editor-mechanics gaps without assigning application or language policy to Luna.
The milestone adds a product-neutral syntax boundary, Sublime color-scheme import, transactional
history, multiple selections, IME composition, richer command state, and actionable accessibility
payloads. The editor demo is an integration harness; these mechanisms are reusable independently.

## New `luna-editor` crate

`luna-editor` owns editor mechanics that do not belong in the UTF-8 document model, renderer, widget
library, or product layer:

- `SyntaxProvider`, immutable `SyntaxSnapshot`, validated UTF-8 `SyntaxSpan`, and resolved style rules;
- `SublimeColorSchemeAdapter` for comment-tolerant `.sublime-color-scheme` JSON;
- `SelectionSet` for ordered directional multiple selections and simultaneous right-to-left edits;
- grapheme-safe multi-cursor backward and forward deletion;
- bounded `EditHistory`, semantic edit groups, coalescing, undo, redo, and text-based saved checkpoints;
- `ImeComposition` with pre-edit text, pre-edit selection, replacement range, commit, and cancellation;
- rendering-independent `EditorParityFixture` replay for matching behavior in other implementations.

The adapter recognizes global background, foreground, caret, and selection values plus rule scopes,
foreground/background colors, and bold/italic/underline flags. M5 applies foreground, background, and
underline output in the proof editor. Bold and italic remain retained style metadata until the shaping
adapter exposes stable font-style selection without disturbing caret geometry.

## Styled text path

```text
application-owned syntax provider
    -> validated UTF-8 syntax snapshot
    -> imported product-neutral syntax theme
    -> resolved foreground/background/underline spans
    -> retained cosmic-text geometry keyed by document + style revision
    -> immutable TextView decorations
    -> identical CPU or wgpu display-list presentation
```

Syntax providers never enter `luna-text-cosmic` or `luna-ui`. The shaping adapter receives only
validated color spans. The widget receives only logical decorations. Parsing and language semantics
remain replaceable.

## Transaction and multiple-selection model

One `SelectionSet` owns a primary directional selection plus zero or more secondary selections.
Selections are snapped to UTF-8 boundaries, sorted, deduplicated, and merged before edits. One edit
operation computes all pre-edit ranges and applies replacements from the end of the document toward
the beginning, preventing earlier changes from shifting later ranges.

Every committed change records complete before/after text and selection snapshots. Typing and
deletion may coalesce when transaction boundaries are contiguous; replacement, completion, search,
IME, command, and accessibility edits remain discrete. Undo/redo restores text and every selection.
Caret movement alone does not invalidate the saved checkpoint.

The editor integration routes typing, Backspace, Delete, Enter, completion acceptance, Replace
Current, Replace All, IME commit, and editable accessibility replacement through this transaction
path. Secondary selections are intentionally transient in M5 session persistence; the durable M3.3c
session format continues to restore the primary view selection.

## IME contract

`luna-input` now represents native IME enable, disable, pre-edit, and commit events. Both native hosts
forward winit IME events and update the operating-system candidate-window area from application-owned
caret geometry. Pre-edit state is visible but does not mutate the document or history. Commit replaces
the composition's captured document range as one transaction; disable, focus change, pointer editing,
or explicit cancellation clears the composition.

## Commands and accessibility

`luna-commands` adds dynamic `CommandState` and `CommandStateProvider` contracts so menu, palette,
keyboard, and accessibility projections can share enabled/checked decisions. The editor uses this for
Undo and Redo availability.

Semantic nodes may now declare explicit product-neutral actions. The AccessKit bridge translates
those actions, and both native hosts carry UTF-8 `Value` payloads for `ReplaceSelectedText` and
`SetValue`. The editor and text demo execute those requests through the same edit path used by normal
input. Selection payload translation remains a later text-accessibility phase because AccessKit text
positions refer to semantic text-run node identities rather than Luna's durable document coordinates.

## Validation

Automated Linux/Pop!_OS acceptance:

```bash
cargo fmt --all
./scripts/test-m5.sh
```

Manual editor checks are generated at `/tmp/luna-m5-parity/README.txt`. M5 is not accepted until the
CPU and GPU editor runs pass syntax, undo/redo, multi-cursor, IME, accessibility, and four-theme checks.

macOS has a separate advisory lane documented in [`MACOS_TESTING.md`](MACOS_TESTING.md). Windows is
not an official target, release gate, or packaging commitment.
