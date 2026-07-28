# Luna Reference Consumer

`luna-reference-consumer` is the M8.2 external downstream proof for Luna UI Rust.
It intentionally owns a separate Cargo workspace and is not a member of the Luna
repository workspace. Every Luna dependency is consumed through a public package
API by an ordinary downstream crate.

## What it proves

- native presentation through both the CPU and `wgpu` hosts;
- reusable Luna button, toggle, progress, label, and editable-text widgets;
- real workspace scanning through `StdWorkspaceService`;
- versioned session persistence through `StdSessionStore`;
- packaged-resource discovery through `ResourceLocator`;
- launch and self-test from an extracted ZIP without repository-relative paths.

The consumer is product-neutral. It does not contain Moth Text identity, command
policy, settings policy, language-server policy, or product-specific compatibility
behavior.

## Source-tree commands

Run the deterministic headless proof:

```bash
LUNA_RESOURCE_ROOT="$PWD/downstream/luna-reference-consumer/resources" \
    cargo run \
    --manifest-path downstream/luna-reference-consumer/Cargo.toml \
    --release -- --self-test --workspace "$PWD"
```

Run the native CPU application:

```bash
LUNA_RESOURCE_ROOT="$PWD/downstream/luna-reference-consumer/resources" \
    cargo run \
    --manifest-path downstream/luna-reference-consumer/Cargo.toml \
    --release -- --backend cpu --workspace "$PWD"
```

Run the same application through `wgpu`:

```bash
LUNA_RESOURCE_ROOT="$PWD/downstream/luna-reference-consumer/resources" \
    cargo run \
    --manifest-path downstream/luna-reference-consumer/Cargo.toml \
    --release -- --backend wgpu --workspace "$PWD"
```

## Controls

- type, select, scroll, and edit in the text surface;
- **Control-S** saves the downstream session;
- **Control-R** rescans the selected workspace;
- **Control-T** cycles Luna themes;
- click **Reload Workspace** or **Cycle Theme** to exercise reusable controls;
- **Escape** saves the session and closes the application.

## Linux package

```bash
./downstream/luna-reference-consumer/scripts/package-linux.sh
```

The package is written below `dist/m8.2/` as a ZIP. The extracted binary discovers
`share/org.lunaui.ReferenceConsumer/welcome.txt` relative to its executable.
