# Contributing

Luna-UI-Rust is an editor-class foundation, so correctness and stable boundaries matter more than
rapid dependency accumulation.

Before submitting changes:

1. Keep product behavior out of Luna crates.
2. Derive paint, hit testing, caret/selection geometry, scrolling, and accessibility from shared
   geometry or one immutable shaped snapshot.
3. Preserve Luna document coordinates as logical line plus UTF-8 byte column; use extended
   grapheme boundaries for user-visible motion and deletion.
4. Return typed errors at recoverable boundaries; do not use `unwrap`, `expect`, or `panic` in
   production code.
5. Keep unsafe code forbidden until a narrowly reviewed platform boundary proves it unavoidable.
6. Document every public item and explain architectural decisions, not obvious syntax.
7. Add regression tests for Unicode, clipping, tiny viewports, long lines, and invalid coordinates
   when changing text or layout behavior.
8. Keep text-file, storage-observation, and native-dialog implementations behind
   `luna-document-services`; do not put byte I/O or modal product policy in `luna-documents` or
   `luna-ui`.
9. Keep recursive folder scans, workspace snapshots, expansion, and selection contracts behind
   `luna-workspaces`; do not put document buffers or filesystem mutation policy in that crate.
10. Run `./scripts/validate.sh`; use `./scripts/test-m3-2d.sh` for workspace changes.

Add dependencies only in the narrowest adapter crate that needs them. A window backend does not
belong in `luna-core`; a shaping cache does not belong in `luna-text`; a GPU backend does not belong
in `luna-ui`; product commands do not belong in any Luna crate.
