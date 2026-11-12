//! Loss-graph admission (NGRAPH-DERIV-01), the `clamped-linear` width domain
//! and replay's recorded-identity checks.

use noble_ngraph::grad::{BooleanPolicy, DerivOptions};
use noble_ngraph::ir::{
    Fact, Graph, GraphBuilder, Literal, NodeId, Op, OutOfRange, Provenance, ScalarType, SubProfile,
    Subject,
};
use noble_ngraph::train::{self, TrainControl};

use super::common::{checked, contract, linear, mse, points, IDS};
use super::{code, run};

/// `s ∈ [0, 1]`, the fact `step` requires of its operand.
fn unit_range() -> Fact {
    Fact::Range {
        lo: Literal::f32(0.0),
        hi: Literal::f32(1.0),
    }
}

/// `y = rn(rn(p·x) + 0.5)`: input `x`, param `p`.
fn biased_linear() -> Graph {
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&IDS, "tests/biased-linear"),
    );
    let x = b.input(ScalarType::F32);
    let p = b.param();
    let px = b.op(Op::F32Mul(p, x));
    let half = b.f32_const(0.5);
    let y = b.op(Op::F32Add(px, half));
    b.output(y);
    b.finish()
}

/// `y = step(p)·x`: the same boolean path in the model graph. The step's
/// operand is the param itself, which is an interface cell and may carry the
/// input-validated fact `step` requires.
fn step_model() -> Graph {
    let mut b = GraphBuilder::new(SubProfile::F32, Subject::authored(&IDS, "tests/step-model"));
    let x = b.input(ScalarType::F32);
    let p = b.param();
    b.fact(p, unit_range(), Provenance::InputValidated);
    let s = b.op(Op::F32Step(p));
    let y = b.op(Op::F32Mul(s, x));
    b.output(y);
    b.finish()
}

/// `(y, t) ↦ (step(y) − t)²`.
fn step_loss() -> Graph {
    let mut b = GraphBuilder::new(SubProfile::F32, Subject::authored(&IDS, "tests/step-loss"));
    let y = b.input(ScalarType::F32);
    let t = b.input(ScalarType::F32);
    b.fact(y, unit_range(), Provenance::InputValidated);
    let s = b.op(Op::F32Step(y));
    let d = b.op(Op::F32Sub(s, t));
    let l = b.op(Op::F32Mul(d, d));
    b.output(l);
    b.finish()
}

/// `(y, t) ↦ (step(t) − y)²`: the boolean node holds a target, not the model
/// output, so no parameter reaches the loss through it.
fn step_target_loss() -> Graph {
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&IDS, "tests/step-target-loss"),
    );
    let y = b.input(ScalarType::F32);
    let t = b.input(ScalarType::F32);
    b.fact(t, unit_range(), Provenance::InputValidated);
    let s = b.op(Op::F32Step(t));
    let d = b.op(Op::F32Sub(s, y));
    let l = b.op(Op::F32Mul(d, d));
    b.output(l);
    b.finish()
}

/// `(y, t) ↦ index(y, [t, 0.5])`: the index operand is the model output.
fn index_loss() -> Graph {
    let mut b = GraphBuilder::new(SubProfile::F32, Subject::authored(&IDS, "tests/index-loss"));
    let y = b.input(ScalarType::F32);
    let t = b.input(ScalarType::F32);
    b.fact(y, Fact::Integral, Provenance::InputValidated);
    b.fact(y, unit_range(), Provenance::InputValidated);
    let half = b.f32_const(0.5);
    let r = b.op(Op::F32Index {
        index: y,
        table: vec![t, half],
        below: OutOfRange::Refuse,
        above: OutOfRange::Refuse,
    });
    b.output(r);
    b.finish()
}

