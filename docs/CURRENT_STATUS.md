# Current Status

**Milestone:** M3.3c desktop interaction hardening — implemented, Pop!_OS validation pending

## Baseline

M3.3c is based on committed M3.3b.6 at
`45150b72de3df8922c8cac9dfb8d88638d27c784`. That baseline supplies recursive panes, advanced tabs,
nested menus, tab context menus, completion-popup presentation, richer literal find/replace, and
interactive scrollbars.

## Implemented in M3.3c

- versioned V2 editor-session format with V1 compatibility;
- recursive pane topology, split-ratio, focus, active-tab, order, pin, preview, and overflow restore;
- independent caret, directional selection, and scroll restoration per `DocumentViewId`;
- one persisted shared buffer per `DocumentId`;
- dirty-state and storage-baseline persistence with restart-time external-change detection;
- safe preview normalization for dirty, untitled, and virtual documents;
- keyboard tab reorder and previous/next-pane movement;
- arbitrary-depth dropdown selection paths, geometry, pointer routing, and accessibility;
- pointer-intent corridors and reusable delayed cascading-menu state;
- asynchronous completion request, cancellation, delivery, and stale-result rejection contracts;
- delayed demo completion provider with explicit candidate replacement ranges;
- bounded search history, wrap search, and Find in Selection;
- scrollbar page-up/page-down track actions;
- native-first Linux workspace watching with polling fallback;
- deterministic event coalescing and incremental subtree reconciliation;
- expanded model, service, editor-integration, corruption, restart, and runtime tests;
- `scripts/test-m3-3c.sh` and M3.3c runtime fixture.

## Architectural boundary

`luna-session` owns the persisted wire format. `luna-panes` owns valid topology and pane-local tab
policy. `luna-ui` owns product-neutral popup, completion, search-history, and scrollbar interaction
contracts. `luna-workspaces` owns watch delivery and snapshot reconciliation. The editor application
owns lifecycle decisions, provider choice, search policy, and UI-thread integration.

## Validation status

Static source checks, TOML parsing, shell syntax, delimiter scans, SPDX checks, banned-token scans,
and archive reconstruction checks are performed in this delivery environment. Rust 1.97.1 is not
installed in that environment, so compiler, rustfmt, strict Clippy, tests, and rustdoc must be run on
the target Pop!_OS workstation before M3.3c is accepted.

Run:

```bash
cargo fmt --all
./scripts/test-m3-3c.sh
```

Then perform the runtime checklist in `docs/M3_3C_DESKTOP_HARDENING.md` and the generated handoff.

## Next milestone

After M3.3c passes locally, M4 begins with `luna-render-wgpu`, immutable-display-list consumption,
surface/device-loss handling, batching, clip stacks, atlas upload, and CPU/GPU comparison fixtures.
