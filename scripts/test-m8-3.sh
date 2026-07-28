#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# Complete M8.3 repeated long-session qualification.
# Generated reports and packages remain below ignored dist/m8.3.
# This child script never closes the terminal that launches it.

set -Eeuo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
consumer_root="$repo_root/downstream/luna-reference-consumer"
consumer_manifest="$consumer_root/Cargo.toml"
output_root="$repo_root/dist/m8.3"
package_root="$output_root/package"
extracted_root="$output_root/extracted"
workspace_root="$output_root/workspace"
report_one="$output_root/long-session-run-1.json"
report_two="$output_root/long-session-run-2.json"
normalized_one="$output_root/long-session-run-1.normalized.json"
normalized_two="$output_root/long-session-run-2.normalized.json"
cycles="${LUNA_M8_3_CYCLES:-64}"
package_loops="${LUNA_M8_3_PACKAGE_LOOPS:-4}"
temporary_root="$(mktemp -d -t luna-m8.3-XXXXXXXX)"

on_exit() {
    status=$?
    rm -rf -- "$temporary_root"
    if [[ "$status" -ne 0 ]]; then
        printf '[FAIL] M8.3 qualification stopped with status %s.\n' "$status" >&2
    fi
    printf '[SAFE] Your terminal remains open.\n'
}
trap on_exit EXIT

step() {
    printf '\n============================================================\n'
    printf '==> %s\n' "$1"
    printf '============================================================\n'
}

fail() {
    printf '[FAIL] %s\n' "$1" >&2
    return 1
}

for command_name in cargo python3 unzip zip sha256sum cmp git find grep tee; do
    command -v "$command_name" >/dev/null 2>&1 ||
        fail "Required command is missing: $command_name"
done

if [[ ! "$cycles" =~ ^[0-9]+$ ]] || ((cycles < 1 || cycles > 512)); then
    fail "LUNA_M8_3_CYCLES must be an integer from 1 through 512."
fi
if [[ ! "$package_loops" =~ ^[0-9]+$ ]] || ((package_loops < 1 || package_loops > 32)); then
    fail "LUNA_M8_3_PACKAGE_LOOPS must be an integer from 1 through 32."
fi

cd -- "$repo_root"

step "Formatting the applied M8.3 Rust source"
cargo fmt --all

step "Verifying M8.2 acceptance and M8.3 candidate documentation"
grep -qF "## M8.2 external downstream consumer acceptance" docs/CURRENT_STATUS.md
grep -qF "## M8.3 repeated long-session qualification candidate" docs/CURRENT_STATUS.md
grep -qF "## M8.3 repeated long-session acceptance" docs/RELEASE_CHECKLIST.md
grep -qF "# M8.3 Repeated Long-Session Qualification" docs/M8_3_LONG_SESSION_QUALIFICATION.md
grep -qF "M8.3 deterministic repeated long-session qualification" CHANGELOG.md

step "Re-running the accepted M8.2 downstream qualification"
bash "$repo_root/scripts/test-m8-2.sh"

step "Building the private M8.3 qualification binary"
cargo build \
    --release \
    -p luna-ui-rust-qualification \
    --bin luna-ui-rust-m8-3-long-session

step "Preparing ignored M8.3 package and workspace fixtures"
rm -rf -- "$output_root"
mkdir -p -- "$package_root" "$extracted_root" "$workspace_root/src"
printf 'fn main() { println!("m8.3 workspace fixture"); }\n' > "$workspace_root/src/main.rs"
printf '# M8.3 workspace fixture\n' > "$workspace_root/README.md"

"$consumer_root/scripts/package-linux.sh" \
    --output "$package_root" \
    --skip-build

archive="$(find "$package_root" -maxdepth 1 -type f -name 'Luna-Reference-Consumer-linux-*.zip' -print -quit)"
[[ -n "$archive" ]] || fail "The M8.3 downstream package ZIP was not created."
(
    cd -- "$package_root"
    sha256sum -c "$(basename -- "$archive").sha256"
)
unzip -q "$archive" -d "$extracted_root"
extracted="$extracted_root/Luna-Reference-Consumer"
extracted_binary="$extracted/bin/luna-reference-consumer"
source_resource_root="$consumer_root/resources"
extracted_resource_root="$extracted/share/org.lunaui.ReferenceConsumer"
[[ -x "$extracted_binary" ]] || fail "The extracted consumer executable is missing."
[[ -f "$extracted_resource_root/welcome.txt" ]] || fail "The extracted resource is missing."

