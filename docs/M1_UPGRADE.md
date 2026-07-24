# M1 Overlay Upgrade

This tree is packaged as a repository-root overlay for the committed M0 checkout.

From the local repository root:

```bash
unzip -o /path/to/Luna-UI-Rust-M1-repo-root.zip
cargo fmt --all
./scripts/validate.sh
cargo run -p luna-ui-rust-native-demo
```

The overlay adds these workspace members:

- `crates/luna-layout`
- `crates/luna-commands`
- `crates/luna-accessibility-accesskit`
- `crates/luna-host-winit`
- `apps/luna-ui-rust-native-demo`

It also updates M0 crates and root documentation. The archive does not contain `.git`, `target`, a
Cargo registry, or generated framebuffer output. Existing Git history remains intact.

After validation, inspect and commit the exact change set:

```bash
git status --short
git diff --stat
git diff
```

Commit the validated change with a normal inline Git commit message.
