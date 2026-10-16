#!/usr/bin/env bash
# Reproduce only the final selected lane. Invoke under the campaign cargo-A lock.
set -euo pipefail
. /home/brittonr/.cache/noble-issues/env.sh
export CARGO_HOME=/home/brittonr/.cache/noble-issues/cargo-home/ngraph-cpu-20261016
export CARGO_TARGET_DIR=/home/brittonr/.cache/noble-issues/targets/ngraph-cpu-selected-20261016
export CARGO_BUILD_JOBS=6 RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= RUSTFLAGS= NIX_LOCAL=1
export NIX_CONFIG=$'min-free = 0\nmax-free = 0\nmax-jobs = 1'
nx nice -n10 cargo build --manifest-path /home/brittonr/.cache/noble-issues/wt/ngraph-cpu-20261016/Cargo.toml --release --locked --offline -j6 -p noble-ngraph-experiment --bin ngraph-phase-c
for n in 1 2 3; do
 nice -n10 taskset -c 2 /home/brittonr/.cache/noble-issues/targets/ngraph-cpu-selected-20261016/release/ngraph-phase-c --out "/home/brittonr/.cache/noble-issues/evidence/ngraph-cpu-20261016/selected-final-$n.json" --source-rev 7e1de8f1eccdece595dea295a6e00a14c93b2e62 --toolchain 'rustc 1.96.0-nightly (ac7f9ec7d 2026-03-20)' --pinned-cpu 2
done
nice -n10 taskset -c 2 /home/brittonr/.cache/noble-issues/targets/ngraph-cpu-selected-20261016/release/ngraph-phase-c --phase-profile true --out /home/brittonr/.cache/noble-issues/evidence/ngraph-cpu-20261016/selected-final-profile.json --source-rev 7e1de8f1eccdece595dea295a6e00a14c93b2e62 --toolchain 'rustc 1.96.0-nightly (ac7f9ec7d 2026-03-20)' --pinned-cpu 2
