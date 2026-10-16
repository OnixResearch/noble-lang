#!/usr/bin/env bash
set -euo pipefail
. /home/brittonr/.cache/noble-issues/env.sh
export CARGO_HOME=/home/brittonr/.cache/noble-issues/cargo-home/ngraph-cpu-20261016 CARGO_BUILD_JOBS=6
export CARGO_TARGET_DIR=/home/brittonr/.cache/noble-issues/targets/ngraph-cpu-selected-20261016
export RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= RUSTFLAGS= NIX_LOCAL=1
export NIX_CONFIG=$'min-free = 0\nmax-free = 0\nmax-jobs = 1'
nx nice -n10 cargo test --locked --offline -j6 -p noble-ngraph -p noble-ngraph-experiment -- --test-threads=4
nx nice -n10 cargo test --locked --offline -j6 -p noble-cli --test ngraph -- --test-threads=4
nx nice -n10 cargo clippy --locked --offline -j6 -p noble-ngraph -p noble-ngraph-experiment -p noble-cli --all-targets -- -D warnings
nx cargo fmt -p noble-ngraph -p noble-ngraph-experiment -p noble-cli -- --check
nx nice -n10 cargo build --release --locked --offline -j6 -p noble-cli --bin noble
for name in table-boundaries training-refusal; do
 nx nice -n10 rustc --edition 2021 -O -C overflow-checks=yes -C panic=unwind "/home/brittonr/.cache/noble-issues/evidence/ngraph-cpu-20261016/$name.rs" --extern noble_ngraph=/home/brittonr/.cache/noble-issues/targets/ngraph-cpu-selected-20261016/release/deps/libnoble_ngraph-e929b8376ee7de88.rlib --extern noble_ngraph_experiment=/home/brittonr/.cache/noble-issues/targets/ngraph-cpu-selected-20261016/release/deps/libnoble_ngraph_experiment-c539eab08e0d1d57.rlib -L dependency=/home/brittonr/.cache/noble-issues/targets/ngraph-cpu-selected-20261016/release/deps -o "/home/brittonr/.cache/noble-issues/evidence/ngraph-cpu-20261016/$name-selected"
 "/home/brittonr/.cache/noble-issues/evidence/ngraph-cpu-20261016/$name-selected"
done
"$NOBLE_BUN" /home/brittonr/.cache/noble-issues/evidence/ngraph-cpu-20261016/run-cli-smoke.mjs
export CARGO_TARGET_DIR=/home/brittonr/.cache/noble-issues/targets/ngraph-cpu-baseline-selected-20261016
nx nice -n10 cargo test --manifest-path /home/brittonr/.cache/noble-issues/wt/ngraph-cpu-baseline-20261016/Cargo.toml --release --locked --offline -j6 -p noble-ngraph-experiment --test b6_ownership -- --test-threads=4
