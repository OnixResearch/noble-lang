#!/usr/bin/env bash
set -euo pipefail
. /home/brittonr/.cache/noble-issues/env.sh
export CARGO_HOME=/home/brittonr/.cache/noble-issues/cargo-home/ngraph-cpu-20261016
export CARGO_BUILD_JOBS=6 RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= RUSTFLAGS= NIX_LOCAL=1
export NIX_CONFIG=$'min-free = 0\nmax-free = 0\nmax-jobs = 1'
export CARGO_TARGET_DIR=/home/brittonr/.cache/noble-issues/targets/ngraph-cpu-baseline-selected-20261016
nx nice -n10 cargo build --manifest-path /home/brittonr/.cache/noble-issues/wt/ngraph-cpu-baseline-20261016/Cargo.toml --release --locked --offline -j6 -p noble-ngraph-experiment --bin ngraph-phase-c
for n in 1 2 3; do
 nice -n10 taskset -c 2 /home/brittonr/.cache/noble-issues/targets/ngraph-cpu-baseline-selected-20261016/release/ngraph-phase-c --out "/home/brittonr/.cache/noble-issues/evidence/ngraph-cpu-20261016/selected-baseline-$n.json" --source-rev f03aa4d1a4d8612721fbed9a1577ae65befffa93 --toolchain 'rustc 1.96.0-nightly (ac7f9ec7d 2026-03-20)' --pinned-cpu 2
done
nice -n10 taskset -c 2 /home/brittonr/.cache/noble-issues/targets/ngraph-cpu-baseline-selected-20261016/release/ngraph-phase-c --phase-profile true --out /home/brittonr/.cache/noble-issues/evidence/ngraph-cpu-20261016/selected-baseline-profile.json --source-rev f03aa4d1a4d8612721fbed9a1577ae65befffa93 --toolchain 'rustc 1.96.0-nightly (ac7f9ec7d 2026-03-20)' --pinned-cpu 2
export CARGO_TARGET_DIR=/home/brittonr/.cache/noble-issues/targets/ngraph-cpu-selected-20261016
nx nice -n10 cargo build --manifest-path /home/brittonr/.cache/noble-issues/wt/ngraph-cpu-20261016/Cargo.toml --release --locked --offline -j6 -p noble-ngraph-experiment --bin ngraph-phase-c
for n in 1 2 3; do
 nice -n10 taskset -c 2 /home/brittonr/.cache/noble-issues/targets/ngraph-cpu-selected-20261016/release/ngraph-phase-c --out "/home/brittonr/.cache/noble-issues/evidence/ngraph-cpu-20261016/selected-optimized-$n.json" --source-rev 5ca3665e895d51f74a38575eb5a546a8d838058c --toolchain 'rustc 1.96.0-nightly (ac7f9ec7d 2026-03-20)' --pinned-cpu 2
done
nice -n10 taskset -c 2 /home/brittonr/.cache/noble-issues/targets/ngraph-cpu-selected-20261016/release/ngraph-phase-c --phase-profile true --out /home/brittonr/.cache/noble-issues/evidence/ngraph-cpu-20261016/selected-optimized-profile.json --source-rev 5ca3665e895d51f74a38575eb5a546a8d838058c --toolchain 'rustc 1.96.0-nightly (ac7f9ec7d 2026-03-20)' --pinned-cpu 2
printf 'selected comparison complete\n'
