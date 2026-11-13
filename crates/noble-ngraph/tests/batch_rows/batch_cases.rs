#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::explicit_defaults,
    tigerstyle::non_trait_imports,
    tigerstyle::assertion_density,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::ambiguous_params,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; these batch contract tests compare checked graph execution and model-derived rows against explicit expected results, so dense behavioral assertions and bounded test buffers are intentional."
)]
//! Batch contract cases for interface layouts and the kernel-checked operation corpus (issue #41).

use noble_ngraph::batch::{Limits, Prepared, Refusal, RowOp, Stop};
use noble_ngraph::check;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::Obs;
use noble_ngraph::ir::{Graph, GraphBuilder, Op, ScalarType, ShapeTree, SubProfile, Subject};
use noble_ngraph::sem::Value;

/// Three scalar slots whose interface item is nested as `((I64, I64), I64)`.
fn shape_graph() -> Graph {
    let scheme = super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&scheme, "batch-nested-shapes"),
    );
    let a = builder.input(ScalarType::I64);
    let b = builder.input(ScalarType::I64);
    let c = builder.input(ScalarType::I64);
    let sum = builder.op(Op::I64Add(a, b));
    let product = builder.op(Op::I64Mul(sum, c));
    let is_self_product = builder.op(Op::I64Eq(product, a));
    let nested = ShapeTree::pair(
        ShapeTree::pair(ShapeTree::SCALAR, ShapeTree::SCALAR).expect("one pair deep"),
        ShapeTree::SCALAR,
    )
    .expect("two pairs deep");
    builder.shapes(
        Some(vec![nested]),
        Some(vec![
            ShapeTree::pair(ShapeTree::SCALAR, ShapeTree::SCALAR).expect("one pair deep")
        ]),
    );
    builder.output(product);
    builder.output(is_self_product);
    builder.finish()
}

#[test]
fn interface_pair_layout_stays_flat() {
    let checked = super::common::checked(shape_graph());
    assert_eq!(checked.graph().slots.len(), 3);
    assert!(checked.graph().input_shape.is_some());
    assert!(checked.graph().output_shape.is_some());
    let compiled = Compiled::new(&super::common::IDS, &checked);
    let mut prepared = Prepared::new(&compiled).expect("an I64 graph");
    let rows = [
        [Value::I64(3), Value::I64(5), Value::I64(-2)],
        [Value::I64(i64::MAX), Value::I64(1), Value::I64(2)],
        [Value::I64(-4), Value::I64(2), Value::I64(0)],
        [Value::I64(9), Value::I64(-100), Value::I64(101)],
    ];
    let values: Vec<Value> = rows.iter().flatten().copied().collect();
    let outcome = prepared.run(rows.len(), &values, super::graph::OPEN, None);
    assert_eq!(outcome.stop, None, "the run completes");
    for (index, (row, report)) in rows.iter().zip(&outcome.reports).enumerate() {
        assert_eq!(
            report,
            &super::graph::independent(&compiled, row, None),
            "row {index} against a fresh compiled run"
        );
        let (Value::I64(a), Value::I64(b), Value::I64(c)) = (row[0], row[1], row[2]) else {
            panic!("row {index} is three I64 values");
        };
        let product = a.wrapping_add(b).wrapping_mul(c);
        assert_eq!(
            report.obs,
            Obs::Normal(vec![Value::I64(product), Value::Bool(product == a)]),
            "row {index} outputs in graph order"
        );
    }
    assert_eq!(
        prepared
            .run(2, &[Value::I64(1), Value::I64(2)], super::graph::OPEN, None,)
            .stop,
        Some(Stop::Refused(Refusal::Shape {
            rows: 2,
            width: 3,
            values: 2,
        }))
    );
}

#[test]
fn i64_graphs_refuse_parameter_slots() {
    let scheme = super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&scheme, "batch-i64-parameter"),
    );
    let input = builder.input(ScalarType::I64);
    let parameter = builder.param();
    let sum = builder.op(Op::I64Add(input, parameter));
    builder.output(sum);
    let refusal = check::check(&scheme, builder.finish())
        .expect_err("an I64 graph cannot declare a parameter slot");
    assert_eq!(refusal.code_string(), "E-TYPE-MIX");
}

const OPEN: Limits = Limits {
    max_rows: 1024,
    max_output_values: 4096,
    row_fuel: None,
    run_fuel: None,
};

/// Two `I64` inputs and one wrapping operation, returned as the graph's only output.
fn operation(op: RowOp) -> Graph {
    let scheme = super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&scheme, "batch-corpus-replay"),
    );
    let left = builder.input(ScalarType::I64);
    let right = builder.input(ScalarType::I64);
    let result = builder.op(match op {
        RowOp::Add => Op::I64Add(left, right),
        RowOp::Sub => Op::I64Sub(left, right),
        RowOp::Mul => Op::I64Mul(left, right),
    });
    builder.output(result);
    builder.finish()
}

#[test]
fn model_rows_run_via_prepared() {
    let cases = super::corpus();
    let mut replayed: usize = 0;
    let mut rows = Vec::with_capacity(cases.len());
    let mut expected = Vec::with_capacity(cases.len());
    let mut values = Vec::with_capacity(cases.len().saturating_mul(2));
    for op in [RowOp::Add, RowOp::Sub, RowOp::Mul] {
        let checked = super::common::checked(operation(op));
        let compiled = Compiled::new(&super::common::IDS, &checked);
        let mut prepared = Prepared::new(&compiled).expect("an I64 graph is admitted");
        rows.clear();
        expected.clear();
        values.clear();
        for case in cases.iter().filter(|case| case.row.op == op) {
            rows.push([case.row.deeper, case.row.top]);
            expected.push(
                case.expected
                    .parse::<i64>()
                    .expect("the model accepted every corpus row"),
            );
        }
        assert!(!rows.is_empty(), "the model has {op:?} cases");
        values.extend(
            rows.iter()
                .flat_map(|[left, right]| [Value::I64(*left), Value::I64(*right)]),
        );
        let outcome = prepared.run(rows.len(), &values, OPEN, None);
        assert_eq!(outcome.stop, None, "the {op:?} batch completes");
        assert_eq!(outcome.reports.len(), rows.len(), "one report per row");
        for (index, (report, expected)) in outcome.reports.iter().zip(&expected).enumerate() {
            assert_eq!(
                report.obs,
                Obs::Normal(vec![Value::I64(*expected)]),
                "{op:?} case {index} of {}",
                rows.len()
            );
        }
        replayed = replayed.saturating_add(rows.len());
    }
    assert_eq!(
        replayed,
        cases.len(),
        "every model row is run through one graph"
    );
}
