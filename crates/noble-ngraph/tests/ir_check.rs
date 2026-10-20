#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    tigerstyle::assertion_density,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Checker and reference-interpreter behaviour on directly built IR.

mod common;

use common::IDS;
use noble_ngraph::eval::{i64_inputs, Control};
use noble_ngraph::ir::{Fact, Literal, Op, Provenance, Subject};
use noble_ngraph::{
    check, reference, Code, GraphBuilder, Malformed, Obs, ScalarType, SubProfile, Value,
};

fn subject() -> Subject {
    Subject::authored(&IDS, "ir_check")
}

/// I1 by hand: `(x y) dup * swap 3 * + 7 -` = `y*y + 3x - 7`; `(5,4) → 24`.
#[test]
fn i64_graph_operand_order_and_value() {
    let mut b = GraphBuilder::new(SubProfile::I64, subject());
    let x = b.input(ScalarType::I64);
    let y = b.input(ScalarType::I64);
    let yy = b.op(Op::I64Mul(y, y));
    let three = b.i64_const(3);
    let x3 = b.op(Op::I64Mul(x, three));
    let s = b.op(Op::I64Add(yy, x3));
    let seven = b.i64_const(7);
    let r = b.op(Op::I64Sub(s, seven));
    b.output(r);
    let c = check(&IDS, b.finish()).expect("checks");
    let rep = reference(&c, &i64_inputs(&[5, 4]), None, &mut Control::default());
    assert_eq!(rep.obs, Obs::Normal(vec![Value::I64(24)]));
    // `5 3 -` = 2 (a is the deeper operand).
    let mut b = GraphBuilder::new(SubProfile::I64, subject());
    let a = b.i64_const(5);
    let bb = b.i64_const(3);
    let d = b.op(Op::I64Sub(a, bb));
    b.output(d);
    let c = check(&IDS, b.finish()).expect("checks");
    let rep = reference(&c, &[], None, &mut Control::default());
    assert_eq!(rep.obs, Obs::Normal(vec![Value::I64(2)]));
}

#[test]
fn i64_wraps_and_eq() {
    let mut b = GraphBuilder::new(SubProfile::I64, subject());
    let x = b.input(ScalarType::I64);
    let one = b.i64_const(1);
    let s = b.op(Op::I64Add(x, one));
    let m = b.i64_const(i64::MIN);
    let e = b.op(Op::I64Eq(s, m));
    b.output(s);
    b.output(e);
    let c = check(&IDS, b.finish()).expect("checks");
    let rep = reference(&c, &i64_inputs(&[i64::MAX]), None, &mut Control::default());
    assert_eq!(
        rep.obs,
        Obs::Normal(vec![Value::I64(i64::MIN), Value::Bool(true)])
    );
}

#[test]
fn type_mix_is_refused() {
    let mut b = GraphBuilder::new(SubProfile::I64, subject());
    let x = b.input(ScalarType::I64);
    let f = b.f32_const(1.0);
    let s = b.op(Op::I64Add(x, f));
    b.output(s);
    assert_eq!(check(&IDS, b.finish()).unwrap_err().code, Code::TypeMix);

    let mut b = GraphBuilder::new(SubProfile::F32, subject());
    let x = b.input(ScalarType::F32);
    let s = b.op(Op::I64Add(x, x));
    b.output(s);
    assert_eq!(check(&IDS, b.finish()).unwrap_err().code, Code::TypeMix);
}

#[test]
fn malformed_topology_is_refused() {
    use noble_ngraph::ir::{Node, NodeId};
    let mk = |ops: Vec<Op>, outs: Vec<u32>| {
        let mut b = GraphBuilder::new(SubProfile::I64, subject());
        b.declare_slot(noble_ngraph::ir::SlotKind::Input, ScalarType::I64);
        let mut g = b.finish();
        g.nodes = ops
            .into_iter()
            .map(|op| Node { op, origin: None })
            .collect();
        g.outputs = outs.into_iter().map(NodeId).collect();
        g
    };
    let n = NodeId;
    let cases = [
        (
            mk(
                vec![Op::Input(0), Op::I64Add(n(0), n(2)), Op::I64Add(n(0), n(1))],
                vec![2],
            ),
            Malformed::Cycle,
        ),
        (
            mk(
                vec![
                    Op::Input(0),
                    Op::I64Add(n(0), n(2)),
                    Op::Const(Literal::I64(1)),
                ],
                vec![1],
            ),
            Malformed::ForwardReference,
        ),
        (
            mk(vec![Op::Input(0), Op::I64Add(n(0), n(9))], vec![1]),
            Malformed::UnknownNode,
        ),
        (
            mk(vec![Op::Input(0), Op::Const(Literal::I64(1))], vec![0]),
            Malformed::DanglingNode,
        ),
        (
            mk(vec![Op::Input(0), Op::Input(0)], vec![0]),
            Malformed::DuplicateSlot,
        ),
        (
            mk(
                vec![
                    Op::Input(0),
                    Op::Const(Literal::Bool(true)),
                    Op::I64Add(n(0), n(1)),
                ],
                vec![2],
            ),
            Malformed::TypeError,
        ),
    ];
    for (g, want) in cases {
        let d = check(&IDS, g).unwrap_err();
        assert_eq!(d.code, Code::GraphMalformed(want), "{d}");
    }
}

