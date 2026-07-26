# M4 Validation Report

## Baseline

M4 is developed from committed M3.3c at
`b9aa4bbe8e65bc03e28ada1ec7a60726840bbb03`.

## Change set

- added `luna-render-wgpu` with ordered quad compilation, scissor batches, image-atlas upload, and
  deterministic scene tests;
- added `luna-host-wgpu` with winit lifecycle, AccessKit integration, surface recovery, device-loss
  recovery with event-loop wakeup, and GPU diagnostics;
- added nested display-list clip commands and matching CPU-renderer behavior;
- added runtime CPU/GPU proof-gallery and editor selection;
- added Luna Dark, Luna Light, Amber Monitor, and Green Terminal theme presets;
- added proof-gallery cycling and editor **View > Color Scheme** projection;
- added `docs/M4_GPU_RENDERING.md` and `scripts/test-m4.sh`;
- updated README, architecture, current status, roadmap, porting map, Swift parity, and validation
  documentation.

## Implemented invariants

- widgets and applications do not depend on `wgpu`;
- CPU and GPU consume the same immutable display-list layers;
- CPU and GPU nested clips use the same floor-leading/ceil-trailing DPI conversion;
- disjoint image and stack clips draw nothing;
- retained static paint precedes dynamic paint on both hosts;
- repeated identical raster images occupy one per-frame atlas entry;
- batch merging never reorders painter operations;
- surface reconfiguration does not recreate application state;
- device recovery rebuilds GPU resources while retaining application and accessibility state;
- all built-in themes are selected through one stable `ThemePreset` catalog;
- theme changes invalidate raster presentation, not document or pane identity.

## Static validation performed in the delivery environment

- all TOML files parse;
- every workspace member and local path dependency exists;
- Rust lexical delimiter and duplicate-splice scans pass;
- shell scripts pass `bash -n`;
- `git diff --check` reports no malformed whitespace;
- changed Rust and shell files retain SPDX identifiers;
- generated archives are tested with `unzip -t`;
- the repo-root overlay is reconstructed over the exact baseline and compared byte-for-byte;
- SHA-256 manifests are generated after final archive creation.

## Compiler and runtime boundary

Rust 1.97.1 and the new registry dependencies are unavailable in the artifact-building container.
Therefore this report does **not** claim that Cargo dependency resolution, rustfmt, rustc, strict
Clippy, tests, rustdoc, WGSL validation, adapter selection, surface presentation, or device recovery
were executed there.

The authoritative validation command is:

```bash
./scripts/test-m4.sh
```

The manual CPU/GPU and four-theme runtime checklist is documented in
`docs/M4_GPU_RENDERING.md` and generated at `/tmp/luna-m4-comparison/README.txt` by the script.
Any formatting, compiler, Clippy, test, rustdoc, shader, surface, accessibility, parity, or runtime
regression blocks acceptance.
