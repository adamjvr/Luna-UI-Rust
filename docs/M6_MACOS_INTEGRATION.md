# M6 macOS Hardening and Downstream Integration

## Scope

M6 turns the macOS roadmap from an advisory compile lane into a concrete secondary-platform
implementation track. Linux remains the primary release and graphical-acceptance platform. macOS is
maintained as the supported secondary platform, but its CI job remains advisory until repeated
real-hardware CPU, Metal, IME, VoiceOver, dialog, watcher, and package acceptance reports exist.
Windows remains best-effort and non-blocking.

M6 does not assign Luna-UI-Rust to a particular downstream product. Integration examples compose the
existing product-neutral service boundaries and leave document policy, language semantics, settings,
commands, packaging identity, and workflow with the consuming application.

## Native lifecycle and dirty state

`NativeApplication` now exposes three additional host contracts:

- `has_unsaved_changes` reports application-owned dirty state without teaching the host document
  policy;
- `handle_lifecycle` receives Resumed, Suspended, and MemoryWarning events;
- `request_close` lets applications persist, ask product-owned questions, veto, or accept native
  window close.

Both CPU and wgpu hosts drive the same contracts. On macOS, `has_unsaved_changes` is mirrored into
winit's native document-edited window state. The editor proof persists its complete session on
suspend, memory warning, and accepted close, while clearing reconstructible text/label caches under
memory pressure.

## Dialog and session paths

`SystemDialogService` now chooses:

- macOS Standard Additions dialogs through `/usr/bin/osascript`;
- Zenity on Linux when available;
- KDialog as the Linux fallback;
- an explicit unavailable error when no supported backend exists.

Open File, Open Folder, Save As, dirty close, save conflict, rename/name entry, replacement, deletion,
and dirty workspace deletion all remain expressed through the existing product-neutral dialog trait.

`StdSessionStore::for_application` now resolves conventional platform state locations:

```text
Linux:  $XDG_STATE_HOME/<app>/editor-session-v2.txt
        or ~/.local/state/<app>/editor-session-v2.txt
macOS:  ~/Library/Application Support/<app>/editor-session-v2.txt
```

Windows has a best-effort `LOCALAPPDATA` path but no official support promise.

## Workspace delivery

`PlatformWorkspaceWatchService` owns desktop selection and fallback:

- Linux retains the existing native-first inotify helper path and polling fallback;
- macOS uses `notify`'s recommended FSEvents watcher;
- unsupported or failed native paths use the deterministic standard-library polling watcher;
- all backends emit the same `WorkspaceWatchEvent` model and reuse existing coalescing and
  incremental-subtree reconciliation.

The workspace crate pins `notify` 8.2.0. Native events still cross a channel and are drained on the UI
thread; watcher callbacks never mutate application or widget state.

## Downstream composition

The new `luna-integration` crate provides:

- validated stable application metadata;
- a typed `DownstreamServices` bundle for file, dialog, workspace, watcher, session, syntax, and
  completion adapters;
- a deterministic platform-support report suitable for diagnostics or About surfaces;
- tests using only existing in-memory/scripted Luna adapters.

This is an example of composition, not a global service locator and not a product rewrite plan.

## macOS packaging

`scripts/package-macos.sh` creates a conventional application bundle for the editor integration proof:

```text
Luna UI Rust Editor Demo.app/
  Contents/
    Info.plist
    MacOS/LunaUIRustEditorDemo
    Resources/Luna-UI-Rust.txt
```

The script supports release/debug profiles, output directory, bundle identifier, display name,
signing identity, and `--skip-build`. It validates the plist, performs ad-hoc signing by default, and
verifies the resulting bundle. This is a development package path; notarization, hardened runtime,
release signing, and distribution policy remain downstream responsibilities.

## “Different” theme

M6 adds the stable preset `different`, displayed as **Different**. It combines milky graphite
surfaces, translucent blueberry chrome, dark graphite text, and a saturated candy-aqua focus color.
The character is inspired by late-1990s and early-2000s translucent desktop hardware and interfaces,
but it is implemented entirely through Luna's five semantic color tokens. No platform widgets,
logos, product assets, or application-specific code are required.

The stable preset order is now:

```text
Luna Dark -> Luna Light -> Amber Monitor -> Green Terminal -> Different -> Luna Dark
```

## Validation

Linux primary gate:

```bash
./scripts/test-m6.sh
```

macOS advisory gate:

```bash
./scripts/test-macos.sh
```

Manual macOS acceptance must cover CPU and Metal rendering, Retina and external displays,
sleep/wake, memory pressure, native edited indication, AppleScript dialogs, Application Support
sessions, FSEvents updates, dead keys, emoji, multi-stage CJK IME, VoiceOver actions, and launch of the
signed `.app` bundle.
