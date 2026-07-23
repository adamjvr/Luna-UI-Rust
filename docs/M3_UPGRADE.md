# M3 Upgrade

M3 is a repository-root overlay intended for a committed, compiler-verified M2 checkout.

```bash
unzip -o Luna-UI-Rust-M3-repo-root.zip
cargo fmt --all
./scripts/validate.sh
cargo run -p luna-ui-rust-proof-gallery
cargo run -p luna-ui-rust-editor-demo
```

The overlay adds two workspace applications and reusable editor/demo anatomy inside `luna-ui`. It
also extends the winit host with an optional scheduled logical-update hook. Existing event-driven
applications require no changes because the new trait methods provide default implementations.

The first validation run may update `Cargo.lock` only if the local committed M2 lockfile differs from
the dependency graph already pinned in the workspace.
