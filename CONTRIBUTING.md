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
8. Keep document identity and lifecycle decisions in `luna-documents`.
9. Keep text-file I/O and native dialog implementations behind `luna-document-services`.
10. Keep immutable tree observation and filesystem mutations behind `luna-workspaces`; widgets must
    not call `std::fs` directly.
11. Keep persisted recent/workspace state behind `luna-session`; rendering and document crates must
    not know the session-file format.
12. Keep recursive pane topology, tab order, pin/preview metadata, movement, and overflow offsets in
    `luna-panes`; document bytes and lifecycle state remain keyed by `DocumentId`.
13. Keep popup geometry and semantics product-neutral in `luna-ui`; applications own command,
    completion-provider, and search policy.
14. Deliver watcher events and filesystem snapshots onto the application UI thread before mutating
    editor state.
15. Keep operating-system lifecycle, dialogs, watchers, state paths, and packaging in leaf adapters;
    Linux is primary, macOS is secondary, and Windows is best-effort only.
16. Keep downstream service composition application-owned; do not turn `luna-integration` into a
    service locator or product-policy crate.
17. Keep every public library in `api/public-api.toml` and `CRATE_CONTRACTS`; public errors must
    implement `CodedError`.
18. Update `docs/EDITOR_DEMO_COMMANDS.md` whenever editor keyboard, mouse, or acceptance behavior
    changes.
19. Preserve deterministic qualification budgets and explicit GPU/resource caps; do not replace
    them with flaky shared-runner timing assertions.
20. Run `./scripts/validate.sh` and the current milestone-specific test script.

Add dependencies only in the narrowest adapter crate that needs them. A window backend does not
belong in `luna-core`; a shaping cache does not belong in `luna-text`; a GPU backend does not belong
in `luna-ui`; product commands do not belong in any Luna crate.
