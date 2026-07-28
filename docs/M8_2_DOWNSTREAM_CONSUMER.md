# M8.2 External Downstream Consumer Proof

M8.2 validates Luna as a dependency of a separate, product-neutral Rust application. The reference
consumer lives at `downstream/luna-reference-consumer`, declares its own empty Cargo workspace, and is
not a member of the Luna repository workspace. Its Luna dependencies are ordinary path dependencies
used only through public package APIs.

This phase is a downstream-consumption proof, not a new product and not a broad Luna feature phase.
The reference consumer must remain free of Moth Text identity, commands, settings, language-service
policy, compatibility policy, and product workflow.

## Implemented proof surface

The external consumer exercises:

- `luna-host-winit` and `luna-host-wgpu` with one `NativeApplication` implementation;
- `Button`, `Toggle`, `ProgressBar`, `TextLabel`, and `TextView` reusable UI components;
- `EditableText`, `TextEngine`, shaped caret/selection geometry, scrolling, IME commit, and editable
  accessibility actions;
- `StdWorkspaceService` and `WorkspaceModel` for a real selected workspace;
- `StdSessionStore` for virtual-document text, caret, selection, scroll, workspace root, expansion,
  and selection persistence;
- `DownstreamServices` for application-owned adapter composition;
- `ResourceLocator` for source-tree and executable-relative packaged resource loading;
- a deterministic headless self-test that constructs a real `UiFrame` and validates display and
  accessibility output;
- a relocatable Linux ZIP that launches and self-tests from an unrelated working directory.

## Workspace boundary

The consumer manifest contains an intentional empty `[workspace]` table. Its Cargo metadata must
report:

```text
workspace_root = <repository>/downstream/luna-reference-consumer
```

It must not report the Luna repository root as its workspace root, and the root `Cargo.toml` must not
list the consumer as a workspace member.

## Complete automated qualification

From the Luna repository root:

```bash
./scripts/test-m8-2.sh
```

The gate performs:

1. external-workspace metadata verification;
2. the accepted Luna workspace format, API-contract, check, strict Clippy, test, and rustdoc gates;
3. rustfmt normalization followed by independent check, strict Clippy, test, rustdoc, and
   release build of the consumer;
4. source-tree headless public-API self-test;
5. Linux ZIP packaging and checksum verification;
6. extracted-package resource and executable checks;
7. extracted-package self-test from an unrelated current working directory;
8. final source whitespace validation.

Generated packages remain below ignored `dist/` or a temporary qualification directory. M8.2 does
not add binaries, package archives, raw build logs, or generated evidence directories to Git.

## Build the external consumer directly

```bash
cargo build \
    --manifest-path downstream/luna-reference-consumer/Cargo.toml \
    --release
```

## Run the source-tree headless proof

```bash
LUNA_RESOURCE_ROOT="$PWD/downstream/luna-reference-consumer/resources" \
    cargo run \
    --manifest-path downstream/luna-reference-consumer/Cargo.toml \
    --release -- \
    --self-test \
    --workspace "$PWD"
```

The final line must be:

```text
m8_2_self_test=passed
```

## Native CPU operator pass

```bash
LUNA_RESOURCE_ROOT="$PWD/downstream/luna-reference-consumer/resources" \
    cargo run \
    --manifest-path downstream/luna-reference-consumer/Cargo.toml \
    --release -- \
    --backend cpu \
    --workspace "$PWD"
```

Confirm:

- the native window opens and renders the header, workspace card, controls, editor, and status bar;
- typing, selection, navigation, deletion, pointer placement, drag selection, and scrolling work;
- **Reload Workspace** and **Control-R** rescan the selected workspace;
- **Cycle Theme** and **Control-T** change the complete surface theme;
- **Control-S** reports a successful session save;
- closing and relaunching restores edited text, caret/selection/scroll, and workspace state;
- accessibility exposes the window, labels, button, toggle, progress indicator, and editable text;
- **Escape** saves the session and closes the application.

## Native `wgpu` operator pass

```bash
LUNA_RESOURCE_ROOT="$PWD/downstream/luna-reference-consumer/resources" \
    cargo run \
    --manifest-path downstream/luna-reference-consumer/Cargo.toml \
    --release -- \
    --backend wgpu \
    --workspace "$PWD"
```

Repeat the CPU checklist and confirm the same state, input, text, resource, workspace, session, and
accessibility behavior through the GPU host.

## Build the Linux ZIP

```bash
./downstream/luna-reference-consumer/scripts/package-linux.sh
```

Expected output:

```text
dist/m8.2/Luna-Reference-Consumer/
dist/m8.2/Luna-Reference-Consumer-linux-<architecture>.zip
dist/m8.2/Luna-Reference-Consumer-linux-<architecture>.zip.sha256
```

## Extracted-package operator pass

```bash
rm -rf /tmp/luna-reference-consumer-m8.2
mkdir -p /tmp/luna-reference-consumer-m8.2
unzip -q \
    dist/m8.2/Luna-Reference-Consumer-linux-"$(uname -m)".zip \
    -d /tmp/luna-reference-consumer-m8.2

cd /tmp
/tmp/luna-reference-consumer-m8.2/Luna-Reference-Consumer/bin/luna-reference-consumer \
    --self-test \
    --workspace "$HOME/GitHub/Luna-UI-Rust"
```

Do not set `LUNA_RESOURCE_ROOT` for this pass. The extracted executable must discover
`share/org.lunaui.ReferenceConsumer/welcome.txt` relative to its package layout and finish with
`m8_2_self_test=passed`.

Then launch the extracted graphical application from `/tmp` through both backends and repeat the
basic editing, workspace, theme, session, and close checks.

## Acceptance boundary

M8.2 is ready for acceptance when:

- `scripts/test-m8-2.sh` passes;
- CPU and `wgpu` graphical operator passes complete;
- the extracted ZIP launches from an unrelated directory without `LUNA_RESOURCE_ROOT`;
- session state survives a close/relaunch cycle;
- no repository-relative resource path is required;
- no private Luna module is imported;
- no consumer-specific product policy enters a Luna crate;
- only the source files listed by the delivery are staged for Git.
