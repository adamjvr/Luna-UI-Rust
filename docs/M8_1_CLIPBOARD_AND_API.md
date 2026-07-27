# M8.1a Clipboard Integration and API-Contract Classification

M8.1a closes the editor demo's known disabled Cut/Copy/Paste gap before the broader M8.1 symbol-level
public-API review. It introduces a product-neutral clipboard contract, a deterministic memory adapter,
and a native desktop adapter while keeping platform mechanics outside editor and widget crates.

## Architecture

`luna-clipboard` is a new provisional public crate containing:

- `ClipboardService`, the object-safe UTF-8 text clipboard boundary;
- `MemoryClipboardService`, used by tests and embedders;
- `SystemClipboardService`, backed by `arboard` for Linux, macOS, and Windows;
- `ClipboardError`, `ClipboardErrorKind`, and stable `CodedError` values.

The editor demo owns clipboard command policy. Luna's host and rendering crates remain unaware of Cut,
Copy, and Paste. The same editor implementation therefore runs through CPU and `wgpu` hosts without
renderer-specific clipboard logic.

## Editor semantics

- Copy is enabled only when the native adapter is available and at least one selection is non-empty.
- Cut has the same enablement and only deletes text after the clipboard write succeeds.
- Paste is enabled whenever the native adapter initialized successfully.
- Multiple selected ranges are copied in document order and joined with newline separators.
- Paste inserts the same clipboard text at every active caret or replaces every active selection.
- Cut and Paste are separate undoable replacement transactions.
- Clipboard failures preserve document text and are reported in the editor status area.
- `Ctrl+C`, `Ctrl+X`, and `Ctrl+V` share the same command path as menu and accessibility activation.

## M8.1 contract difference

The accepted M7.0.1 crate inventory remains unchanged in `api/baselines/m7.0.1.toml`.
`api/public-api.toml` adds `luna-clipboard` as a provisional crate. The checked-in
`api/compatibility/m8.1.json` classifies that difference as compatible and additive.

`scripts/check-api-contract-diff.py` requires every crate addition, removal, or tier change to have an
explicit classification and rationale. It rejects accidental changes and unclassified differences.
This is the M8.1 crate-contract layer; symbol-level snapshots and migration analysis remain subsequent
M8.1 work.

## Automated gate

Run the phase gate through a child-shell logging wrapper:

```bash
LUNA_M8_EVIDENCE_NAME=m8.1-clipboard-api \
./run-luna-safe.sh ./scripts/test-m8-1.sh
```

The script re-runs M8/M7 qualification, captures evidence, classifies the API contract difference,
runs clipboard and editor tests, and builds the release editor.

## Manual acceptance

Perform the following through both CPU and `wgpu` editor backends. Keep Luna running during cross-application Linux transfer because the originating application normally serves the clipboard selection.

1. Select text and confirm Edit > Copy and Edit > Cut become enabled.
2. Copy text and paste it into another Luna document.
3. Copy from Luna and paste into a different desktop application.
4. Copy from another desktop application and paste into Luna.
5. Cut text, undo, redo, and confirm document and clipboard state remain coherent.
6. Create multiple selections, copy them, and verify newline-separated clipboard text.
7. Paste with multiple carets and verify one insertion per caret.
8. Launch the extracted Linux package and repeat cross-application copy/paste.
9. Verify menu, keyboard, and accessibility command paths produce the same behavior.
10. Confirm CPU and `wgpu` behavior is identical.

M8.1a is accepted only after the automated gate and this operator pass succeed. Afterward,
`scripts/accept-m8-1.py` requires all six manual confirmation flags, verifies passing qualification and
API-diff JSON evidence, updates the acceptance documents, checks the M8.1a checklist, and records the
manual operator result under `release/evidence/m8.1-clipboard-api/`.
