# M8.3 Repeated Long-Session Qualification

M8.3 validates that Luna's accepted M8.2 integration surfaces remain bounded and repeatable across
longer deterministic workloads and repeated native operator passes. It is a qualification phase, not
a feature-expansion phase.

The automated harness uses counts, capacities, and high-water marks as blocking evidence. It records
wall-clock durations only as diagnostics because shared machines and desktop environments do not
provide stable timing baselines.

## Architecture

The private binary is discovered by Cargo at:

```text
apps/luna-ui-rust-qualification/src/bin/luna-ui-rust-m8-3-long-session/
```

It belongs to the existing `luna-ui-rust-qualification` package and does not add a public library,
workspace package, or public Luna symbol. Workloads are divided into focused Rust modules so a failure
identifies the affected subsystem.

## Deterministic workload matrix

### Document lifecycle

Each cycle creates one in-memory UTF-8 file, loads it through `TextFileService`, registers one canonical
file document, edits through `EditableText`, saves with a matching storage precondition, marks the
document saved, verifies exact bytes, closes the document, and removes its backing file.

Blocking invariants:

- one open document maximum;
- one successful open, edit, save, and close per cycle;
- saved bytes exactly equal the editable buffer;
- the registry and memory filesystem return to an empty cycle boundary.

### Large-text cache reuse

A 4,096-line multilingual document is shaped repeatedly across near and distant scroll bands.

Blocking invariants:

- exactly one logical layout miss for an unchanged document and request;
- every sample is classified as a layout hit or miss;
- at least one raster-band hit occurs;
- raster misses remain below total samples;
- logical content exceeds one viewport.

### Pane and tab cycles

Each cycle creates four document views and exercises pinned and preview partitions, reordering,
horizontal splitting, cross-pane movement, splitter-ratio input, snapshot/restore, layout, pane close,
and split collapse.

Blocking invariants:

- no more than four views, two leaves, and one splitter;
- moved views have one pane owner;
- snapshots round-trip exactly;
- closing one side collapses the tree without an orphan splitter.

### Workspace mutation and watcher bursts

A deterministic memory workspace repeatedly creates, renames, and removes one source file. A memory
watcher emits intentionally redundant bursts, which are coalesced and delivered to `WorkspaceModel`.

Blocking invariants:

- six raw and three coalesced events per cycle;
- every real mutation produces a refresh;
- events stay below the active root;
- the workspace reaches four nodes maximum and returns to its three-node baseline.

### Session round-trips

Each cycle saves and restores one complete virtual document, view, workspace state, and valid one-pane
tree, then modifies and saves the restored state a second time.

Blocking invariants:

- exact equality after both round-trips;
- one document, one view, and one pane leaf;
- validated selection, caret, scroll, workspace, and tab ownership state.

### Render and lifecycle transitions

One private `NativeApplication` produces a real `UiFrame` through every built-in theme across multiple
logical sizes and scale factors. The CPU renderer allocates physical framebuffers, and the `wgpu` scene
compiler analyzes the same display list. The application receives resume, suspend, and memory-warning
transitions every cycle.

Blocking invariants:

- display commands, draw batches, atlas bytes, framebuffer bytes, and accessibility nodes remain
  below fixed high-water limits;
- resume and suspend update active state without requesting exit;
- every memory-warning transition is observed;
- default GPU vertex and index policy capacities remain visible in the report.

The scene compiler does not create a native GPU device. Actual Vulkan/`wgpu` reconstruction and
retained-resource behavior remain part of the real graphical operator protocol below.

### Resource loading

The harness receives explicit source-tree and extracted-package resource roots and reads
`welcome.txt` through a new `ResourceLocator` for every root on every cycle.

Blocking invariants:

- every load contains the reference-consumer marker;
- source and extracted content remain byte-for-byte identical;
- the resource stays below 64 KiB;
- load count equals cycles multiplied by root count.

## Complete automated qualification

From the repository root:

```bash
bash ./scripts/test-m8-3.sh
```

The gate:

1. formats the newly applied root-workspace Rust source;
2. verifies accepted M8.2 and candidate M8.3 documentation;
3. reruns the complete M8.2 root/consumer/package gate;
4. builds the private M8.3 qualification binary in release mode;
5. creates an ignored `dist/m8.3` workspace fixture;
6. packages and extracts the downstream consumer with checksum verification;
7. repeats source-tree and extracted-package self-tests in isolated state directories;
8. runs the seven-workload Rust harness twice;
9. validates report schema, workload order, counts, and limits;
10. removes only diagnostic timing fields and compares normalized reports byte-for-byte;
11. finishes with `git diff --check`.

Default workload settings:

```text
LUNA_M8_3_CYCLES=64
LUNA_M8_3_PACKAGE_LOOPS=4
```

For a larger local campaign:

```bash
LUNA_M8_3_CYCLES=256 \
LUNA_M8_3_PACKAGE_LOOPS=8 \
bash ./scripts/test-m8-3.sh
```

Accepted ranges are one through 512 workload cycles and one through 32 package loops. Generated JSON,
normalized reports, package ZIPs, extracted bundles, state directories, and self-test output remain
under ignored `dist/m8.3` and are not staged automatically.

Expected final line:

```text
[PASS] deterministic counts, capacities, high-water marks, and package loops passed
```

## Run the Rust harness directly

Build:

