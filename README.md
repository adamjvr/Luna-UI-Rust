# Luna-UI-Rust

**Luna-UI-Rust** is a clean Rust-native rewrite of Luna UI: the product-neutral UI, input,
layout, rendering, accessibility, and host foundation used to build editor-class desktop
applications such as Moth Text.

This is not a mechanical Swift-to-Rust translation. The rewrite preserves Luna's architectural
contracts while expressing them with Rust's ownership model, explicit error handling, workspace
structure, and lint/tooling ecosystem.

## M0 status

The first milestone is implemented as a dependency-light Rust 2024 workspace:

- stable validated node IDs;
- integer geometry shared by paint, hit testing, and accessibility;
- platform-neutral input events;
- strongly typed theme colors;
- immutable backend-neutral display lists;
- safe BGRA8 CPU reference framebuffer and renderer;
- validated platform-neutral accessibility trees;
- deterministic frame invalidation/runtime state;
- a product-neutral `Widget` contract;
- a headless demo that writes a PPM proof frame;
- unit tests, strict lints, formatting policy, and GitHub Actions CI.

The initial milestone intentionally has **zero third-party runtime dependencies**. This keeps the
core easy to audit and validates the architecture before native host, GPU, text shaping, and
AccessKit adapters are pinned behind their own crates.

## Build and validate

Install the pinned toolchain through rustup, then run:

```bash
./scripts/validate.sh
```

Run the proof application:

```bash
cargo run -p luna-ui-rust-demo -- ./luna-ui-rust-m0.ppm
```

Most Linux image viewers can open PPM directly. ImageMagick can convert it when desired:

```bash
magick luna-ui-rust-m0.ppm luna-ui-rust-m0.png
```

## Governing architecture

```text
native host event
    -> platform-neutral Luna input
    -> deterministic state update
    -> layout / shared geometry
    -> immutable display list + accessibility snapshot
    -> renderer / accessibility adapter
    -> present
```

Widgets do not call graphics APIs, open native dialogs, own operating-system event loops, or embed
Moth-specific policy. See `docs/ARCHITECTURE.md` and `docs/PORTING_MAP.md`.

## License

Mozilla Public License 2.0. Source files include SPDX identifiers and modifications to MPL-covered
files remain available under the MPL terms.
