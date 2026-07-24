# M2 Overlay Upgrade

This tree is packaged as a repository-root overlay for the committed and corrected M1 checkout.

From the local repository root:

```bash
unzip -o /path/to/Luna-UI-Rust-M2-repo-root.zip
cargo fmt --all
./scripts/validate.sh
cargo run -p luna-ui-rust-text-demo
```

The overlay adds these workspace members:

- `crates/luna-text`
- `crates/luna-text-cosmic`
- `apps/luna-ui-rust-text-demo`

It extends `luna-render`, `luna-accessibility`, `luna-accessibility-accesskit`, and `luna-ui`, and it
retains the M1 AccessKit correction that reads `ActionRequest::target_node`.

The archive does not contain `.git`, `target`, a Cargo registry, a generated framebuffer, or an
outer wrapper directory. Existing Git history remains intact. Cargo may update the existing
`Cargo.lock` on the first validation run to add the pinned M2 text dependencies.

After validation:

```bash
git status --short
git diff --stat
git diff
git add -A
git commit -m "Build Luna-UI-Rust M2 editor-grade text foundation"
```
