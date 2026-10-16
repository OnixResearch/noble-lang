//! Checker and reference-interpreter behaviour on directly built IR.

use noble_ngraph_experiment::eval::{i64_inputs, Control};
use noble_ngraph_experiment::ir::{Fact, Literal, Op, Provenance, Subject};
use noble_ngraph_experiment::{
    check, reference, Code, GraphBuilder, Malformed, Obs, ScalarType, SubProfile, Value,
};

fn subject() -> Subject {
    Subject::authored("ir_check")
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
    let c = check(b.finish()).expect("checks");
    let rep = reference(&c, &i64_inputs(&[5, 4]), None, &Control::default());
    assert_eq!(rep.obs, Obs::Normal(vec![Value::I64(24)]));
    // `5 3 -` = 2 (a is the deeper operand).
    let mut b = GraphBuilder::new(SubProfile::I64, subject());
    let a = b.i64_const(5);
    let bb = b.i64_const(3);
    let d = b.op(Op::I64Sub(a, bb));
    b.output(d);
    let c = check(b.finish()).expect("checks");
    let rep = reference(&c, &[], None, &Control::default());
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
    let c = check(b.finish()).expect("checks");
    let rep = reference(&c, &i64_inputs(&[i64::MAX]), None, &Control::default());
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
    assert_eq!(check(b.finish()).unwrap_err().code, Code::TypeMix);

    let mut b = GraphBuilder::new(SubProfile::F32, subject());
    let x = b.input(ScalarType::F32);
    let s = b.op(Op::I64Add(x, x));
    b.output(s);
    assert_eq!(check(b.finish()).unwrap_err().code, Code::TypeMix);
}

#[test]
fn malformed_topology_is_refused() {
    use noble_ngraph_experiment::ir::{Node, NodeId};
    let mk = |ops: Vec<Op>, outs: Vec<u32>| {
        let mut b = GraphBuilder::new(SubProfile::I64, subject());
        b.declare_slot(
            noble_ngraph_experiment::ir::SlotKind::Input,
            ScalarType::I64,
        );
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
        let d = check(g).unwrap_err();
        assert_eq!(d.code, Code::GraphMalformed(want), "{d}");
    }
}

#[test]
fn nonfinite_constant_and_caller_assertion_are_refused() {
    let mut b = GraphBuilder::new(SubProfile::F32, subject());
    let x = b.constant(Literal::F32(0x7fc0_0000));
    b.output(x);
    assert_eq!(check(b.finish()).unwrap_err().code, Code::LiteralNonfinite);

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
    assert_eq!(check(b.finish()).unwrap_err().code, Code::FactUnchecked);
}
