# Public API and Compatibility Policy

M7 introduces an explicit compatibility inventory without pretending the pre-1.0 workspace is
already frozen. `api/public-api.toml` and `luna_qualification::CRATE_CONTRACTS` classify every
public library package as stable, provisional, or internal.

## Stability tiers

- **Stable** — foundational types and behavior intended to remain source-compatible across ordinary
  `0.1.x` releases. A breaking change requires an explicit roadmap decision, migration notes, and a
  versioning decision.
- **Provisional** — public because downstream applications need the boundary, but still subject to
  focused revision before 1.0. Changes require documentation and fixture updates rather than silent
  churn.
- **Internal** — workspace implementation detail. Downstream applications should not import it.

Rust visibility alone does not communicate this project commitment; the checked-in manifest does.
The API audit fails when a public library appears in Cargo metadata but not in the contract inventory.

## Error contracts

Public errors keep descriptive `Display` output for people and implement
`luna_core::CodedError` for stable machine-readable identity. Consumers should branch on
`ErrorCode`, not parse prose. Public errors likely to grow use `#[non_exhaustive]` where introducing
that marker does not create avoidable churn.

## Semver qualification

The blocking gate checks metadata, documentation, strict lints, examples, tests, and the explicit
contract inventory. `cargo-semver-checks` is advisory until the project has a retained published API
baseline. When installed, `scripts/test-m7.sh` compares against `LUNA_SEMVER_BASELINE`, defaulting to
the committed M6 baseline.

## Documentation expectations

Every public item remains covered by `missing_docs = deny`. Public functions that can fail document
`# Errors`; functions with special panic behavior must document `# Panics`. Examples must use the
same product-neutral boundaries recommended to downstream users.
