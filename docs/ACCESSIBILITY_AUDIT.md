# Accessibility Qualification Audit

Accessibility is a release boundary, not optional polish. M7 keeps the existing law:

```text
paint bounds = hit-test bounds = accessibility bounds
```

## Automated evidence

- semantic trees reject duplicate IDs, missing roots, missing children, and cycles;
- fingerprints are deterministic and suppress unchanged native translation;
- editor text exposes UTF-8 text, caret, selected, and visible ranges;
- menus, tabs, trees, dialogs, status regions, completion rows, and editable surfaces expose stable
  roles and actions;
- the M7 qualification executable builds and budgets a representative semantic tree;
- strict tests and rustdoc remain part of the blocking Linux gate.

## Manual Linux acceptance

Use a supported AccessKit desktop path to inspect focus order, labels, checked/disabled menu state,
editable replacement actions, tab and tree navigation, completion activation, and status notices.
Verify that pointer, keyboard, and accessibility activation dispatch the same command identity.

## Manual macOS acceptance

VoiceOver acceptance remains real-hardware work. Exercise application launch, menu and pane focus,
editable text, selection announcements, completion and context menus, dirty-close dialogs, IME
composition, and CPU/Metal rendering. Record hardware, macOS version, architecture, backend, and any
exceptions in the release checklist.
