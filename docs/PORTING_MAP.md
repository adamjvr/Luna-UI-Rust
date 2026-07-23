# Swift-to-Rust Porting Map

| Swift Luna module | Rust destination | Current status |
|---|---|---|
| `LunaCore` | `luna-core` | M0 IDs, geometry, and diagnostics implemented |
| `LunaInput` | `luna-input` | Platform-neutral model plus winit translation implemented |
| `LunaTheme` | `luna-theme` | Typed reference palette implemented |
| `LunaRender` | `luna-render` | Display lists, safe CPU renderer, DPI scaling, raster images, and alpha composition implemented |
| `LunaAccessibility` | `luna-accessibility` | Validated semantic tree plus M2 text ranges implemented |
| Native accessibility bridge | `luna-accessibility-accesskit` | Stable-ID AccessKit bridge and text-area role/value mapping implemented |
| `LunaHostCore` | `luna-host-core` | Frame invalidation/runtime implemented |
| `LunaLayout` | `luna-layout` | Row/column/stack/split snapshots implemented |
| `LunaCommands` | `luna-commands` | Typed registry and key bindings implemented |
| `LunaUI` | `luna-ui` | Widget contract, workspace fixture, and M2 `TextView` implemented |
| `LunaHostSDL` / `LunaHostMetal` | `luna-host-winit` plus render adapters | Native CPU host implemented; GPU remains M3 |
| Swift static document/location/range types | `luna-text` | M2 line-plus-UTF-8 model implemented |
| Swift editable text foundation | `luna-text::EditableText` | M2 compact mutation/caret/selection model implemented |
| Swift text shaping and glyph path | `luna-text-cosmic` | M2 cosmic-text advanced shaping and raster adapter implemented |
| CPU/glyph rendering | `luna-render` plus `luna-text-cosmic` | M2 immutable glyph images and CPU composition implemented |
| GPU rendering | future `luna-render-wgpu` | Planned M3 |
| `LunaUITestApp` | headless/native/workspace/text demo applications | M0, M1, and M2 proofs implemented |

## Porting rule

Behavioral tests and architectural invariants are ported before feature breadth. Rust types model
invalid states out where practical, but the rewrite must not invent Moth product policy inside Luna.
Native and text-engine adapters translate; they do not redefine widget or document semantics.
