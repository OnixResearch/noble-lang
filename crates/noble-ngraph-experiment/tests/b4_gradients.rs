#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::compound_assertion,
    tigerstyle::explicit_defaults,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! B4: smooth-region gradients (G1 vs central differences, G2 vs G1) and the
//! corner, boolean and index policies tested separately.

use noble_ngraph::grad::{self, BooleanPolicy, DerivOptions, IndexPolicy};
use noble_ngraph::ir::{
    Fact, GraphBuilder, NodeId, Op, OutOfRange, Provenance, ScalarType, SubProfile, Subject,
};
use noble_ngraph::{check, CheckedGraph, Code};
use noble_ngraph_experiment::fixtures::{identity_loss, range};
use noble_ngraph_experiment::scheme::BLAKE3;
use noble_ngraph_experiment::suites::gradient_campaign;

fn id_loss() -> CheckedGraph {
    check(&BLAKE3, identity_loss()).expect("identity loss")
}

/// d/dslot of the identity-loss objective, in binary64 and binary32.
fn grads(m: &CheckedGraph, slots: &[f32], opts: &DerivOptions) -> (Vec<f64>, Vec<f32>) {
    let l = id_loss();
    let s64: Vec<f64> = slots.iter().map(|v| f64::from(*v)).collect();
    let (_, g64) = grad::objective::<f64>(m, &l, &s64, &[], opts);
    let (_, g32) = grad::objective::<f32>(m, &l, slots, &[], opts);
    (g64, g32)
}

#[test]
fn smooth_region_g1_and_g2_hold_on_f1_mse() {
    let st = gradient_campaign(0x00b4, 60);
    assert!(st.used >= 40, "{st:?}");
    assert_eq!(st.g1_failures, 0, "{st:?}");
    assert_eq!(st.g2_failures, 0, "{st:?}");
}

/// `y = reglu(p0·x, p1·x)`: analytic `∂y/∂p0 = x·relu(p1·x)`, `∂y/∂p1 = p0·x·x` (gate > 0).
fn reglu_model() -> CheckedGraph {
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "b4 reglu analytic"),
    );
    let x = b.input(ScalarType::F32);
    let p0 = b.param();
    let p1 = b.param();
    let l = b.op(Op::F32Mul(p0, x));
    let g = b.op(Op::F32Mul(p1, x));
    let y = b.op(Op::F32Reglu { left: l, gate: g });
    b.output(y);
    check(&BLAKE3, b.finish()).expect("checks")
}

#[test]
fn analytic_examples() {
    let m = reglu_model();
    let (g64, g32) = grads(&m, &[2.0, 3.0, 0.5], &DerivOptions::default());
    // ∂y/∂x = p0·relu(p1·x) + p0·x·p1 = 3 + 3 = 6.
    assert_eq!(g64, vec![6.0, 2.0, 12.0]);
    assert_eq!(g32, vec![6.0f32, 2.0, 12.0]);
}

#[test]
fn relu_corner_at_plus_and_minus_zero_gives_zero_gate_gradient() {
    let m = reglu_model();
    // p1 = 0, x = 2 → gate = +0; x = -2 → gate = -0.
    for x in [2.0f32, -2.0] {
        let (g64, g32) = grads(&m, &[x, 3.0, 0.0], &DerivOptions::default());
        assert_eq!(g64[2], 0.0, "x={x}");
        assert_eq!(g32[2], 0.0, "x={x}");
    }
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "b4 relu corner"),
    );
    let p = b.param();
    let y = b.op(Op::F32Relu(p));
    b.output(y);
    let m = check(&BLAKE3, b.finish()).expect("checks");
    for p in [0.0f32, -0.0] {
        let (g64, g32) = grads(&m, &[p], &DerivOptions::default());
        assert_eq!((g64[0], g32[0]), (0.0, 0.0));
    }
}

/// `y = step(p·x)` with a guard `p·x ∈ [0,1]`.
fn step_model() -> (CheckedGraph, NodeId) {
    let mut b = GraphBuilder::new(SubProfile::F32, Subject::authored(&BLAKE3, "b4 step"));
    let x = b.input(ScalarType::F32);
    let p = b.param();
    let s = b.op(Op::F32Mul(p, x));
    b.guard(range(0.0, 1.0), s);
    let y = b.op(Op::F32Step(s));
    b.output(y);
    (check(&BLAKE3, b.finish()).expect("checks"), y)
}

#[test]
fn boolean_path_is_refused_statically_and_surrogates_are_recorded() {
    let (m, step) = step_model();
    let d = grad::admit(&m, &[1], &DerivOptions::default()).unwrap_err();
    assert_eq!(d.code, Code::DerivBoolean);

    let mut st = DerivOptions::default();
    st.boolean.insert(step, BooleanPolicy::StraightThrough);
    grad::admit(&m, &[1], &st).expect("straight-through admitted");
    assert!(st.uses_surrogate());
    assert!(st.describe().contains("straight-through(surrogate)"));
    let (g64, g32) = grads(&m, &[0.5, 0.75], &st);
    assert_eq!((g64[1], g32[1]), (0.5, 0.5), "identity surrogate: ∂/∂p = x");

    let mut cl = DerivOptions::default();
    cl.boolean.insert(step, BooleanPolicy::ClampedLinear(0.5));
    assert!(cl.describe().contains("clamped-linear"));
    // s = 0.375: |s-0.5| < 0.25 → slope 2; ∂/∂p = 2·x.
    let (g64, _) = grads(&m, &[0.5, 0.75], &cl);
    assert_eq!(g64[1], 1.0);
    // s = 0.9: outside → 0.
    let (g64, _) = grads(&m, &[1.0, 0.9], &cl);
    assert_eq!(g64[1], 0.0);
}

