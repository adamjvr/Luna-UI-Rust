#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

public_api_version="${LUNA_CARGO_PUBLIC_API_VERSION:-0.52.0}"
semver_checks_version="${LUNA_CARGO_SEMVER_CHECKS_VERSION:-0.49.0}"
nightly="${LUNA_API_NIGHTLY:-nightly-2026-07-10}"

step() {
    printf '==> %s\n' "$1"
}

has_exact_version() {
    local command_name="$1"
    local expected="$2"
    local output
    if ! output="$(cargo +stable "$command_name" --version 2>/dev/null)"; then
        return 1
    fi
    case "$output" in
        *" $expected"*|*" $expected "*) return 0 ;;
        *) return 1 ;;
    esac
}

step "Installing/updating the stable toolchain used to build API review tools"
rustup toolchain install stable --profile minimal

step "Installing pinned rustdoc-JSON nightly: $nightly"
rustup toolchain install "$nightly" --profile minimal

if has_exact_version public-api "$public_api_version"; then
    step "cargo-public-api $public_api_version is already installed"
else
    step "Installing cargo-public-api $public_api_version"
    cargo +stable install cargo-public-api \
        --version "$public_api_version" \
        --locked \
        --force
fi

if has_exact_version semver-checks "$semver_checks_version"; then
    step "cargo-semver-checks $semver_checks_version is already installed"
else
    step "Installing cargo-semver-checks $semver_checks_version"
    cargo +stable install cargo-semver-checks \
        --version "$semver_checks_version" \
        --locked \
        --force
fi

step "Installed API review toolchain"
printf 'stable:               %s\n' "$(rustc +stable --version)"
printf 'snapshot nightly:      %s\n' "$(rustc +"$nightly" --version)"
printf 'cargo-public-api:      %s\n' "$(cargo +stable public-api --version)"
printf 'cargo-semver-checks:   %s\n' "$(cargo +stable semver-checks --version)"
