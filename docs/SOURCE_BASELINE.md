# Source Baseline

The rewrite was started from the Luna UI Swift 5D.3.2 proof-gallery/static-cache source snapshot.
The rewrite preserves architectural boundaries and externally intended behavior; it is not a
line-for-line transliteration.

Baseline archive SHA-256:

```text
8de3e8673f3d53c3c47cc52ab80ed0f206c1869947c0888fd78ea011dd4f398e
```

M2 also cross-references the product-neutral behavior introduced by the Swift text milestones:

- Phase 3A: static accessible text document and view;
- Phase 3B: UTF-8 caret coordinates, selection geometry, and hit testing;
- Phase 3C: viewport scrolling and visible text ranges;
- Phase 3D: compact editable text input and mutation foundation.

Those patches were used as behavioral provenance. The Rust implementation deliberately redesigns
storage, ownership, shaping, caching, error handling, and host boundaries according to Rust
practice rather than copying Swift source structure.
