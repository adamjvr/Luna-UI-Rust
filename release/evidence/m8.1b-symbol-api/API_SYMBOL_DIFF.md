# M8.1b Symbol-Level API Comparison

- Baseline commit: `e696df0cedaeda7ac5c0892cf8f709f8325eff8b`
- Current commit: `692f2403383e6aeeb178401c77dd7d3ea8efed77`
- Baseline crates: 23
- Current crates: 24
- Changed crates: 3
- Added symbols: 52
- Removed symbols: 3
- Intentionally breaking changes: 2

## Classified differences

### `luna-clipboard` — compatible

- Kind: `crate-added`
- Tier: `None` → `provisional`
- Added symbols: 49
- Removed symbols: 0
- Diff: `diffs/luna-clipboard.diff`
- Rationale: M8.1a adds a new provisional clipboard crate without removing or changing any accepted M7 public symbol.

### `luna-editor` — intentionally-breaking

- Kind: `symbols-changed`
- Tier: `provisional` → `provisional`
- Added symbols: 2
- Removed symbols: 2
- Diff: `diffs/luna-editor.diff`
- Rationale: M8.0 changes the provisional public ParityError::Mismatch expected and actual payloads from ParityResult to Box<ParityResult> to bound enum size under strict Clippy. Downstream code that constructs this variant must wrap payloads with Box::new, and code that destructures it must handle boxed values.

### `luna-ui` — intentionally-breaking

- Kind: `symbols-changed`
- Tier: `provisional` → `provisional`
- Added symbols: 1
- Removed symbols: 1
- Diff: `diffs/luna-ui.diff`
- Rationale: M8.0 renames the provisional public SearchHistory::next method to SearchHistory::next_newer. The new name states the navigation direction explicitly and avoids resembling Iterator::next. Downstream callers must replace history.next() with history.next_newer().
