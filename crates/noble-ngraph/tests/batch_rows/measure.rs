#![allow(
    tigerstyle::ambiguous_params,
    tigerstyle::assertion_density,
    tigerstyle::explicit_defaults,
    tigerstyle::non_trait_imports,
    tigerstyle::missing_const_fn,
    tigerstyle::no_unwrap,
    tigerstyle::numeric_units,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::underscore_in_module_filename,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this reporting-only measurement lane compares four evaluation lanes over identical rows, so dense assertions, owner-path imports and checked unwraps are the readable form. Nothing here asserts a timing value."
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
//! taken.
//!
//! What this lane cannot measure: **allocation counts**. The workspace forbids `unsafe_code`, so no
//! counting global allocator can exist here. The lane reports resident-set growth instead and
//! records that missing instrument rather than inferring an allocation count from register reuse.
//!
//! Run it with `cargo test -p noble-ngraph --test batch_row_reference -- --ignored --nocapture
//! repeated_workloads`.

use std::time::Instant;

use noble_ngraph::batch::{Limits, Prepared};
use noble_ngraph::check;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control};
use noble_ngraph::ir::{Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::sem::Value;

/// One row of the workload: two operands.
type Row = (i64, i64);

/// Outputs one row produces.
type Outputs = Vec<Value>;

pub(super) const SIZES: [usize; 3] = [1, 4096, 65536];

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

fn per_row(total_ns: u128, size: usize) -> u128 {
    total_ns.checked_div(size.max(1) as u128).unwrap_or(0)
}

fn report(lane: &str, size: usize, timed: (u128, u128), grown: u64) {
    let (total_ns, first_row_ns) = timed;
    println!(
        "MEASURE {{\"lane\":\"{lane}\",\"rows\":{size},\"total_ns\":{total_ns},\"ns_per_row\":{},\"first_row_ns\":{first_row_ns},\"resident_growth_bytes\":{grown}}}",
        per_row(total_ns, size)
    );
}

fn native(rows: &[Row]) -> (i64, bool) {
    let mut last = (0, false);
    for (left, right) in rows {
        let sum = left.wrapping_add(*right);
        let product = left.wrapping_mul(*right);
        let scaled = sum.wrapping_mul(product);
        let difference = scaled.wrapping_sub(*right);
        last = (difference, difference == sum);
    }
    last
}

/// The reference interpreter once per row: total, first-row latency, last row's outputs, growth.
fn reference_lane(
    accepted: &noble_ngraph::check::CheckedGraph,
    rows: &[Row],
) -> (u128, u128, Option<Outputs>, u64) {
    let before = resident_bytes();
    let mut last = None;
    let mut first = 0;
    let start = Instant::now();
    for (index, row) in rows.iter().enumerate() {
        let two = [Value::I64(row.0), Value::I64(row.1)];
        let at = Instant::now();
        let outcome = eval::reference(accepted, &two, None, &mut Control::default());
        if index == 0 {
            first = at.elapsed().as_nanos();
        }
        if let Some(outputs) = outcome.obs.outputs() {
            last = Some(outputs.to_vec());
        }
    }
    let total = start.elapsed().as_nanos();
    (total, first, last, growth(before, resident_bytes()))
}

/// The compiled evaluator once per row, reusing one workspace.
fn single_lane(
    compiled: &Compiled,
    workspace: &mut noble_ngraph::compiled::Workspace,
    rows: &[Row],
) -> (u128, u128, Option<Outputs>, u64) {
    let before = resident_bytes();
    let mut last = None;
    let mut first = 0;
    let start = Instant::now();
    for (index, row) in rows.iter().enumerate() {
        let two = [Value::I64(row.0), Value::I64(row.1)];
        let at = Instant::now();
        let outcome = compiled.evaluate_in(workspace, &two, None, &mut Control::default());
        if index == 0 {
            first = at.elapsed().as_nanos();
        }
        if let Some(outputs) = outcome.obs.outputs() {
            last = Some(outputs.to_vec());
        }
    }
    let total = start.elapsed().as_nanos();
    (total, first, last, growth(before, resident_bytes()))
}

/// `batch::Prepared::run` over all rows at once.
fn batch_lane(
    prepared: &mut Prepared<'_>,
    inputs: &[Value],
    size: usize,
    limits: Limits,
) -> (u128, u128, Option<Outputs>, u64) {
    let before = resident_bytes();
    let start = Instant::now();
    let outcome = prepared.run(size, inputs, limits, None);
    let total = start.elapsed().as_nanos();
    let last = outcome
        .reports
        .last()
        .and_then(|report| report.obs.outputs().map(<[Value]>::to_vec));
    let first = per_row(total, size);
    (total, first, last, growth(before, resident_bytes()))
}

#[test]
#[ignore = "reporting-only measurement lane; run it explicitly with --ignored --nocapture"]
fn repeated_workloads() {
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
        let (total, first, reference_last, grown) = reference_lane(&accepted, &rows);
        report("reference", size, (total, first), grown);
        let (total, first, single_last, grown) = single_lane(&compiled, &mut workspace, &rows);
        report("single", size, (total, first), grown);
        let (total, first, batch_last, grown) = batch_lane(&mut prepared, &inputs, size, limits);
        report("batch", size, (total, first), grown);
        let before = resident_bytes();
        let start = Instant::now();
        let (last, equal) = native(&rows);
        let total = start.elapsed().as_nanos();
        report(
            "native",
            size,
            (total, total),
            growth(before, resident_bytes()),
        );
        let native_last = vec![Value::I64(last), Value::Bool(equal)];
        println!(
            "MEASURE {{\"agreement\":{{\"rows\":{size},\"reference\":{reference_last:?},\"single\":{single_last:?},\"batch\":{batch_last:?},\"native\":{native_last:?}}}}}"
        );
    }
}
