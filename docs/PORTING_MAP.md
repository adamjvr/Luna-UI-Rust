# Swift-to-Rust Porting Map

| Swift Luna module | Rust destination | M0 status |
|---|---|---|
| `LunaCore` | `luna-core` | Node IDs, geometry, diagnostics started |
| `LunaInput` | `luna-input` | Platform-neutral events started |
| `LunaTheme` | `luna-theme` | Typed reference palette started |
| `LunaRender` | `luna-render` | Display list + safe CPU renderer started |
| `LunaAccessibility` | `luna-accessibility` | Validated semantic tree started |
| `LunaHostCore` | `luna-host-core` | Frame invalidation/runtime started |
| `LunaUI` | `luna-ui` | Widget contract + proof widget started |
| `LunaLayout` | future `luna-layout` | Planned M1 |
| `LunaTextCore` | future `luna-text-core` | Planned M2 |
| `LunaText` | future `luna-text` / `luna-text-cosmic` | Planned M2 |
| `LunaCommands` | future `luna-commands` | Planned M1 |
| `LunaHostSDL` / `LunaHostMetal` | `luna-host-winit` + platform adapters | Planned M1/M3 |
| CPU/glyph rendering | `luna-render` + future text adapter | Rectangle path started |
| GPU rendering | future `luna-render-wgpu` | Planned M3 |
| `LunaUITestApp` | `luna-ui-rust-demo` | Headless proof started |

## Porting rule

Behavioral tests and architectural invariants are ported before feature breadth. Rust types should
model invalid states out where practical, but the rewrite must not invent Moth product policy inside
Luna.
