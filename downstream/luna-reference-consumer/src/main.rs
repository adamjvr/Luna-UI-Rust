// SPDX-License-Identifier: MPL-2.0

//! Executable entry point for the Luna M8.2 external consumer proof.

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    luna_reference_consumer::run_from_environment()
}
