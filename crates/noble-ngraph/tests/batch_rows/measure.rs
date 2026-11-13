#![allow(
    tigerstyle::ambiguous_params,
    tigerstyle::ambient_env,
    tigerstyle::assertion_density,
    tigerstyle::explicit_defaults,
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::numeric_units,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::underscore_in_module_filename,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this reporting-only harness keeps shared workload and resident-set helpers plus a DHAT-selected process lane together. Its environment is supplied by the pinned runner; checked unwraps, deliberate panic-on-bad-selector and owner-path imports are intentional, and no timing value is asserted."
)]
//! Repeated-workload measurement for the CPU batch entry point (issue #41), reporting only.
//!
//! Four lanes over identical inputs and identical semantics: `reference` (the normative reference
//! interpreter once per row), `single` (the compiled evaluator once per row, reusing one workspace),
//! `batch` (`batch::Prepared::run` over all rows) and `native` (straightforward `i64` wrapping
//! arithmetic, the host-native floor).
//!
//! The continuation criteria, every size and every threshold used here are frozen in
//! `verification/ngraph/batch-continuation-criteria.md` **before** any number in this lane was
//! taken. The frozen sizes are 1, 4096 and 65536 rows; the re-measurement below adds 16, 64, 256 and
//! 1024 rows so the batch/single crossover can be stated, and each lane is run `RUNS` times so a
//! median is reported instead of one sample. Per lane and size the lane reports the median and best
//! total, the derived per-row cost, the first row's own time (`first_row_ns`; for `batch`, which
//! does not report intermediate timing, it is the run's mean row time), the resident growth of the
//! first run and of the second identical run. The native lane measures its first row with one
//! `Instant` around that iteration, like the other per-row lanes.
//!
//! The `native` lane keeps every row's value alive: each row's difference is folded into a checksum
//! that reaches the caller through `black_box`, so the optimiser cannot delete the loop body and
//! report a floor of 0 ns/row. That live checksum is *not* one of the graph's outputs; the last
//! row's own values are compared across lanes.
//!
//! Allocation counts are measured separately: the workspace forbids `unsafe_code`, so no counting
//! global allocator can exist here. `verification/ngraph/batch-allocation.mjs` profiles the
//! ignored `allocation_lane` out of process with Valgrind's DHAT.
//!
//! Run it through `verification/ngraph/batch-measure.mjs`, or with
//! `cargo test --release -p noble-ngraph --test batch_row_reference -- --ignored --nocapture
//! measure::repeated::timed_workloads`.

use noble_ngraph::batch::{Limits, Prepared};
use noble_ngraph::check;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control};
use noble_ngraph::ir::{Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::sem::Value;

/// One row of the workload: two operands.
pub(super) type Row = (i64, i64);

/// The sizes the post-decision re-measurement reports: the three frozen decision sizes (1, 4096,
/// 65536 rows) plus 16, 64, 256 and 1024 so the batch/single crossover can be stated.
pub(super) const SIZES: [usize; 7] = [1, 16, 64, 256, 1024, 4096, 65536];

/// The sizes the retained 2026-11-07 breakdown receipt covers; kept separate so that receipt's
/// table stays reproducible without a new breakdown measurement.
pub(super) const BREAKDOWN_SIZES: [usize; 3] = [1, 4096, 65536];

/// Five arithmetic nodes over two inputs and two outputs, so one row does more than one step while
/// staying small enough to measure per-row cost honestly.
pub(super) fn workload() -> Graph {
    let scheme = super::common::IDS;
    let subject = Subject::authored(&scheme, "batch-measure");
    let mut builder = GraphBuilder::new(SubProfile::I64, subject);
    let a = builder.input(ScalarType::I64);
    let b = builder.input(ScalarType::I64);
    let sum = builder.op(Op::I64Add(a, b));
    let product = builder.op(Op::I64Mul(a, b));
    let scaled = builder.op(Op::I64Mul(sum, product));
    let difference = builder.op(Op::I64Sub(scaled, b));
    let equal = builder.op(Op::I64Eq(difference, sum));
    builder.output(difference);
    builder.output(equal);
    builder.finish()
}

/// Operands that stress the wrap boundaries without making every row identical.
pub(super) fn rows(count: usize) -> Vec<Row> {
    const BOUNDARY: [i64; 7] = [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX];
    let mut built = Vec::with_capacity(count);
    let mut left = 0;
    let mut right = 0;
    while built.len() < count {
        built.push((BOUNDARY[left], BOUNDARY[right]));
        left = next(left);
        if built.len().checked_rem(2) == Some(0) {
            right = next(right);
        }
    }
    built
}

/// The next index in the boundary cycle, without a division or remainder.
fn next(index: usize) -> usize {
    if index + 1 == 7 {
        0
    } else {
        index + 1
    }
}

pub(super) fn values(rows: &[Row]) -> Vec<Value> {
    let mut values = Vec::with_capacity(rows.len() * 2);
    for (left, right) in rows {
        values.push(Value::I64(*left));
        values.push(Value::I64(*right));
    }
    values
}

/// Resident set size in bytes, from the kernel's own report.
pub(super) fn resident_bytes() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages = text.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    Some(pages * 4096)
}

