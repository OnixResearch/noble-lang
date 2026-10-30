#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::explicit_defaults,
    tigerstyle::non_trait_imports,
    tigerstyle::assertion_density,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::ambiguous_params,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this harness builds one checked graph, runs it as a batch and compares every row with independent single-row runs, so checked unwraps, default controls and dense assertions are the readable form."
)]
//! The graph-backed batch (`noble_ngraph::batch::Prepared`) compared row by row with independent
//! single-row runs of the same graph (issue #41). Fuel, cancellation and reuse are in `budgets`.
//!
//! Every row of a run must carry exactly the report a fresh compiled evaluation of that row gives,
//! the observation the reference interpreter gives, and, for rows that complete, the wrapping
//! results computed here. Rows that fail a guard or are refused are interleaved with ordinary rows,
//! so a value, failed guard or register that leaked from one row into the next would show up as a
//! difference.

use std::cell::RefCell;

use noble_ngraph::batch::{Limits, Outcome, Prepared, Refusal, Stop};
use noble_ngraph::check::CheckedGraph;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs, Report};
use noble_ngraph::ir::{Fact, Graph, GraphBuilder, Literal, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::sem::Value;

/// Bounds that admit every run in this file unless a test lowers them.
pub(super) const OPEN: Limits = Limits {
    max_rows: 1024,
    max_output_values: 4096,
    row_fuel: None,
    run_fuel: None,
};

/// Two inputs, four outputs in a fixed order (sum, difference, product, sum == difference) and
/// a range guard on the second input.
fn arithmetic() -> Graph {
    let scheme = super::common::IDS;
    let mut builder = GraphBuilder::new(SubProfile::I64, Subject::authored(&scheme, "batch-graph"));
    let a = builder.input(ScalarType::I64);
    let b = builder.input(ScalarType::I64);
    let sum = builder.op(Op::I64Add(a, b));
    let difference = builder.op(Op::I64Sub(a, b));
    let product = builder.op(Op::I64Mul(a, b));
    let is_equal = builder.op(Op::I64Eq(sum, difference));
    builder.guard(
        Fact::Range {
            lo: Literal::I64(-100),
            hi: Literal::I64(100),
        },
        b,
    );
    builder.output(sum);
    builder.output(difference);
    builder.output(product);
    builder.output(is_equal);
    builder.finish()
}

pub(super) fn compiled() -> (CheckedGraph, Compiled) {
    let checked = super::common::checked(arithmetic());
    let compiled = Compiled::new(&super::common::IDS, &checked);
    (checked, compiled)
}

pub(super) fn flatten(rows: &[[Value; 2]]) -> Vec<Value> {
    rows.iter().flatten().copied().collect()
}

/// A fresh compiled evaluation of one row: a new workspace, no earlier row.
pub(super) fn independent(compiled: &Compiled, row: &[Value], fuel: Option<u64>) -> Report {
    let mut control = Control {
        fuel,
        ..Control::default()
    };
    compiled.evaluate(row, None, &mut control)
}

/// The four outputs of a completed row, computed here with wrapping arithmetic.
const fn expected_values(a: i64, b: i64) -> [Value; 4] {
    let sum = a.wrapping_add(b);
    let difference = a.wrapping_sub(b);
    [
        Value::I64(sum),
        Value::I64(difference),
        Value::I64(a.wrapping_mul(b)),
        Value::Bool(sum == difference),
    ]
}

/// Ordinary rows, operand-order and wrapping rows, guard failures and refused rows, interleaved.
pub(super) fn mixed_rows() -> Vec<[Value; 2]> {
    let mut rows = vec![
        [Value::I64(3), Value::I64(5)],
        [Value::I64(5), Value::I64(3)],
        [Value::I64(i64::MAX), Value::I64(1)],
        [Value::I64(7), Value::I64(1000)],
        [Value::I64(i64::MIN), Value::I64(1)],
        [Value::Bool(true), Value::I64(4)],
        [Value::I64(i64::MAX), Value::I64(2)],
        [Value::I64(0), Value::I64(0)],
        [Value::I64(9), Value::I64(101)],
        [Value::I64(-9), Value::I64(-100)],
    ];
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    for _ in 0..32 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let a = state.cast_signed();
        let b = (state >> 33).cast_signed() % 160 - 80;
        rows.push([Value::I64(a), Value::I64(b)]);
    }
    rows
}

