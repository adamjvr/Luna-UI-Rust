# Contributing

Luna-UI-Rust is an editor-class foundation, so correctness and stable boundaries matter more than
rapid dependency accumulation.

Before submitting changes:

1. Keep product behavior out of Luna crates.
2. Derive paint, hit testing, and accessibility from shared geometry.
3. Return typed errors at recoverable boundaries; do not use `unwrap`, `expect`, or `panic` in
   production code.
4. Keep unsafe code forbidden until a narrowly reviewed platform boundary proves it unavoidable.
5. Document every public item and explain architectural decisions, not obvious syntax.
6. Run `./scripts/validate.sh`.

Add dependencies only in the narrowest adapter crate that needs them. A window backend does not
belong in `luna-core`; a GPU backend does not belong in `luna-ui`; product commands do not belong in
any Luna crate.
