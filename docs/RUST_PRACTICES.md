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
Do not introduce global mutable state. `Arc` is allowed when immutable frame data is intentionally
shared across multiple display-list or widget snapshots; M2 uses it for raster pixels and caret-stop
arrays so reshaping products remain cheap to clone without exposing mutable caches.

Native and shaping adapters own mutable external-library state. In particular, cosmic-text
`FontSystem` and `SwashCache` values remain in `luna-text-cosmic`; widgets receive only immutable
backend-neutral geometry and pixels.

## Text coordinates

Durable document coordinates are zero-based logical-line plus UTF-8-byte columns. Every public
mutation snaps through explicit scalar-boundary policy, while user-visible movement and deletion use
Unicode extended-grapheme boundaries. Never persist cosmic-text cursor values, Rust byte indices, or
platform accessibility positions as Luna document identity.

Paint, caret geometry, selection, hit testing, scrolling, and semantic ranges must derive from the
same shaped snapshot. A change that computes one of those independently requires a documented
reason and regression coverage.

## Concurrency

The deterministic UI lane remains single-threaded by architecture. Background services may use
threads or async runtimes behind message-oriented adapters. No async runtime is selected yet; that
decision belongs to concrete service requirements rather than the core UI model.

## Dependencies

Add a dependency only after documenting:

- the specific adapter boundary that owns it;
- license compatibility;
- supported platforms;
- maintenance/activity status;
- feature flags actually required;
- whether a smaller dependency or standard-library implementation suffices.

Current external dependencies are isolated by role: winit and softbuffer in the native host,
AccessKit in the accessibility bridge, unicode-segmentation in the platform-neutral text model, and
cosmic-text in the shaping/raster adapter. A future wgpu backend belongs in a renderer leaf crate,
not in `luna-core`, `luna-text`, or `luna-ui`.
