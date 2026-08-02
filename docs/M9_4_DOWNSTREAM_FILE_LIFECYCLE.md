# M9.4 — Downstream Product File Lifecycle Qualification

## Purpose

M9.4 proves that an independent Luna consumer can implement a complete text-file
lifecycle without moving product workflow or editor state into Luna.

## Qualified reusable mechanisms

- canonical file identity and duplicate-open detection;
- UTF-8 loading and explicit invalid-UTF-8 errors;
- deterministic storage snapshots containing content and storage-instance identity;
- optimistic write preconditions;
- same-directory atomic replacement;
- native open, save, dirty-close, and product-labeled conflict dialog boundaries;
- deterministic in-memory file and scripted-dialog adapters.

## Ownership boundary

Luna owns reusable file-system and dialog mechanisms. The downstream product owns
which document is active, buffer bytes, history, dirty state, command behavior,
conflict wording, close policy, and the decision to overwrite, reload, save as, or
cancel.

M9.4 adds qualification coverage and documentation. It does not make Luna the
source of truth for downstream editor state.
