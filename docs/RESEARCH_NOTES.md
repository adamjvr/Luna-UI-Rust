# Research Notes

The M1 implementation was checked against current primary Rust ecosystem documentation on
July 22, 2026.

## Rust workspace policy

- Rust 1.97.1 is the pinned stable release for the workspace.
- Edition 2024 virtual workspaces explicitly use Cargo resolver version 3.
- `rust-version` records the minimum supported compiler and participates in dependency resolution.
- Package metadata and lint policy are inherited by every member crate.
- The project enables Clippy's `all` group plus selected policy lints rather than enabling the full
  `restriction` group indiscriminately.
- Stable rustfmt options are sufficient; nightly formatting is not required.

## Native M1 adapters

- **winit 0.30.13:** use `ApplicationHandler`, `EventLoop::run_app`, and window creation from
  `resumed`. Redraws are rendered from `WindowEvent::RedrawRequested`; scale-factor changes trigger
  a fresh logical viewport and physical framebuffer.
- **softbuffer 0.4.8:** use safe `Context::new`, `Surface::new`, `resize`, `buffer_mut`, and
  `present` APIs. M1 intentionally proves native lifecycle with the CPU display-list oracle before
  introducing wgpu.
- **AccessKit 0.24.1 / accesskit_winit 0.33.2:** create the adapter while the window is still hidden,
  call `process_event` before application handling, preserve numeric node identity across frames,
  and send a complete tree when using the event-loop-proxy constructor.
- **DPI:** layout and hit testing stay in logical pixels. The CPU renderer and AccessKit bridge
  convert the same snapshot to physical coordinates at their leaf boundaries.

## Deliberate exclusions

M1 does not introduce an async runtime, ECS, retained-mode callback graph, GPU API, text shaper,
serialization framework, or general-purpose dependency injection system. Those additions would
obscure the native lifecycle proof and make deterministic geometry harder to audit.

## CI hardening

The workflow uses read-only repository permissions and pins `actions/checkout` to a complete v6.0.2
commit SHA. Rust installation uses rustup already present on GitHub-hosted runners, avoiding an
additional toolchain action dependency.
