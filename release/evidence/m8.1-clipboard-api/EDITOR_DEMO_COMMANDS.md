# Editor Demo Commands and Acceptance Guide

This is the permanent operator reference for `luna-ui-rust-editor-demo`. It describes the current
checked-in editor integration proof, not a promise that every command belongs in a downstream
product. Linux uses Control-based shortcuts. On macOS, the current winit proof still presents the
same Luna command labels while host-specific command-key policy remains under qualification.

## Launch commands

CPU reference renderer:

```bash
cargo run --release -p luna-ui-rust-editor-demo
```

Linux Vulkan through `wgpu`:

```bash
WGPU_BACKEND=vulkan \
LUNA_RENDER_BACKEND=wgpu \
cargo run --release -p luna-ui-rust-editor-demo
```

macOS Metal through `wgpu`:

```bash
WGPU_BACKEND=metal \
LUNA_RENDER_BACKEND=wgpu \
cargo run --release -p luna-ui-rust-editor-demo
```

## Application commands

| Shortcut | Command |
|---|---|
| `Ctrl+N` | Create a new untitled document. |
| `Ctrl+O` | Open a strict UTF-8 file through the native dialog boundary. |
| `Ctrl+Shift+O` | Open a workspace folder. |
| `Ctrl+Shift+R` | Refresh the active workspace snapshot. |
| `Ctrl+S` | Save, or request Save As for untitled content. |
| `Ctrl+Shift+S` | Save As to a new destination. |
| `Ctrl+W` | Close the active pane-local tab. |
| `Ctrl+P` | Open the searchable command palette. |
| `Ctrl+F` | Open the find panel. |
| `Ctrl+H` | Open find and replace. |
| `F3` | Select the next match. |
| `Shift+F3` | Select the previous match. |
| `Ctrl+Space` | Request completion suggestions at the caret. |
| `Ctrl+Z` | Undo the active shared-document transaction. |
| `Ctrl+Shift+Z` | Redo the active shared-document transaction. |
| `Ctrl+A` | Select the complete active document. |
| `Ctrl+B` | Toggle the workspace sidebar. |
| `Ctrl+\\` | Split the focused pane to the right. |
| `Ctrl+Shift+\\` | Split the focused pane downward. |
| `Ctrl+Shift+W` | Close the focused pane. |
| `Ctrl+Alt+Left` or `Ctrl+Alt+Up` | Focus the previous pane. |
| `Ctrl+Alt+Right` or `Ctrl+Alt+Down` | Focus the next pane. |
| `Ctrl+Alt+Shift+Left` or `Ctrl+Alt+Shift+Up` | Move the active tab to the previous pane. |
| `Ctrl+Alt+Shift+Right` or `Ctrl+Alt+Shift+Down` | Move the active tab to the next pane. |
| `Ctrl+Shift+PageUp` | Move the active tab left. |
| `Ctrl+Shift+PageDown` | Move the active tab right. |
| `Ctrl+Shift+Up` | Add a cursor on the preceding logical line. |
| `Ctrl+Shift+Down` | Add a cursor on the following logical line. |
| `Escape` | Dismiss the active popup/menu, clear secondary cursors, or exit when no surface is open. |

Cut, Copy, and Paste remain visible disabled placeholders in the Edit menu. They are not accepted
features and should not be recorded as test failures.

## Text navigation and editing

| Input | Behavior |
|---|---|
| Arrow keys | Move the primary caret through shared text geometry. |
| `Shift` plus arrow keys | Extend the directional selection. |
| `Home` / `End` | Move to the logical line start/end. |
| `Shift+Home` / `Shift+End` | Select to the logical line start/end. |
| `PageUp` / `PageDown` | Move by one editor viewport. |
| `Backspace` / `Delete` | Delete one Unicode grapheme at every active cursor. |
| `Enter` | Insert a newline at every active cursor. |
| Text input | Replace every active selection in one shared transaction. |

## Menu, popup, and pointer checks

1. Click File, Edit, Find, View, and Help and verify each dropdown anchors below its heading.
2. Move across headings while a dropdown is open; the active menu should switch without an extra
   click.
3. Exercise Up, Down, Home, End, Left, Right, Enter, Space, mnemonics, Escape, and outside-click
   dismissal.
4. Hover an arbitrary-depth submenu, then move diagonally toward the child popup. Pointer intent
   should keep the parent path open.
5. Open `View > Color Scheme` and verify Luna Dark, Luna Light, Amber Monitor, Green Terminal, and
   Different. The selected preset must expose a checked row.
6. Open the command palette, filter commands, move selection with the keyboard, activate by Enter
   and pointer, and dismiss by outside click.
7. Open completion suggestions, navigate with Up/Down, accept with Enter/Tab or pointer, and verify
   the replacement is one undoable transaction.
8. Open find/replace and exercise next, previous, case sensitivity, whole word, wrap, selection only,
   replace current, replace all, history, and close controls.

## Editor and selection pointer checks

1. Click to position the caret and Shift-click to extend the selection.
2. Drag across plain and wrapped rows; selection, paint, hit testing, and accessibility geometry must
   agree.
3. Start a drag, leave the editor edge, and verify captured selection and edge autoscroll continue.
4. Add multiple cursors, then type, delete, and insert a newline. Every edit must occur
   simultaneously from the end of the document toward the beginning.
5. Press Escape and verify secondary cursors are removed without changing text.
6. Use wheel/trackpad scrolling. Use Shift-wheel for horizontal movement where content is wider than
   the viewport.
7. Click above and below the vertical scrollbar thumb for page movement, then drag the thumb.

## Tabs, panes, documents, and workspaces

1. Open enough documents to exercise tab overflow and the overflow arrow controls.
2. Activate tabs, use close buttons, reorder by drag, and move a tab into another pane.
3. Secondary-click a tab and exercise pin, preview, move, and close commands.
4. Drag horizontal and vertical splitters and verify minimum extents and independent pane state.
5. Open the same shared buffer in more than one pane. Text must synchronize while caret, selection,
   scroll, focus, and retained text caches remain view-local.
6. Close a pane and verify final document views are rehomed rather than silently lost.
7. Open a folder, expand/collapse rows, activate files, and exercise create file, create folder,
   rename, recursive delete, and refresh behavior.
8. Modify, replace, remove, and recreate files externally. The watcher must distinguish those states
   and unchanged observations must not request frames.
9. Exercise Save, Save As, optimistic conflict handling, Reload from Disk, dirty close, and canceled
   dialogs. Content and dirty state must remain intact on cancellation or failure.
10. Restart and verify workspace root, expansion, selection, recent files, open documents, pane tree,
    active view, caret, directional selection, scroll, dirty buffers, and storage baselines restore.

## M7 release-qualification checks

Run the complete deterministic and compiler gate:

```bash
./scripts/test-m7.sh
```

Run the structural qualification executable directly:

```bash
cargo run --release -p luna-ui-rust-qualification -- \
  --output /tmp/luna-m7-qualification.json
```

Build the Linux development bundle:

```bash
./scripts/package-linux.sh
```

For a manual backend comparison, repeat the entire visual/interaction pass with the CPU and GPU
launch commands. Geometry, clipping, text placement, themes, input routing, and semantic behavior
must match. Performance diagnostics may differ, but the CPU renderer remains the correctness oracle.