step "Repeating source-tree and extracted-package self-tests"
for ((iteration = 1; iteration <= package_loops; iteration++)); do
    source_output="$output_root/source-self-test-$iteration.txt"
    extracted_output="$output_root/extracted-self-test-$iteration.txt"

    LUNA_RESOURCE_ROOT="$source_resource_root" \
    XDG_STATE_HOME="$output_root/source-state-$iteration" \
    cargo run \
        --manifest-path "$consumer_manifest" \
        --release -- \
        --self-test \
        --workspace "$workspace_root" |
    tee "$source_output"
    grep -q '^m8_2_self_test=passed$' "$source_output"

    unrelated="$temporary_root/unrelated-working-directory-$iteration"
    mkdir -p -- "$unrelated"
    (
        cd -- "$unrelated"
        HOME="$output_root/extracted-home-$iteration" \
        XDG_STATE_HOME="$output_root/extracted-state-$iteration" \
        "$extracted_binary" \
            --self-test \
            --workspace "$workspace_root"
    ) | tee "$extracted_output"
    grep -q '^m8_2_self_test=passed$' "$extracted_output"
done

step "Running the deterministic long-session harness twice"
for report in "$report_one" "$report_two"; do
    cargo run \
        --release \
        -p luna-ui-rust-qualification \
        --bin luna-ui-rust-m8-3-long-session -- \
        --cycles "$cycles" \
        --output "$report" \
        --resource-root "$source_resource_root" \
        --resource-root "$extracted_resource_root"
done

step "Validating report structure, limits, and deterministic repeatability"
python3 - \
    "$report_one" \
    "$report_two" \
    "$normalized_one" \
    "$normalized_two" \
    "$cycles" \
    "$package_loops" <<'PY'
import json
import pathlib
import sys

first_path = pathlib.Path(sys.argv[1])
second_path = pathlib.Path(sys.argv[2])
first_normalized_path = pathlib.Path(sys.argv[3])
second_normalized_path = pathlib.Path(sys.argv[4])
expected_cycles = int(sys.argv[5])
package_loops = int(sys.argv[6])

expected_workloads = [
    "document_lifecycle",
    "large_text_cache",
    "pane_tab_cycles",
    "workspace_watcher_bursts",
    "session_round_trips",
    "render_lifecycle_transitions",
    "resource_loading",
]


def load(path: pathlib.Path) -> dict:
    report = json.loads(path.read_text(encoding="utf-8"))
    if report.get("schema") != "luna-m8.3-long-session-v1":
        raise SystemExit(f"unexpected M8.3 schema in {path}")
    if report.get("passed") is not True:
        raise SystemExit(f"M8.3 report did not pass: {path}")
    if report.get("cycles") != expected_cycles:
        raise SystemExit(f"unexpected cycle count in {path}")
    workloads = report.get("workloads")
    if not isinstance(workloads, list):
        raise SystemExit(f"workloads are missing from {path}")
    names = [workload.get("name") for workload in workloads]
    if names != expected_workloads:
        raise SystemExit(f"unexpected workload order in {path}: {names!r}")
    for workload in workloads:
        metrics = workload.get("metrics", {})
        limits = workload.get("limits", {})
        if not isinstance(metrics, dict) or not isinstance(limits, dict):
            raise SystemExit(f"invalid metric maps in {path}")
        for key, maximum in limits.items():
            observed = metrics.get(key)
            if observed is not None and observed > maximum:
                raise SystemExit(
                    f"{workload['name']} metric {key} exceeded {maximum}: {observed}"
                )
    return report


def normalized(report: dict) -> dict:
    result = dict(report)
    result.pop("diagnostic_total_micros", None)
    workloads = []
    for workload in result["workloads"]:
        item = dict(workload)
        item.pop("diagnostic_micros", None)
        workloads.append(item)
    result["workloads"] = workloads
    return result

first = load(first_path)
second = load(second_path)
first_normalized = normalized(first)
second_normalized = normalized(second)
first_normalized_path.write_text(
    json.dumps(first_normalized, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
second_normalized_path.write_text(
    json.dumps(second_normalized, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
if first_normalized != second_normalized:
    raise SystemExit("M8.3 deterministic reports differ after removing timing diagnostics")

resource_metrics = first_normalized["workloads"][-1]["metrics"]
expected_loads = expected_cycles * 2
if resource_metrics.get("loads") != expected_loads:
    raise SystemExit(
        f"resource workload expected {expected_loads} loads, got {resource_metrics.get('loads')}"
    )

print(f"[ OK ] {len(expected_workloads)} deterministic workloads passed twice")
print(f"[ OK ] {package_loops} source and {package_loops} extracted self-tests passed")
print("[ OK ] normalized reports are byte-for-byte deterministic")
PY
cmp --silent "$normalized_one" "$normalized_two"

git diff --check

step "M8.3 automated long-session qualification passed"
printf 'Cycles:                    %s\n' "$cycles"
printf 'Package loops per mode:    %s\n' "$package_loops"
printf 'First report:              %s\n' "$report_one"
printf 'Second report:             %s\n' "$report_two"
printf 'Normalized report:         %s\n' "$normalized_one"
printf 'Package ZIP:               %s\n' "$archive"
printf '[PASS] deterministic counts, capacities, high-water marks, and package loops passed\n'
