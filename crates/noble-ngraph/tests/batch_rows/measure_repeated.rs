#![allow(
    tigerstyle::ambiguous_params,
    tigerstyle::assertion_density,
    tigerstyle::explicit_defaults,
    tigerstyle::missing_const_fn,
    tigerstyle::no_unwrap,
    tigerstyle::numeric_units,
    tigerstyle::non_trait_imports,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this reporting-only measurement lane compares four evaluation lanes over identical rows, so dense assertions, owner-path imports and checked unwraps are the readable form. Nothing here asserts a timing value."
)]
//! Repeated workload measurements for issue #41, kept separate from shared workload helpers.

use std::hint::black_box;
use std::time::Instant;

use noble_ngraph::batch::{Limits, Prepared};
use noble_ngraph::check;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::sem::Value;

use super::{growth, resident_bytes, rows, values, workload, Row, SIZES};

/// Outputs one row produces.
type Outputs = Vec<Value>;

/// Timed repetitions per lane and size.
const RUNS: usize = 5;

fn per_row(total_ns: u128, size: usize) -> u128 {
    total_ns.checked_div(size.max(1) as u128).unwrap_or(0)
}

/// Every timing sample of one lane at one size, plus first and second run resident growth.
struct Samples {
    totals_ns: Vec<u128>,
    first_row_ns: Vec<u128>,
    growth_first: u64,
    growth_second: u64,
}

impl Samples {
    fn new() -> Self {
        Self {
            totals_ns: Vec::with_capacity(RUNS),
            first_row_ns: Vec::with_capacity(RUNS),
            growth_first: 0,
            growth_second: 0,
        }
    }

    fn record(&mut self, run: usize, total_ns: u128, first_row_ns: u128, grown: u64) {
        match run {
            0 => self.growth_first = grown,
            1 => self.growth_second = grown,
            _ => {}
        }
        self.totals_ns.push(total_ns);
        self.first_row_ns.push(first_row_ns);
    }
}

fn report(lane: &str, size: usize, samples: &Samples) {
    let mut totals = samples.totals_ns.clone();
    totals.sort_unstable();
    let mut firsts = samples.first_row_ns.clone();
    firsts.sort_unstable();
    println!(
        "MEASURE {{\"lane\":\"{lane}\",\"rows\":{size},\"runs\":{RUNS},\"total_ns\":{},\"best_total_ns\":{},\"ns_per_row\":{},\"best_ns_per_row\":{},\"first_row_ns\":{},\"resident_growth_bytes\":{},\"resident_growth_second_bytes\":{}}}",
        totals[RUNS / 2],
        totals[0],
        per_row(totals[RUNS / 2], size),
        per_row(totals[0], size),
        firsts[RUNS / 2],
        samples.growth_first,
        samples.growth_second,
    );
}

fn native(rows: &[Row]) -> ((i64, bool), i64, u128) {
    let mut last = (0, false);
    let mut checksum = 0_i64;
    let mut first_row_ns = 0;
    for (index, (left, right)) in rows.iter().enumerate() {
        let at = (index == 0).then(Instant::now);
        let sum = left.wrapping_add(*right);
        let product = left.wrapping_mul(*right);
        let scaled = sum.wrapping_mul(product);
        let difference = scaled.wrapping_sub(*right);
        last = (difference, difference == sum);
        checksum = checksum.wrapping_add(black_box(difference));
        if let Some(at) = at {
            first_row_ns = at.elapsed().as_nanos();
        }
    }
    (last, checksum, first_row_ns)
}

fn reference_lane(
    accepted: &noble_ngraph::check::CheckedGraph,
    rows: &[Row],
) -> (Samples, Option<Outputs>) {
    let mut samples = Samples::new();
    let mut last = None;
    for run in 0..RUNS {
        let before = resident_bytes();
        let mut first = 0;
        let start = Instant::now();
        for (index, row) in rows.iter().enumerate() {
            let two = [Value::I64(row.0), Value::I64(row.1)];
            let at = Instant::now();
            let outcome = eval::reference(accepted, &two, None, &mut Control::default());
            if index == 0 {
                first = at.elapsed().as_nanos();
            }
            if index + 1 == rows.len() {
                last = Some(outcome.obs);
            }
        }
        samples.record(
            run,
            start.elapsed().as_nanos(),
            first,
            growth(before, resident_bytes()),
        );
    }
    (
        samples,
        last.and_then(|obs: Obs| obs.outputs().map(<[Value]>::to_vec)),
    )
}

