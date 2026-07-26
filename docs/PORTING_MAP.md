# Swift-to-Rust Porting Map

| Swift Luna module or surface | Rust destination | Current status |
|---|---|---|
| `LunaCore` | `luna-core` | IDs, geometry, and diagnostics implemented |
| `LunaInput` | `luna-input` | Platform-neutral model plus winit translation implemented |
| `LunaTheme` | `luna-theme` | Luna Dark/Light plus Amber Monitor and Green Terminal presets with derived UI colors implemented |
| `LunaRender` | `luna-render` plus `luna-render-wgpu` | Display lists, nested clips, CPU oracle, DPI scaling, raster images, alpha composition, ordered GPU quad batches, and BGRA atlas upload implemented |
| `LunaAccessibility` | `luna-accessibility` | Validated tree, text ranges, editor-control roles, and deterministic semantic fingerprints implemented |
| Native accessibility bridge | `luna-accessibility-accesskit` | Stable-ID AccessKit bridge plus activation-aware unchanged-tree suppression implemented |
| `LunaHostCore` | `luna-host-core` | Frame invalidation/runtime implemented |
| `LunaLayout` | `luna-layout` | Row/column/stack/split snapshots implemented |
| `LunaCommands` | `luna-commands` | Typed registry and key bindings implemented |
| Swift document/session identity and lifecycle | `luna-documents` | Stable buffer IDs, independent view IDs, file identity, duplicate prevention, dirty/save/close decisions, storage snapshots, relocation, and detachment implemented |
| Swift static/editable text foundation | `luna-text` | UTF-8/grapheme-safe model implemented |
| Swift shaping/glyph path | `luna-text-cosmic` | Retained logical layout plus overscanned visible-run raster implemented |
| `LunaStaticTextView` / editable editor surface | `luna-ui::TextView` | Full logical geometry plus partial-raster placement implemented |
| `LunaEditorShell` | `luna-ui::EditorShell` | Menu/sidebar/editor/status anatomy plus optional legacy global-tab projection implemented |
| recursive editor panes and pane-local tabs | `luna-panes` plus `luna-ui::EditorPaneSurface` | Recursive topology, pin/preview/order/overflow, keyboard reorder/cross-pane movement, validated session snapshots, splitters, focus, close/collapse, hit testing, and accessibility implemented |
| menu bar and dropdowns | `luna-ui::DropdownMenu` and menu-definition types | Anchored parent/child submenu panels, disabled/checked state, mnemonics, pointer/keyboard routing, and accessibility implemented |
| product-neutral context menus | `luna-ui::DropdownMenu` with application-owned context state | Tab-anchored Pin/Preview/Move/Close context commands implemented |
| completion popup | `luna-ui::CompletionPopup` | Caret anchoring, async request/cancellation channels, stale-result rejection, explicit replacement ranges, candidate details, and keyboard/pointer/accessibility activation implemented |
| quick panel / command palette | `luna-ui::CommandPalette` | Separate searchable command surface driven by the same application command catalog |
| find/replace panel | `luna-ui::FindPanel` | Query/replacement fields, case/whole-word options, current/all actions, geometry, and accessibility implemented |
| general proof controls | `Button`, `Toggle`, `ProgressBar`, `TextLabel` | Reusable primitives plus stable-slot label cache implemented |
| `LunaUITestApp --proof-gallery` | `luna-ui-rust-proof-gallery` | Retained layout/static paint/semantics with isolated animation-lane rendering implemented |
| Swift file/dialog service boundary | `luna-document-services` | UTF-8 reads, atomic writes, file/folder dialogs, workspace mutation choices, storage observation, and deterministic mocks implemented |
| Swift workspace/project adapters | `luna-workspaces` | Stable recursive snapshots, mutations, native-first Linux watching, polling fallback, coalescing, incremental subtree reconciliation, refresh preservation, and deterministic adapters implemented |
| Swift persistent editor session | `luna-session` | Versioned atomic recent-file and workspace-tree restoration with standard and memory stores implemented |
| default `LunaUITestApp` editor mode | `luna-ui-rust-editor-demo` | Real files/workspaces, durable recursive pane sessions, advanced tabs, deep menus, async completion, search history/options, scrollbar paging, native watcher delivery, and accessibility integrated |
| `LunaHostSDL` / `LunaHostMetal` | `luna-host-winit` and `luna-host-wgpu` | CPU/softbuffer and GPU/wgpu hosts share input, application, invalidation, and AccessKit contracts; GPU surface/device recovery implemented |
| incremental gallery/accessibility pipeline | existing host/gallery/accessibility crates | M3.1c retained static layer, dirty-region restore, input coalescing, and semantic fingerprints implemented |
| Swift Phase 4C first-level menu behavior | `luna-ui::DropdownMenu` plus editor demo routing | M3.1d complete for first-level menus; submenus/mnemonics remain M3.3 |
| GPU rendering | `luna-render-wgpu` plus `luna-host-wgpu` | M4 optional native backend, batching, scissor clips, atlas upload, metrics, and proof-gallery comparison implemented |

## Porting rule

Behavioral tests and architectural invariants are ported before feature breadth. Rust types model
invalid states out where practical, but the rewrite must not invent Moth product policy inside Luna.
Native, text-engine, and GPU adapters translate; they do not redefine widget or document
semantics.

## Functional parity

See [`SWIFT_PARITY.md`](SWIFT_PARITY.md) for the broader feature inventory. Foundational module
coverage is near parity. First-level dropdown menus, document lifecycle state, UTF-8 file I/O,
atomic Save, Save As, native dialogs, recent files, continuous external-change delivery, and one
real recursive workspace tree, controlled workspace mutations, and persistent recent/workspace
restoration now exist. Recursive live panes, durable tab/view sessions, deep popups, async completion,
and native-first watcher delivery now exist; direct Moth integration remains concentrated in later platform work and M6.
