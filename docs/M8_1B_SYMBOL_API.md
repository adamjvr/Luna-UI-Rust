# M8.1b Symbol-Level Public API Qualification

M8.1b completes the symbol-level portion of the M8.1 compatibility cycle. M8.1a established the
product-neutral clipboard boundary and classified the crate-level addition. M8.1b now captures and
compares the public items exported by every Luna library crate.

## Toolchain

The phase pins:

- `cargo-public-api` 0.52.0;
- `cargo-semver-checks` 0.49.0;
- `nightly-2026-07-10` for rustdoc JSON snapshot generation;
- the ordinary current stable toolchain for `cargo-semver-checks`.

The project's supported MSRV remains Rust 1.97.1. The review toolchains do not change the toolchain
used to compile or qualify Luna itself.

## Snapshot model

`scripts/capture-public-api-snapshots.py` creates two complete snapshot directories:

- `m7.0.1`, generated from accepted commit
  `e696df0cedaeda7ac5c0892cf8f709f8325eff8b` in a detached temporary Git worktree;
- `m8.1b`, generated from the current source tree.

Each directory contains one simplified `cargo-public-api` text file per public crate, a JSON manifest,
and SHA-256 checksums. Baseline and current snapshots use the same pinned nightly and
`cargo-public-api` version.

Pre-commit snapshots belong under `/tmp`. Authoritative checked-in snapshots must be regenerated from
the exact committed implementation tree so their manifest commit is accurate.

## Classification policy

`scripts/compare-public-api-snapshots.py` rejects:

- any symbol difference absent from `api/compatibility/m8.1-symbol-policy.json`;
- every change classified as accidental;
- every stable symbol removal not classified as intentionally breaking;
- stale policy entries that no longer correspond to an actual difference.

The initial M8.1b policy expects the M7 public crates to remain symbol-identical and classifies only the
new provisional `luna-clipboard` crate as compatible and additive.

The comparator emits:

- `api-symbol-diff.json`;
- `API_SYMBOL_DIFF.md`;
- one unified diff for every changed crate.

## Stable-crate semver review

`scripts/run-semver-review.py` runs `cargo-semver-checks` against the accepted M7 commit for every
crate marked stable in both the M7 and current contract inventories. The provisional clipboard crate
is not treated as an accepted M7 stable surface.

Each crate receives a retained log plus one machine-readable `semver-summary.json`.

## Gate

Run the phase through the safe child-shell wrapper:

```bash
LUNA_API_SNAPSHOT_ROOT=/tmp/luna-m8.1b-precommit/snapshots \
LUNA_API_REPORT_ROOT=/tmp/luna-m8.1b-precommit/evidence \
LUNA_EVIDENCE_DIR=/tmp/luna-m8.1b-precommit/evidence \
LUNA_M8_EVIDENCE_NAME=m8.1b-precommit \
./run-luna-safe.sh ./scripts/test-m8-1b.sh
```

The script re-runs the complete M8.1a gate before snapshot capture, classification, semver review, and
contract validation.

## Acceptance boundary

M8.1b is accepted only when:

1. the complete M8.1a gate remains green;
2. every snapshot checksum verifies;
3. every actual symbol difference has an explicit non-accidental classification;
4. no stable symbol removal is accepted as merely compatible;
5. every shared stable crate passes `cargo-semver-checks`;
6. a human reviews every generated diff and semver log;
7. authoritative snapshots and evidence are regenerated from the exact committed implementation;
8. `scripts/accept-m8-1b.py` records the completed review.