fn single_lane(
    compiled: &Compiled,
    workspace: &mut noble_ngraph::compiled::Workspace,
    rows: &[Row],
) -> (Samples, Option<Outputs>) {
    let mut samples = Samples::new();
    let mut last = None;
    for run in 0..RUNS {
        let before = resident_bytes();
        let mut first = 0;
        let start = Instant::now();
        for (index, row) in rows.iter().enumerate() {
            let two = [Value::I64(row.0), Value::I64(row.1)];
            let at = Instant::now();
            let outcome = compiled.evaluate_in(workspace, &two, None, &mut Control::default());
            if index == 0 {
                first = at.elapsed().as_nanos();
            }
            if index + 1 == rows.len() {
                last = Some(outcome.obs);
            }
        }
        samples.record(
            run,
            start.elapsed().as_nanos(),
            first,
            growth(before, resident_bytes()),
        );
    }
    (
        samples,
        last.and_then(|obs: Obs| obs.outputs().map(<[Value]>::to_vec)),
    )
}

fn batch_lane(
    prepared: &mut Prepared<'_>,
    inputs: &[Value],
    size: usize,
    limits: Limits,
) -> (Samples, Option<Outputs>) {
    let mut samples = Samples::new();
    let mut last = None;
    for run in 0..RUNS {
        let before = resident_bytes();
        let start = Instant::now();
        let outcome = prepared.run(size, inputs, limits, None);
        let total = start.elapsed().as_nanos();
        last = outcome
            .reports
            .last()
            .and_then(|report| report.obs.outputs().map(<[Value]>::to_vec));
        samples.record(
            run,
            total,
            per_row(total, size),
            growth(before, resident_bytes()),
        );
    }
    (samples, last)
}

fn native_lane(rows: &[Row]) -> (Samples, Option<Outputs>) {
    let mut samples = Samples::new();
    let mut last = None;
    for run in 0..RUNS {
        let before = resident_bytes();
        let start = Instant::now();
        let (native_result, checksum, first_row_ns) = native(rows);
        let total = start.elapsed().as_nanos();
        last = Some(vec![
            Value::I64(native_result.0),
            Value::Bool(native_result.1),
        ]);
        black_box(checksum);
        samples.record(run, total, first_row_ns, growth(before, resident_bytes()));
    }
    (samples, last)
}

#[test]
#[ignore = "reporting-only measurement lane; run it explicitly with --ignored --nocapture"]
fn timed_workloads() {
    let scheme = super::super::common::IDS;
    let accepted = check::check(&scheme, workload()).expect("the workload checks");
    let compiled = Compiled::new(&scheme, &accepted);
    let mut prepared = Prepared::new(&compiled).expect("the workload is an I64 graph");
    let limits = Limits {
        max_rows: 1 << 20,
        max_output_values: 1 << 21,
        row_fuel: None,
        run_fuel: None,
    };
    let declared = compiled.retained_bytes();
    println!(
        "MEASURE {{\"lane\":\"declared\",\"retained_bytes\":{declared},\"debug_assertions\":{}}}",
        cfg!(debug_assertions)
    );
    let mut workspace = compiled.workspace();
    let warm: Vec<Value> = vec![Value::I64(1), Value::I64(2)];
    let _ = compiled.evaluate_in(&mut workspace, &warm, None, &mut Control::default());
    let _ = prepared.run(1, &warm, limits, None);
    for size in SIZES {
        let rows = rows(size);
        let inputs = values(&rows);
        let (reference, reference_last) = reference_lane(&accepted, &rows);
        report("reference", size, &reference);
        let (single, single_last) = single_lane(&compiled, &mut workspace, &rows);
        report("single", size, &single);
        let (batch, batch_last) = batch_lane(&mut prepared, &inputs, size, limits);
        report("batch", size, &batch);
        let (native, native_last) = native_lane(&rows);
        report("native", size, &native);
        assert_eq!(
            reference_last, single_last,
            "reference and single agree at {size}"
        );
        assert_eq!(batch_last, single_last, "batch and single agree at {size}");
        assert_eq!(
            native_last, single_last,
            "native and single agree at {size}"
        );
        println!("MEASURE {{\"agreement\":{{\"rows\":{size},\"outputs\":{single_last:?}}}}}");
    }
}