```bash
cargo build \
    --release \
    -p luna-ui-rust-qualification \
    --bin luna-ui-rust-m8-3-long-session
```

Run against the source resource only:

```bash
cargo run \
    --release \
    -p luna-ui-rust-qualification \
    --bin luna-ui-rust-m8-3-long-session -- \
    --cycles 64 \
    --output dist/m8.3/manual-long-session.json \
    --resource-root downstream/luna-reference-consumer/resources
```

A passing run ends with:

```text
m8_3_long_session=passed
```

## CPU long-session operator protocol

Run the editor:

```bash
LUNA_RENDER_BACKEND=cpu \
cargo run --release -p luna-ui-rust-editor-demo
```

Complete three rounds without restarting between individual steps:

1. open or create multiple text documents, edit them, save them, close them, and verify dirty-close
   protection;
2. split the editor horizontally and vertically, move tabs between panes, pin and preview tabs,
   reorder tabs, resize splitters, and collapse panes by closing them;
3. open the repository as a workspace and verify create, rename, delete, and external-change refreshes;
4. use find/replace, command palette, menus, completion, undo/redo, multiple selections, scrolling,
   and Cut/Copy/Paste;
5. switch through every theme, resize repeatedly, minimize/restore, and move between available DPI
   contexts when the desktop has more than one scale;
6. close and relaunch, confirming documents, dirty buffers, panes, tabs, caret/selection/scroll, recent
   files, and workspace tree state restore correctly.

Run the downstream consumer:

```bash
LUNA_RESOURCE_ROOT="$PWD/downstream/luna-reference-consumer/resources" \
cargo run \
    --manifest-path downstream/luna-reference-consumer/Cargo.toml \
    --release -- \
    --backend cpu \
    --workspace "$PWD"
```

Repeat editing, workspace reload, theme cycling, session save, close/relaunch, accessibility, and
resize checks. No visual corruption, lost input, unbounded status growth, stale workspace row, or
session regression is accepted.

## `wgpu` long-session operator protocol

Repeat the complete CPU protocol through Vulkan/`wgpu`:

```bash
LUNA_RENDER_BACKEND=wgpu \
cargo run --release -p luna-ui-rust-editor-demo
```

```bash
LUNA_RESOURCE_ROOT="$PWD/downstream/luna-reference-consumer/resources" \
cargo run \
    --manifest-path downstream/luna-reference-consumer/Cargo.toml \
    --release -- \
    --backend wgpu \
    --workspace "$PWD"
```

In addition to behavioral parity with CPU, verify:

- repeated resize and minimize/restore do not leave a stale or blank surface;
- theme and text changes do not corrupt image-atlas content;
- closing and relaunching reconstructs GPU state successfully;
- no device-loss, surface-loss, resource-budget, or retained-capacity error appears during ordinary
  operation;
- memory/resource diagnostics remain bounded when exposed by the application logs.

## Extracted-package operator protocol

Build and extract the downstream package:

```bash
./downstream/luna-reference-consumer/scripts/package-linux.sh \
    --output dist/m8.3/operator-package

rm -rf /tmp/luna-reference-consumer-m8.3
mkdir -p /tmp/luna-reference-consumer-m8.3
unzip -q \
    dist/m8.3/operator-package/Luna-Reference-Consumer-linux-"$(uname -m)".zip \
    -d /tmp/luna-reference-consumer-m8.3
```

From an unrelated working directory, do not set `LUNA_RESOURCE_ROOT`:

```bash
cd /tmp

/tmp/luna-reference-consumer-m8.3/Luna-Reference-Consumer/bin/luna-reference-consumer \
    --self-test \
    --workspace "$HOME/GitHub/Luna-UI-Rust"
```

Then launch the extracted application through both backends:

```bash
cd /tmp

/tmp/luna-reference-consumer-m8.3/Luna-Reference-Consumer/bin/luna-reference-consumer \
    --backend cpu \
    --workspace "$HOME/GitHub/Luna-UI-Rust"
```

```bash
cd /tmp

/tmp/luna-reference-consumer-m8.3/Luna-Reference-Consumer/bin/luna-reference-consumer \
    --backend wgpu \
    --workspace "$HOME/GitHub/Luna-UI-Rust"
```

Repeat the consumer editing, workspace, theme, session, accessibility, resize, close, and relaunch
checks. The extracted executable must load its own resource and persist session state without any
repository-relative resource path.

## Acceptance boundary

M8.3 is ready for acceptance when:

- `scripts/test-m8-3.sh` passes at the selected cycle and package-loop counts;
- two normalized reports are byte-for-byte identical;
- no deterministic limit is exceeded;
- CPU editor and consumer complete the operator protocol;
- `wgpu` editor and consumer complete the same protocol;
- extracted CPU and `wgpu` applications pass from an unrelated working directory;
- session state survives repeated relaunches;
- watcher and external-file changes remain accurate;
- no generated package, JSON report, extracted bundle, state directory, or raw log is staged in Git.

## M8.3.1 operator-discovered dirty-close blocker

The deterministic lifecycle tests passed, but Linux graphical acceptance found that one Zenity
extra-button response could be misclassified when the helper wrote the selected label on standard
error. M8.3 final acceptance therefore includes `scripts/test-m8-3-1.sh` and a manual CPU and
Vulkan/`wgpu` check that **Discard** removes the dirty tab without saving. The complete M8.3 harness
remains accepted as qualification infrastructure.
