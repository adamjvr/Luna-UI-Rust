# Research Notes

The M0 policy and roadmap were checked against current primary Rust ecosystem documentation on
July 22, 2026.

## Confirmed baseline

- Rust 1.97.1 is the current stable release used by this workspace.
- Edition 2024 virtual workspaces should explicitly use Cargo resolver version 3.
- `rust-version` records the minimum supported compiler and participates in dependency resolution.
- Workspace package metadata and lint policy can be inherited by member crates.
- Rust API Guidelines favor conventional naming, common traits where semantics are clear,
  newtypes for domain concepts, complete rustdoc, and examples for public APIs.
- Clippy lint groups are not equally suitable for blanket enabling; the project enables `all` and
  selected correctness/policy lints while avoiding an indiscriminate `restriction` group.
- Stable rustfmt options are kept in `rustfmt.toml`; nightly-only formatting is not required.

## Planned native adapters

- **winit:** use the current `ApplicationHandler` and `EventLoop::run_app` lifecycle. Native window
  and graphics initialization belongs after the application is resumed.
- **wgpu:** consume Luna's immutable display lists from a dedicated backend crate; widgets never
  call wgpu directly.
- **cosmic-text:** use advanced shaping for complex scripts, bidirectional text, ligatures, and font
  fallback from the first text milestone.
- **AccessKit:** translate Luna's validated semantic tree, preserving stable node IDs and shared
  geometry.

## CI hardening

The workflow uses read-only repository permissions and pins `actions/checkout` to the full v6.0.2
commit SHA. Rust installation uses rustup already present on GitHub-hosted runners, avoiding an
additional toolchain action dependency.
