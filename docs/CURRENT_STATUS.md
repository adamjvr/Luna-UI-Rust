# Current Status

**Milestone:** M0 deterministic foundation

**Implemented:** workspace policy, core geometry/identity, input values, theme colors, display list,
safe CPU framebuffer renderer, accessibility tree validation, frame invalidation runtime, widget
contract, proof panel, headless PPM demo, unit tests, and CI definition.

**Not yet implemented:** native window, GPU backend, text shaping, reusable layout crate, command
system, native accessibility adapter, editor widgets, dialogs, theme-file parsing, or Moth integration.

**Validation note:** the package was structurally generated and inspected in an environment without
a Rust toolchain. `./scripts/validate.sh` is the mandatory first command on a Rust-equipped machine;
compiler or Clippy findings should be treated as M0 blockers rather than deferred.
