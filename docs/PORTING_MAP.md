# Swift-to-Rust Porting Map

| Swift Luna module | Rust destination | Current status |
|---|---|---|
| `LunaCore` | `luna-core` | M0 IDs, geometry, and diagnostics implemented |
| `LunaInput` | `luna-input` | M0 model plus M1 winit translation implemented |
| `LunaTheme` | `luna-theme` | Typed reference palette implemented |
| `LunaRender` | `luna-render` | Display list, safe CPU renderer, and DPI scaling implemented |
| `LunaAccessibility` | `luna-accessibility` | Validated semantic tree implemented |
| Native accessibility bridge | `luna-accessibility-accesskit` | M1 stable-ID AccessKit bridge implemented |
| `LunaHostCore` | `luna-host-core` | Frame invalidation/runtime implemented |
| `LunaLayout` | `luna-layout` | M1 row/column/stack/split snapshots implemented |
| `LunaCommands` | `luna-commands` | M1 typed registry and key bindings implemented |
| `LunaUI` | `luna-ui` | Widget contract, proof panel, and composite workspace fixture implemented |
| `LunaHostSDL` / `LunaHostMetal` | `luna-host-winit` plus render adapters | M1 native CPU host implemented; GPU remains M3 |
| `LunaTextCore` | future `luna-text-core` | Planned M2 |
| `LunaText` | future `luna-text` / `luna-text-cosmic` | Planned M2 |
| CPU/glyph rendering | `luna-render` plus future text adapter | Rectangles implemented; glyphs planned M2 |
| GPU rendering | future `luna-render-wgpu` | Planned M3 |
| `LunaUITestApp` | headless and native demo applications | M0 headless and M1 native proofs implemented |

## Porting rule

Behavioral tests and architectural invariants are ported before feature breadth. Rust types model
invalid states out where practical, but the rewrite must not invent Moth product policy inside Luna.
Native adapters translate; they do not redefine widget semantics.
