# Linux Development Packaging

M7 adds a relocatable development bundle, not a distro-native stable release.

```bash
./scripts/package-linux.sh
```

The default output is:

```text
dist/linux/Luna-UI-Rust-EditorDemo/
dist/linux/Luna-UI-Rust-EditorDemo.tar.gz
dist/linux/Luna-UI-Rust-EditorDemo.tar.gz.sha256
```

The bundle contains the editor-demo executable, a freedesktop Desktop Entry, AppStream metadata,
MPL-2.0 license, project README, and `EDITOR_DEMO_COMMANDS.md`. When installed, application resources
belong below `share/org.lunaui.EditorDemo/`; `luna_integration::ResourceLocator` can resolve the same
relative resources from development and packaged layouts.

Use `--debug`, `--output DIR`, or `--skip-build` for qualification workflows. The Desktop Entry
intentionally stays within the conservative 1.0 key set accepted by the validator shipped with the
Linux primary platform; newer optional hints do not justify breaking development packaging on older
distributions. If available, `desktop-file-validate` and `appstreamcli` run automatically. Their
absence is reported but does not make the Rust gate depend on distro-specific developer packages.
The tarball uses a stable file
order, normalized ownership and timestamps, and receives a neighboring SHA-256 file so identical
source and `SOURCE_DATE_EPOCH` inputs can be compared directly.

Verify the archive from its output directory:

```bash
cd dist/linux
sha256sum -c Luna-UI-Rust-EditorDemo.tar.gz.sha256
```
