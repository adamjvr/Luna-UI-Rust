# M9.3 — Downstream Product Boundary

M9.3 begins paired Luna/Moth development without turning Luna into Moth's
application layer.

## Governing rule

```text
Moth owns product state and policy.
Luna owns reusable mechanisms and presentation.
The Luna editor demo remains a proof application, not a reusable Moth runtime.
```

The new `luna-integration::DownstreamApplicationProfile` gives downstream
products explicit, validated identities for commands, sessions, and packaged
resources. Luna does not interpret those namespaces or create product policy
from them.

## Moth integration requirements

A Moth consumer must:

- live in an independent repository and Cargo workspace;
- pin Luna through an exact Git submodule gitlink;
- keep Moth core, buffer, editor, document, settings, and workspace models
  independent of Luna UI and native-host crates;
- isolate provisional Luna APIs inside the Moth application/integration layer;
- consume Luna crates through public package APIs;
- never import the Luna editor-demo application;
- keep product commands under a Moth-owned namespace;
- keep session and resource namespaces explicit and product-owned.

## Qualification

```bash
./scripts/test-m9-3.sh
```

This focused gate validates the new integration profile and checks the existing
external downstream consumer against Luna's public APIs. Full workspace
qualification remains authoritative before Git.