#[test]
fn select_gate_path_is_refused_by_default() {
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "b4 select gate"),
    );
    let off = b.input(ScalarType::F32);
    let on = b.input(ScalarType::F32);
    let p = b.param();
    b.guard(range(0.0, 1.0), p);
    let bit = b.op(Op::F32Step(p));
    let y = b.op(Op::F32Select { bit, off, on });
    b.output(y);
    let m = check(&BLAKE3, b.finish()).expect("checks");
    let mut o = DerivOptions::default();
    o.boolean.insert(bit, BooleanPolicy::StraightThrough);
    let d = grad::admit(&m, &[2], &o).unwrap_err();
    assert_eq!(d.code, Code::DerivBoolean, "select gate still refused");
    // Candidate gradients through select need no opt-in.
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "b4 select candidates"),
    );
    let bitin = b.input(ScalarType::F32);
    b.fact(bitin, Fact::Bit, Provenance::InputValidated);
    let p0 = b.param();
    let p1 = b.param();
    let y = b.op(Op::F32Select {
        bit: bitin,
        off: p0,
        on: p1,
    });
    b.output(y);
    let m = check(&BLAKE3, b.finish()).expect("checks");
    grad::admit(&m, &[1, 2], &DerivOptions::default()).expect("candidates admitted");
    let (g64, g32) = grads(&m, &[1.0, 5.0, 7.0], &DerivOptions::default());
    assert_eq!(
        (&g64[1..], &g32[1..]),
        (&[0.0, 1.0][..], &[0.0f32, 1.0][..])
    );
}

#[test]
fn index_candidate_gradients_accumulate() {
    // y = index(0,[a,a,b]) + index(1,[a,a,b]) → ∂y/∂a = 2, ∂y/∂b = 0.
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "b4 index candidates"),
    );
    let i0 = b.input(ScalarType::F32);
    let i1 = b.input(ScalarType::F32);
    for i in [i0, i1] {
        b.fact(i, Fact::Integral, Provenance::InputValidated);
        b.fact(i, range(0.0, 2.0), Provenance::InputValidated);
    }
    let a = b.param();
    let bb = b.param();
    let r = |b: &mut GraphBuilder, i| {
        b.op(Op::F32Index {
            index: i,
            table: vec![a, a, bb],
            below: OutOfRange::Refuse,
            above: OutOfRange::Refuse,
        })
    };
    let r0 = r(&mut b, i0);
    let r1 = r(&mut b, i1);
    let y = b.op(Op::F32Add(r0, r1));
    b.output(y);
    let m = check(&BLAKE3, b.finish()).expect("checks");
    grad::admit(&m, &[2, 3], &DerivOptions::default())
        .expect("candidate gradients allowed by default");
    let (g64, g32) = grads(&m, &[0.0, 1.0, 4.0, 9.0], &DerivOptions::default());
    assert_eq!(
        (&g64[2..], &g32[2..]),
        (&[2.0, 0.0][..], &[2.0f32, 0.0][..])
    );
}

#[test]
fn index_operand_policies_reject_zero_backward_difference() {
    // y = index(q, [c0, c1, c2]) with a param index q (guarded integral, range).
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "b4 index operand"),
    );
    let c: Vec<NodeId> = (0..3).map(|_| b.input(ScalarType::F32)).collect();
    let q = b.param();
    b.guard(Fact::Integral, q);
    b.guard(range(0.0, 2.0), q);
    let y = b.op(Op::F32Index {
        index: q,
        table: c.clone(),
        below: OutOfRange::Clamp,
        above: OutOfRange::Clamp,
    });
    b.output(y);
    let m = check(&BLAKE3, b.finish()).expect("checks");
    // reject (default)
    assert_eq!(
        grad::admit(&m, &[3], &DerivOptions::default())
            .unwrap_err()
            .code,
        Code::DerivIndex
    );
    // zero
    let mut z = DerivOptions::default();
    z.index.insert(y, IndexPolicy::Zero);
    grad::admit(&m, &[3], &z).expect("zero admitted");
    assert!(!z.uses_surrogate());
    let (g64, g32) = grads(&m, &[10.0, 20.0, 50.0, 2.0], &z);
    assert_eq!((g64[3], g32[3]), (0.0, 0.0));
    // backward-difference (surrogate): read(2) − read(1) = 30.
    let mut bd = DerivOptions::default();
    bd.index.insert(y, IndexPolicy::BackwardDifference);
    grad::admit(&m, &[3], &bd).expect("backward-difference admitted");
    assert!(bd.uses_surrogate() && bd.describe().contains("backward-difference(surrogate)"));
    let (g64, g32) = grads(&m, &[10.0, 20.0, 50.0, 2.0], &bd);
    assert_eq!((g64[3], g32[3]), (30.0, 30.0));
}

#[test]
fn i64_and_undeclared_params_are_refused() {
    let mut b = GraphBuilder::new(SubProfile::I64, Subject::authored(&BLAKE3, "b4 i64"));
    let x = b.input(ScalarType::I64);
    let y = b.op(Op::I64Mul(x, x));
    b.output(y);
    let m = check(&BLAKE3, b.finish()).expect("checks");
    assert_eq!(
        grad::admit(&m, &[], &DerivOptions::default())
            .unwrap_err()
            .code,
        Code::DerivI64
    );
    let m = reglu_model();
    assert_eq!(
        grad::admit(&m, &[0], &DerivOptions::default())
            .unwrap_err()
            .code,
        Code::ParamUndeclared
    );
}
