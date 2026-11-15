//! Fixed #32 partial-static offset comparison: D3's baseline is always the control.
//! Timing is descriptive; semantic equality and growth refusals are asserted.

use noble_ngraph::check::{self, Budgets};
use noble_ngraph::ir::{GraphBuilder, Op, ScalarType, ShapeTree, SubProfile, Subject};
use noble_ngraph::sem::Value;
use noble_ngraph::specialize::{self, offset, Request};

use super::common::IDS;

#[path = "offset/fixtures.rs"]
mod fixtures;
#[path = "offset/measurement.rs"]
mod measurement;
#[path = "offset/scratch.rs"]
mod scratch;

use self::fixtures::{compare, graph, observed, Fixture};

#[test]
fn normalization_preserves_wrapping_order_and_shared_bindings() {
    for kind in [
        Fixture::Add,
        Fixture::Sub,
        Fixture::Wrap,
        Fixture::Shared,
        Fixture::Nonprefix,
        Fixture::Dead,
        Fixture::Growth,
    ] {
        let _ = compare(kind);
    }
    let (base_work, offset_work, base_nodes, offset_nodes) = compare(Fixture::Add);
    assert!(offset_work < base_work, "(x+2)+3 reduces evaluator work");
    assert!(offset_nodes < base_nodes, "(x+2)+3 becomes x+5");
    let (shared, map) = graph(Fixture::Shared);
    let result = offset::compare(
        &IDS,
        &shared,
        &Request {
            static_inputs: &map,
            budgets: &Budgets::default(),
        },
    )
    .expect("shared output");
    let emitted = specialize::emit_source(&result.normalized).expect("lexical sharing");
    assert!(
        emitted.contains("let v"),
        "the reused subexpression remains a lexical binding"
    );
    let inputs = [Value::I64(3)];
    let actual = observed(&result.normalized, &inputs).0;
    assert_eq!(
        actual,
        vec![Value::I64(5), Value::I64(10)],
        "the same shared node occurs twice"
    );
}

#[test]
fn growth_cap_refuses_without_publishing_and_baseline_is_unchanged() {
    let (original, map) = graph(Fixture::Growth);
    let base = specialize::specialize(
        &IDS,
        &original,
        &Request {
            static_inputs: &map,
            budgets: &Budgets::default(),
        },
    )
    .expect("D3 baseline");
    let baseline_nodes = base.checked.node_count() as u64;
    let mut budgets = Budgets {
        residual_nodes: baseline_nodes + 2,
        ..Budgets::default()
    };
    let accepted = offset::compare(
        &IDS,
        &original,
        &Request {
            static_inputs: &map,
            budgets: &budgets,
        },
    )
    .expect("exact generated-node cap");
    assert_eq!(accepted.generated_nodes, baseline_nodes + 2);
    assert_eq!(
        accepted.baseline.checked.id(),
        base.checked.id(),
        "D3 baseline is unchanged"
    );
    for allowed_nodes in [baseline_nodes + 1, baseline_nodes, baseline_nodes - 1] {
        budgets.residual_nodes = allowed_nodes;
        assert_eq!(
            offset::compare(
                &IDS,
                &original,
                &Request {
                    static_inputs: &map,
                    budgets: &budgets
                }
            )
            .expect_err("insufficient baseline or generated-node cap")
            .code_string(),
            "E-BUDGET-RESIDUAL-NODES"
        );
    }
}

#[test]
fn refuses_nested_partially_known_pairs_and_preserves_dynamic_bool_slots() {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&IDS, "offset-nested-pair"),
    );
    let x = builder.input(ScalarType::I64);
    let _y = builder.input(ScalarType::I64);
    builder.output(x);
    builder.shapes(
        Some(vec![ShapeTree::pair(
            ShapeTree::pair(ShapeTree::SCALAR, ShapeTree::UNIT).expect("bounded nested pair"),
            ShapeTree::SCALAR,
        )
        .expect("bounded nested pair")]),
        None,
    );
    let pair = check::check(&IDS, builder.finish()).expect("shape-bearing checked graph");
    let map = [Some(2), None];
    assert_eq!(
        offset::compare(
            &IDS,
            &pair,
            &Request {
                static_inputs: &map,
                budgets: &Budgets::default()
            }
        )
        .expect_err("authored shape excluded")
        .code_string(),
        "E-AGGREGATE"
    );
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&IDS, "offset-dynamic-bool"),
    );
    let flag = builder.input(ScalarType::Bool);
    let number = builder.input(ScalarType::I64);
    let two = builder.i64_const(2);
    let three = builder.i64_const(3);
    let sum = builder.op(Op::I64Add(number, two));
    let sum = builder.op(Op::I64Add(sum, three));
    builder.output(flag);
    builder.output(sum);
    let graph = check::check(&IDS, builder.finish()).expect("mixed scalar slots");
    let result = offset::compare(
        &IDS,
        &graph,
        &Request {
            static_inputs: &[None, None],
            budgets: &Budgets::default(),
        },
    )
    .expect("dynamic Bool remains ordered");
    assert_eq!(
        observed(&result.normalized, &[Value::Bool(true), Value::I64(4)]).0,
        vec![Value::Bool(true), Value::I64(9)]
    );
}
