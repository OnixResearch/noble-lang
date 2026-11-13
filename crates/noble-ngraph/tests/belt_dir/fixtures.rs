#![allow(
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    reason = "Owner: noble-maintainers; these fixed test fixtures intentionally fail loudly if graph admission or baseline evaluation stops working."
)]

use noble_ngraph::check::{self, CheckedGraph};
use noble_ngraph::ir::{Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::sem::Value;

/// Interleaved use lifetimes, fanout, asymmetric subtraction, a boundary constant, equality,
/// duplicate outputs, and a value used after a belt-capacity-2 eviction.
pub(super) fn graph() -> Graph {
    let ids = super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&ids, "belt-113-correspondence"),
    );
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let minimum = builder.i64_const(i64::MIN);
    let sum = builder.op(Op::I64Add(x, y));
    let difference = builder.op(Op::I64Sub(y, x));
    let product = builder.op(Op::I64Mul(difference, minimum));
    let equal = builder.op(Op::I64Eq(sum, product));
    let fanout = builder.op(Op::I64Add(x, x));
    for output in [sum, difference, equal, sum, product, fanout, difference] {
        builder.output(output);
    }
    builder.finish()
}

pub(super) fn checked(graph: Graph) -> CheckedGraph {
    check::check(&super::common::IDS, graph).expect("the fixed I64 graph checks")
}

pub(super) fn vectors() -> [([Value; 2], [Value; 7]); 3] {
    [
        (
            [Value::I64(i64::MAX), Value::I64(1)],
            [
                Value::I64(i64::MIN),
                Value::I64(-9_223_372_036_854_775_806),
                Value::Bool(false),
                Value::I64(i64::MIN),
                Value::I64(0),
                Value::I64(-2),
                Value::I64(-9_223_372_036_854_775_806),
            ],
        ),
        (
            [Value::I64(i64::MIN), Value::I64(-1)],
            [
                Value::I64(i64::MAX),
                Value::I64(i64::MAX),
                Value::Bool(false),
                Value::I64(i64::MAX),
                Value::I64(i64::MIN),
                Value::I64(0),
                Value::I64(i64::MAX),
            ],
        ),
        (
            [Value::I64(5), Value::I64(3)],
            [
                Value::I64(8),
                Value::I64(-2),
                Value::Bool(false),
                Value::I64(8),
                Value::I64(0),
                Value::I64(10),
                Value::I64(-2),
            ],
        ),
    ]
}
