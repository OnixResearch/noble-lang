#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::numeric_units,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this reporting-only lane times the one-time and per-run costs of the batch entry point beside the warm run, so checked unwraps, owner-path imports and plain nanosecond counters are the readable form. Nothing here asserts a timing value."
)]
//! Cold, staging, output-copy and byte breakdown of the CPU batch entry point (issue #41, item 6),
//! reporting only, over the workload and rows of `measure.rs`.
//!
//! One-time costs per graph: checking, compiling and preparing, each timed on its first call in
//! the process and as the median of five fresh calls. Per batch size: staging the flat input
//! values, the warm run, and copying every row's outputs into one caller-owned vector, each the
//! median of five. Bytes are declared, not measured: the compiled code's retained bytes, one
//! workspace's scratch bytes, and the input, report and copied-output storage each size needs.
//! Allocation counts stay unavailable, because the workspace forbids `unsafe_code`; resident
//! growth across the first run at each size is reported instead.
//!
//! Run it through `verification/ngraph/batch-breakdown.mjs`, or with `cargo test --release -p
//! noble-ngraph --test batch_row_reference -- --ignored --nocapture cold_staging_and_bytes`.

use std::hint::black_box;
use std::mem::size_of;
use std::time::Instant;

use noble_ngraph::batch::{Limits, Outcome, Prepared};
use noble_ngraph::check;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::Report;
use noble_ngraph::sem::Value;

use super::measure::{growth, resident_bytes, rows, values, workload, BREAKDOWN_SIZES};

const RUNS: usize = 5;

/// The first of `RUNS` timings of `work`, and their median.
fn timed(mut work: impl FnMut()) -> (u128, u128) {
    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        work();
        samples.push(start.elapsed().as_nanos());
    }
    let first = samples[0];
    samples.sort_unstable();
    (first, samples[RUNS / 2])
}

/// The bytes `count` values of `T` occupy.
fn bytes<T>(count: usize) -> u64 {
    u64::try_from(count * size_of::<T>()).expect("a byte count fits u64")
}

/// Every row's outputs, copied into one caller-owned vector in row order.
fn copied_outputs(outcome: &Outcome, capacity: usize) -> Vec<Value> {
    let mut copied = Vec::with_capacity(capacity);
    for report in &outcome.reports {
        copied.extend_from_slice(report.obs.outputs().expect("every row is normal"));
    }
    copied
}

fn cold(scheme: &dyn noble_ngraph::identity::IdentityScheme) {
    let mut graphs = (0..RUNS)
        .map(|_| workload())
        .collect::<Vec<_>>()
        .into_iter();
    let (check_first, check_median) = timed(|| {
        black_box(check::check(scheme, graphs.next().expect("one graph per run")).expect("checks"));
    });
    let accepted = check::check(scheme, workload()).expect("the workload checks");
    let (compile_first, compile_median) = timed(|| {
        black_box(Compiled::new(scheme, &accepted));
    });
    let compiled = Compiled::new(scheme, &accepted);
    let (prepare_first, prepare_median) = timed(|| {
        black_box(Prepared::new(&compiled).expect("an I64 graph"));
    });
    println!(
        "MEASURE {{\"lane\":\"cold\",\"check\":{{\"first_ns\":{check_first},\"median_ns\":{check_median}}},\"compile\":{{\"first_ns\":{compile_first},\"median_ns\":{compile_median}}},\"prepare\":{{\"first_ns\":{prepare_first},\"median_ns\":{prepare_median}}},\"retained_bytes\":{},\"workspace_scratch_bytes\":{}}}",
        compiled.retained_bytes(),
        compiled.scratch_bytes(),
    );
}

fn per_size(prepared: &mut Prepared<'_>, size: usize, limits: Limits) {
    let rows = rows(size);
    let (_, staging) = timed(|| {
        black_box(values(&rows));
    });
    let inputs = values(&rows);
    let before = resident_bytes();
    let outcome = prepared.run(size, &inputs, limits, None);
    let grown = growth(before, resident_bytes());
    assert_eq!(outcome.reports.len(), size, "every row ran");
    let outputs = outcome.reports[0]
        .obs
        .outputs()
        .expect("every row is normal")
        .len();
    let (_, run) = timed(|| {
        black_box(prepared.run(size, &inputs, limits, None));
    });
    let (_, copy) = timed(|| {
        black_box(copied_outputs(&outcome, size * outputs));
    });
    assert_eq!(
        copied_outputs(&outcome, size * outputs).len(),
        size * outputs,
        "every output copied"
    );
    println!(
        "MEASURE {{\"lane\":\"size\",\"rows\":{size},\"staging_median_ns\":{staging},\"run_median_ns\":{run},\"output_copy_median_ns\":{copy},\"input_bytes\":{},\"report_bytes\":{},\"report_output_bytes\":{},\"copied_output_bytes\":{},\"first_run_resident_growth_bytes\":{grown}}}",
        bytes::<Value>(inputs.len()),
        bytes::<Report>(size),
        bytes::<Value>(size * outputs),
        bytes::<Value>(size * outputs),
    );
}

#[test]
#[ignore = "reporting-only measurement lane; run it explicitly with --ignored --nocapture"]
fn cold_staging_and_bytes() {
    let scheme = super::common::IDS;
    cold(&scheme);
    let accepted = check::check(&scheme, workload()).expect("the workload checks");
    let compiled = Compiled::new(&scheme, &accepted);
    let mut prepared = Prepared::new(&compiled).expect("the workload is an I64 graph");
    let limits = Limits {
        max_rows: 1 << 20,
        max_output_values: 1 << 21,
        row_fuel: None,
        run_fuel: None,
    };
    let warm = values(&rows(1));
    black_box(prepared.run(1, &warm, limits, None));
    for size in BREAKDOWN_SIZES {
        per_size(&mut prepared, size, limits);
    }
}
