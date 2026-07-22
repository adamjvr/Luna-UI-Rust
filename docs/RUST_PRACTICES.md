# Rust Practices

## Toolchain and edition

The workspace pins Rust 1.97.1, uses Edition 2024, and explicitly selects Cargo resolver version 3.
The minimum supported Rust version is encoded through `rust-version` rather than left implicit.

## Error handling

Recoverable failures return typed `Result` values. Production code must not use `unwrap`, `expect`,
`panic`, `todo`, or `unimplemented`. At impossible internal boundaries, code should fail closed and
emit a diagnostic until an invariant can be encoded more strongly in the type system.

## Public API

Public types and functions require rustdoc. Newtypes are preferred for semantic IDs and units.
Common traits (`Debug`, equality, ordering, hashing, defaults) are implemented when their semantics
are unambiguous and useful. API names follow standard Rust casing and conversion conventions.

## Ownership and snapshots

Widget state owns its data. Frame products are immutable owned snapshots or short-lived borrows.
Do not introduce global mutable state. Use `Arc` only when ownership is genuinely shared across
threads; do not use it preemptively to imitate reference semantics from another language.

## Concurrency

The deterministic UI lane remains single-threaded by architecture. Background services may use
threads or async runtimes behind message-oriented adapters. No async runtime is selected at M0;
that decision belongs to concrete service requirements rather than the core UI model.

## Dependencies

Add a dependency only after documenting:

- the specific adapter boundary that owns it;
- license compatibility;
- supported platforms;
- maintenance/activity status;
- feature flags actually required;
- whether a smaller dependency or standard-library implementation suffices.

The expected native stack is winit for window/event integration, wgpu for GPU rendering, cosmic-text
for advanced shaping/fallback, and AccessKit for accessibility adaptation. They will be introduced in
separate leaf crates rather than the deterministic core.