#[test]
fn every_row_of_a_run_is_its_independent_single_row_run() {
    let (checked, compiled) = compiled();
    let rows = mixed_rows();
    let mut prepared = Prepared::new(&compiled).unwrap();
    let outcome = prepared.run(rows.len(), &flatten(&rows), OPEN, None);
    assert_eq!(outcome.stop, None, "an unbounded run completes");
    assert_eq!(outcome.reports.len(), rows.len(), "one report per row");
    let mut kinds = [0usize; 3];
    for (index, (row, report)) in rows.iter().zip(&outcome.reports).enumerate() {
        assert_eq!(
            report,
            &independent(&compiled, row, None),
            "row {index} against a fresh compiled run"
        );
        let mut control = Control::default();
        let reference = eval::reference(&checked, row, None, &mut control);
        assert_eq!(
            report.obs, reference.obs,
            "row {index} against the reference"
        );
        match (&report.obs, row) {
            (Obs::Normal(_), [Value::I64(a), Value::I64(b)]) => {
                assert_eq!(
                    report.obs,
                    Obs::Normal(expected_values(*a, *b).to_vec()),
                    "row {index} values"
                );
                kinds[0] += 1;
            }
            (Obs::GuardFailed { .. }, _) => kinds[1] += 1,
            (Obs::Refused(_), _) => kinds[2] += 1,
            (other, _) => panic!("row {index}: unexpected observation {other:?}"),
        }
    }
    assert!(
        kinds.iter().all(|&count| count > 0),
        "the rows interleave completed, guard-failed and refused rows: {kinds:?}"
    );
}

#[test]
fn empty_and_single_row_runs_are_exact() {
    let (_, compiled) = compiled();
    let mut prepared = Prepared::new(&compiled).unwrap();
    assert_eq!(
        prepared.run(0, &[], OPEN, None),
        Outcome {
            reports: Vec::new(),
            stop: None,
        }
    );
    let row = [Value::I64(-4), Value::I64(6)];
    let outcome = prepared.run(1, &row, OPEN, None);
    assert_eq!(outcome.stop, None);
    assert_eq!(outcome.reports, vec![independent(&compiled, &row, None)]);
    assert_eq!(
        outcome.reports[0].obs,
        Obs::Normal(expected_values(-4, 6).to_vec())
    );
}

#[test]
fn shape_and_limits_refuse_the_run_before_any_row() {
    let (_, compiled) = compiled();
    let mut prepared = Prepared::new(&compiled).unwrap();
    let rows = flatten(&[
        [Value::I64(1), Value::I64(2)],
        [Value::I64(3), Value::I64(4)],
        [Value::I64(5), Value::I64(6)],
    ]);
    let asked = RefCell::new(Vec::new());
    let checkpoint = |row: usize| {
        asked.borrow_mut().push(row);
        false
    };
    let refused = |outcome: Outcome| {
        assert!(outcome.reports.is_empty(), "no row ran: {outcome:?}");
        outcome.stop
    };
    assert_eq!(
        refused(prepared.run(2, &rows, OPEN, Some(&checkpoint))),
        Some(Stop::Refused(Refusal::Shape {
            rows: 2,
            width: 2,
            values: 6,
        }))
    );
    let few_rows = Limits {
        max_rows: 2,
        ..OPEN
    };
    assert_eq!(
        refused(prepared.run(3, &rows, few_rows, Some(&checkpoint))),
        Some(Stop::Refused(Refusal::Rows {
            rows: 3,
            max_rows: 2,
        }))
    );
    let few_outputs = Limits {
        max_output_values: 11,
        ..OPEN
    };
    assert_eq!(
        refused(prepared.run(3, &rows, few_outputs, Some(&checkpoint))),
        Some(Stop::Refused(Refusal::Outputs {
            rows: 3,
            per_row: 4,
            max_output_values: 11,
        }))
    );
    assert!(
        asked.borrow().is_empty(),
        "a refused run never reaches a row checkpoint"
    );
    let exact_outputs = Limits {
        max_output_values: 12,
        max_rows: 3,
        ..OPEN
    };
    let outcome = prepared.run(3, &rows, exact_outputs, Some(&checkpoint));
    assert_eq!(outcome.stop, None, "limits equal to the run admit it");
    assert_eq!(outcome.reports.len(), 3);
    assert_eq!(*asked.borrow(), vec![0, 1, 2]);

    let float = super::common::checked(super::common::linear(false, false));
    let float_compiled = Compiled::new(&super::common::IDS, &float);
    assert!(
        matches!(Prepared::new(&float_compiled), Err(Refusal::Profile)),
        "an F32 graph is not admitted for batching"
    );
}