#[test]
fn nonfinite_constant_and_caller_assertion_are_refused() {
    let mut b = GraphBuilder::new(SubProfile::F32, subject());
    let x = b.constant(Literal::F32(0x7fc0_0000));
    b.output(x);
    assert_eq!(
        check(&IDS, b.finish()).unwrap_err().code,
        Code::LiteralNonfinite
    );

    let mut b = GraphBuilder::new(SubProfile::F32, subject());
    let x = b.input(ScalarType::F32);
    let s = b.op(Op::F32Step(x));
    b.output(s);
    b.fact(
        x,
        Fact::Range {
            lo: Literal::f32(0.0),
            hi: Literal::f32(1.0),
        },
        Provenance::Asserted,
    );
    assert_eq!(
        check(&IDS, b.finish()).unwrap_err().code,
        Code::FactUnchecked
    );
}

/// NGRAPH-CASE-04/08/12 `evaluated_nodes: 0` (not a CLI report field):
/// every refusal before evaluation — an input fact, the interface, a missing
/// or stale snapshot — leaves both evaluators at zero evaluated nodes, and
/// an admitted vector evaluates every node.
#[test]
fn refusals_before_evaluation_evaluate_no_node() {
    use noble_ngraph::compiled::Compiled;
    use noble_ngraph::eval::f32_inputs;
    use noble_ngraph::snapshot::ParamSnapshot;
    let model = check(&IDS, common::linear(false, true)).expect("checks");
    let other = check(&IDS, common::linear(true, false)).expect("checks");
    let s = ParamSnapshot::new(&IDS, &model, &[2.0, 0.5]).expect("snapshot");
    let stale = ParamSnapshot::new(&IDS, &other, &[2.0, 0.5]).expect("snapshot");
    let refusals: [(&str, Vec<Value>, Option<&ParamSnapshot>, Code); 5] = [
        ("input fact", f32_inputs(&[5.0]), Some(&s), Code::FactInput),
        ("missing input", vec![], Some(&s), Code::Interface),
        ("input type", vec![Value::I64(1)], Some(&s), Code::Interface),
        (
            "no snapshot",
            f32_inputs(&[1.0]),
            None,
            Code::SnapshotLayout,
        ),
        (
            "stale snapshot",
            f32_inputs(&[1.0]),
            Some(&stale),
            Code::SnapshotStale,
        ),
    ];
    let compiled = Compiled::new(&IDS, &model);
    for (name, inputs, snapshot, want) in refusals {
        for report in [
            reference(&model, &inputs, snapshot, &mut Control::default()),
            compiled.evaluate(&inputs, snapshot, &mut Control::default()),
        ] {
            assert!(
                matches!(&report.obs, Obs::Refused(d) if d.code == want),
                "{name}: {report:?}"
            );
            assert_eq!(report.evaluated_nodes, 0, "{name}");
        }
    }
    let admitted = reference(
        &model,
        &f32_inputs(&[1.0]),
        Some(&s),
        &mut Control::default(),
    );
    assert_eq!(admitted.obs, Obs::Normal(vec![Value::f32(2.5)]));
    assert_eq!(admitted.evaluated_nodes, model.node_count() as u64);
}

/// Preflight refusal cannot consume a caller's accumulated counter, and a
/// successful reused evaluator must accumulate rather than reset that state.
#[test]
fn caller_owned_node_observer_tracks_execution_not_refusals() {
    use noble_ngraph::compiled::Compiled;
    let mut b = GraphBuilder::new(SubProfile::I64, subject());
    let x = b.input(ScalarType::I64);
    let twice = b.op(Op::I64Add(x, x));
    b.output(twice);
    let checked = check(&IDS, b.finish()).unwrap();
    let compiled = Compiled::new(&IDS, &checked);
    let mut workspace = compiled.workspace();
    for reference_mode in [true, false] {
        let mut nodes = 7;
        let mut run = |inputs: &[Value], nodes: &mut u64| {
            let mut control = Control {
                evaluated_nodes: Some(nodes),
                ..Control::default()
            };
            let report = if reference_mode {
                reference(&checked, inputs, None, &mut control)
            } else {
                compiled.evaluate_in(&mut workspace, inputs, None, &mut control)
            };
            report.obs
        };
        assert!(matches!(run(&[Value::F32(0)], &mut nodes), Obs::Refused(_)));
        assert_eq!(nodes, 7, "preflight refusal preserves caller state");
        assert_eq!(
            run(&i64_inputs(&[9]), &mut nodes),
            Obs::Normal(vec![Value::I64(18)])
        );
        let per_call = nodes - 7;
        assert!(per_call > 0, "successful execution contributes actual work");
        assert_eq!(
            run(&i64_inputs(&[9]), &mut nodes),
            Obs::Normal(vec![Value::I64(18)])
        );
        assert_eq!(
            nodes,
            7 + 2 * per_call,
            "reused execution accumulates without resetting"
        );
        let before_refusal = nodes;
        assert!(matches!(run(&[], &mut nodes), Obs::Refused(_)));
        assert_eq!(
            nodes, before_refusal,
            "later preflight refusal preserves accumulated work"
        );
    }
}