pub(super) fn growth(before: Option<u64>, after: Option<u64>) -> u64 {
    match (before, after) {
        (Some(before), Some(after)) => after.saturating_sub(before),
        _ => 0,
    }
}

#[path = "measure_repeated.rs"]
mod repeated;

fn output_count(report: &noble_ngraph::eval::Report) -> usize {
    report.obs.outputs().map_or(0, <[Value]>::len)
}

#[test]
#[ignore = "allocation-count lane; run it under Valgrind DHAT through verification/ngraph/batch-allocation.mjs"]
fn allocation_lane() {
    let lane = std::env::var("NOBLE_BATCH_ALLOC_LANE").expect("NOBLE_BATCH_ALLOC_LANE is set");
    let count: usize = std::env::var("NOBLE_BATCH_ALLOC_ROWS")
        .expect("NOBLE_BATCH_ALLOC_ROWS is set")
        .parse()
        .expect("NOBLE_BATCH_ALLOC_ROWS is a row count");
    let scheme = super::common::IDS;
    let accepted = check::check(&scheme, workload()).expect("the workload checks");
    let compiled = Compiled::new(&scheme, &accepted);
    let mut prepared = Prepared::new(&compiled).expect("the workload is an I64 graph");
    let limits = Limits {
        max_rows: 1 << 20,
        max_output_values: 1 << 21,
        row_fuel: None,
        run_fuel: None,
    };
    let operands = rows(count);
    let mut outputs: usize = 0;
    match lane.as_str() {
        "reference" => {
            for row in &operands {
                let two = [Value::I64(row.0), Value::I64(row.1)];
                let report = eval::reference(&accepted, &two, None, &mut Control::default());
                outputs = outputs.saturating_add(output_count(&report));
            }
        }
        "single" => {
            let mut workspace = compiled.workspace();
            for row in &operands {
                let two = [Value::I64(row.0), Value::I64(row.1)];
                let report =
                    compiled.evaluate_in(&mut workspace, &two, None, &mut Control::default());
                outputs = outputs.saturating_add(output_count(&report));
            }
        }
        "batch" => {
            let inputs = values(&operands);
            let outcome = prepared.run(count, &inputs, limits, None);
            assert_eq!(outcome.stop, None, "an unbounded batch run completes");
            for report in &outcome.reports {
                outputs = outputs.saturating_add(output_count(report));
            }
        }
        other => panic!("unknown lane {other}"),
    }
    println!("ALLOC {{\"lane\":\"{lane}\",\"rows\":{count},\"outputs\":{outputs}}}");
    std::hint::black_box(outputs);
}
