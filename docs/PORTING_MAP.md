# Swift-to-Rust Porting Map

| Swift Luna module or surface | Rust destination | Current status |
|---|---|---|
| `LunaCore` | `luna-core` | IDs, geometry, and diagnostics implemented |
| `LunaInput` | `luna-input` | Platform-neutral model plus winit translation implemented |
| `LunaTheme` | `luna-theme` | Dark/light reference palettes and derived UI colors implemented |
| `LunaRender` | `luna-render` | Display lists, CPU renderer, DPI scaling, raster images, and alpha composition implemented |
| `LunaAccessibility` | `luna-accessibility` | Validated tree, text ranges, and editor-control roles implemented |
| Native accessibility bridge | `luna-accessibility-accesskit` | Stable-ID AccessKit bridge and M3 role mappings implemented |
| `LunaHostCore` | `luna-host-core` | Frame invalidation/runtime implemented |
| `LunaLayout` | `luna-layout` | Row/column/stack/split snapshots implemented |
| `LunaCommands` | `luna-commands` | Typed registry and key bindings implemented |
| Swift static/editable text foundation | `luna-text` | UTF-8/grapheme-safe model implemented |
| Swift shaping/glyph path | `luna-text-cosmic` | cosmic-text shaping/raster adapter implemented |
| `LunaStaticTextView` / editable editor surface | `luna-ui::TextView` | Shared paint/hit/scroll/accessibility geometry implemented |
| `LunaEditorShell` | `luna-ui::EditorShell` | M3 menu/tab/sidebar/editor/status anatomy implemented |
| quick panel / command palette | `luna-ui::CommandPalette` | M3 reusable overlay implemented |
| find/replace panel foundation | `luna-ui::FindPanel` | M3 reusable geometry/state/accessibility implemented |
| general proof controls | `Button`, `Toggle`, `ProgressBar`, `TextLabel` | M3 reusable primitives implemented |
| `LunaUITestApp --proof-gallery` | `luna-ui-rust-proof-gallery` | M3 dedicated native regression gallery implemented |
| default `LunaUITestApp` editor mode | `luna-ui-rust-editor-demo` | M3 dedicated editor integration harness implemented |
| `LunaHostSDL` / `LunaHostMetal` | `luna-host-winit` plus render adapters | Native CPU host and timed update lane implemented |
| GPU rendering | future `luna-render-wgpu` | Planned M4 |

## Porting rule

Behavioral tests and architectural invariants are ported before feature breadth. Rust types model
invalid states out where practical, but the rewrite must not invent Moth product policy inside Luna.
Native, text-engine, and future GPU adapters translate; they do not redefine widget or document
semantics.
