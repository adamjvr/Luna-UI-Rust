# Swift-to-Rust Porting Map

| Swift Luna module or surface | Rust destination | Current status |
|---|---|---|
| `LunaCore` | `luna-core` | IDs, geometry, and diagnostics implemented |
| `LunaInput` | `luna-input` | Platform-neutral model plus winit translation implemented |
| `LunaTheme` | `luna-theme` | Dark/light reference palettes and derived UI colors implemented |
| `LunaRender` | `luna-render` | Display lists, CPU renderer, DPI scaling, raster images, alpha composition, and retained dirty-region restore primitives implemented |
| `LunaAccessibility` | `luna-accessibility` | Validated tree, text ranges, editor-control roles, and deterministic semantic fingerprints implemented |
| Native accessibility bridge | `luna-accessibility-accesskit` | Stable-ID AccessKit bridge plus activation-aware unchanged-tree suppression implemented |
| `LunaHostCore` | `luna-host-core` | Frame invalidation/runtime implemented |
| `LunaLayout` | `luna-layout` | Row/column/stack/split snapshots implemented |
| `LunaCommands` | `luna-commands` | Typed registry and key bindings implemented |
| Swift document/session identity and lifecycle | `luna-documents` | Stable IDs, file identity, duplicate prevention, dirty/save/close decisions, and external state implemented |
| Swift static/editable text foundation | `luna-text` | UTF-8/grapheme-safe model implemented |
| Swift shaping/glyph path | `luna-text-cosmic` | Retained logical layout plus overscanned visible-run raster implemented |
| `LunaStaticTextView` / editable editor surface | `luna-ui::TextView` | Full logical geometry plus partial-raster placement implemented |
| `LunaEditorShell` | `luna-ui::EditorShell` | Menu/tab/sidebar/editor/status anatomy plus active-menu projection implemented |
| menu bar and first-level dropdowns | `luna-ui::DropdownMenu` and menu-definition types | M3.1d anchored menus, disabled/checked state, pointer/keyboard routing, and accessibility implemented |
| quick panel / command palette | `luna-ui::CommandPalette` | Separate searchable command surface driven by the same application command catalog |
| find/replace panel foundation | `luna-ui::FindPanel` | M3 reusable geometry/state/accessibility implemented |
| general proof controls | `Button`, `Toggle`, `ProgressBar`, `TextLabel` | Reusable primitives plus stable-slot label cache implemented |
| `LunaUITestApp --proof-gallery` | `luna-ui-rust-proof-gallery` | Retained layout/static paint/semantics with isolated animation-lane rendering implemented |
| Swift file/dialog service boundary | `luna-document-services` | M3.2b UTF-8 reads, atomic writes, optimistic conflicts, native Linux dialogs, and deterministic mocks implemented |
| default `LunaUITestApp` editor mode | `luna-ui-rust-editor-demo` | Real Open, Save, Save As, duplicate activation, dirty-close resolution, and conflict reload/overwrite integrated |
| `LunaHostSDL` / `LunaHostMetal` | `luna-host-winit` plus render adapters | Retained working/static CPU buffers, timed update lane, and conditional AccessKit submission implemented |
| incremental gallery/accessibility pipeline | existing host/gallery/accessibility crates | M3.1c retained static layer, dirty-region restore, input coalescing, and semantic fingerprints implemented |
| Swift Phase 4C first-level menu behavior | `luna-ui::DropdownMenu` plus editor demo routing | M3.1d complete for first-level menus; submenus/mnemonics remain M3.3 |
| GPU rendering | future `luna-render-wgpu` | Planned M4 after M3.2/M3.3 editor breadth |

## Porting rule

Behavioral tests and architectural invariants are ported before feature breadth. Rust types model
invalid states out where practical, but the rewrite must not invent Moth product policy inside Luna.
Native, text-engine, and future GPU adapters translate; they do not redefine widget or document
semantics.

## Functional parity

See [`SWIFT_PARITY.md`](SWIFT_PARITY.md) for the broader feature inventory. Foundational module
coverage is near parity. First-level dropdown menus, document lifecycle state, UTF-8 file I/O,
atomic Save, Save As, and native dialog boundaries now exist. Workspaces, continuous external-change delivery, panes, nested menus,
context menus, completion, and direct Moth integration remain concentrated in later M3.2, M3.3,
and M6 work.