#[test]
fn parameter_paths_through_loss_graph_boolean_and_index_nodes_are_refused() {
    let default = DerivOptions::default();
    // The model-graph class the loss graph is compared with.
    let model = checked(step_model());
    let loss = checked(mse());
    let e = train::admit(&model, &loss, model.param_slots(), &default)
        .expect_err("a model step carries the parameter");
    assert_eq!(e.code_string(), "E-DERIV-BOOLEAN");

    // `y = p·x + 0.5`, loss `(step(y) − t)²`: the parameter reaches the loss
    // through the loss graph's step, under the same refusal class.
    let model = checked(biased_linear());
    let loss = checked(step_loss());
    let e = train::admit(&model, &loss, model.param_slots(), &default)
        .expect_err("a loss step carries the model output");
    assert_eq!(e.code_string(), "E-DERIV-BOOLEAN");
    let base = contract(model.clone(), points());
    let stepped = train::Contract {
        loss: loss.clone(),
        ..base.clone()
    };
    let stopped = run(&stepped, 1).expect_err("training is refused before the first step");
    assert_eq!(stopped.diagnostic.code_string(), "E-DERIV-BOOLEAN");
    assert_eq!(stopped.published_steps, 0);
    assert!(stopped.last.is_none(), "refused before S0");
    run(&base, 1).expect("the same model trains against mse");

    // A loss index read whose index operand is the model output.
    let loss = checked(index_loss());
    let e = train::admit(&model, &loss, model.param_slots(), &default)
        .expect_err("a loss index operand carries the model output");
    assert_eq!(e.code_string(), "E-DERIV-INDEX");

    // A boolean node over a target alone is not a parameter path.
    let loss = checked(step_target_loss());
    train::admit(&model, &loss, model.param_slots(), &default)
        .expect("a target-only step is not refused");
}

#[test]
fn clamped_linear_widths_must_be_finite_positive_and_normal() {
    let model = checked(step_model());
    let loss = checked(mse());
    let params = model.param_slots().to_vec();
    let step = model
        .graph()
        .nodes
        .iter()
        .position(|n| matches!(n.op, Op::F32Step(_)))
        .expect("the model has a step node");
    let node = NodeId(u32::try_from(step).expect("a small node index"));
    let with = |width: f32| {
        let mut d = DerivOptions::default();
        d.boolean.insert(node, BooleanPolicy::ClampedLinear(width));
        d
    };
    train::admit(&model, &loss, &params, &with(0.5)).expect("a normal width is admitted");
    for width in [
        f32::NAN,
        0.0,
        -0.5,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::from_bits(1),
    ] {
        let e = train::admit(&model, &loss, &params, &with(width))
            .expect_err("not a width the policy defines");
        assert_eq!(
            e.code_string(),
            "E-DERIV-BOOLEAN",
            "width 0x{:08x}",
            width.to_bits()
        );
    }
}

#[test]
fn derivative_option_widths_compare_by_bits() {
    let width = |bits: u32| {
        let mut d = DerivOptions::default();
        d.boolean.insert(
            NodeId(1),
            BooleanPolicy::ClampedLinear(f32::from_bits(bits)),
        );
        d
    };
    let nan = width(0x7fc0_0000);
    let same_nan = width(0x7fc0_0000);
    assert!(nan.same_bits(&same_nan), "an unmodified NaN width replays");
    assert_ne!(
        nan, same_nan,
        "`==` cannot compare NaN widths, which is why replay compares bits"
    );
    assert!(!nan.same_bits(&width(0x7fc0_0001)), "another payload");
    assert!(width(0.5f32.to_bits()).same_bits(&width(0x3f00_0000)));
    assert!(!width(0.5f32.to_bits()).same_bits(&width(0x3f00_0001)));
    assert!(!width(0.5f32.to_bits()).same_bits(&DerivOptions::default()));
}

#[test]
fn replay_checks_the_recorded_subject_and_tool_revisions() {
    let c = contract(checked(linear(false, false)), points());
    let t = run(&c, 2).expect("trains");
    let mut control = TrainControl::default();
    train::replay(&IDS, &c, &t.provenance, None, &mut control)
        .expect("an unmodified record replays");
    let mut subject = t.provenance.clone();
    subject.subject.identity = [0x5a; 32];
    assert_eq!(
        code(train::replay(&IDS, &c, &subject, None, &mut control)),
        "E-SNAPSHOT-STALE"
    );
    let mut tools = t.provenance.clone();
    tools.tools.push_str("; forged");
    assert_eq!(
        code(train::replay(&IDS, &c, &tools, None, &mut control)),
        "E-SNAPSHOT-STALE"
    );
}
