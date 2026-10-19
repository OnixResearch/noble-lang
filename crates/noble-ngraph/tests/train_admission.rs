#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::bool_naming,
    tigerstyle::excessive_file_length,
    tigerstyle::explicit_defaults,
    tigerstyle::function_length,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Training admission and replay (design D10, NGRAPH-DERIV-01,
//! NGRAPH-SNAP-01/02, NGRAPH-CASE-11/13 at the library level).

mod common;

use common::{checked, contract, linear, mse, points, IDS};
use noble_ngraph::check::Budgets;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::grad::{self, BooleanPolicy, DerivOptions};
use noble_ngraph::ir::{GraphBuilder, NodeId, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::train::{self, Point, Stopped, TrainControl, Trained};

/// A named edit of the `adam/0` hyperparameters.
type AdamEdit = (&'static str, fn(&mut train::Adam));

fn run(c: &train::Contract, steps: u64) -> Result<Trained, Box<Stopped>> {
    train::train(&IDS, c, None, steps, &mut TrainControl::default())
}

fn code(r: Result<Trained, Box<Stopped>>) -> String {
    match r {
        Ok(_) => "trained".to_owned(),
        Err(s) => s.diagnostic.code_string(),
    }
}

fn with_point(x: f32) -> Vec<Point> {
    let mut d = points();
    d.insert(
        2,
        Point {
            inputs: vec![x],
            targets: vec![1.0],
        },
    );
    d
}

#[test]
fn points_the_reference_refuses_stop_training_unpublished() {
    for (guarded, input_fact, want) in
        [(true, false, "G-FACT-FALSE"), (false, true, "E-FACT-INPUT")]
    {
        let model = checked(linear(guarded, input_fact));
        let c = contract(model.clone(), with_point(5.0));
        for steps in [0, 1, 4] {
            let e = run(&c, steps).expect_err("refused point");
            assert_eq!(e.diagnostic.code_string(), want);
            assert_eq!(e.published_steps, 0);
            let s0 = e
                .last
                .as_ref()
                .expect("S0 stays the last published snapshot");
            let obs = eval::reference(
                &model,
                &eval::f32_inputs(&[5.0]),
                Some(s0),
                &mut Control::default(),
            )
            .obs;
            match (want, obs) {
                ("G-FACT-FALSE", Obs::GuardFailed { .. }) => {}
                ("E-FACT-INPUT", Obs::Refused(d)) => assert_eq!(d.code_string(), want),
                (_, o) => panic!("reference disagrees: {o:?}"),
            }
        }
        let fine = contract(model, with_point(4.0));
        assert!(run(&fine, 4).is_ok(), "x = 4 is inside the range");
    }
}

#[test]
fn the_checked_objective_equals_the_plain_objective_on_admitted_points() {
    let model = checked(linear(true, false));
    let loss = checked(mse());
    let opts = DerivOptions::default();
    for (x, w, b, t) in [
        (0.5f32, 1.25f32, -0.5f32, 2.0f32),
        (3.0, -2.0, 0.125, -1.0),
        (0.0, 0.1, 7.0, 7.0),
    ] {
        let slots = [x, w, b];
        let (l, g) = grad::objective::<f32>(&model, &loss, &slots, &[t], &opts);
        let (lc, gc) =
            grad::objective_checked(&model, &loss, &slots, &[t], &opts).expect("admitted");
        assert_eq!(l.to_bits(), lc.to_bits());
        assert_eq!(
            g.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            gc.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
    }
}

/// A loss-graph variant: the sum of its inputs (and param), output
/// `outputs` times.
fn loss_graph(
    params: bool,
    outputs: usize,
    inputs: usize,
    profile: SubProfile,
) -> noble_ngraph::CheckedGraph {
    let mut b = GraphBuilder::new(profile, Subject::authored(&IDS, "tests/loss variant"));
    let f32 = profile == SubProfile::F32;
    let ty = if f32 {
        ScalarType::F32
    } else {
        ScalarType::I64
    };
    let mut terms: Vec<NodeId> = (0..inputs).map(|_| b.input(ty)).collect();
    if params {
        terms.push(b.param());
    }
    let mut acc = match terms.first() {
        Some(t) => *t,
        None => b.f32_const(1.0),
    };
    for t in terms.iter().skip(1) {
        acc = b.op(if f32 {
            Op::F32Add(acc, *t)
        } else {
            Op::I64Add(acc, *t)
        });
    }
    for _ in 0..outputs {
        b.output(acc);
    }
    checked(b.finish())
}

#[test]
fn the_loss_graph_interface_is_admitted() {
    let model = checked(linear(false, false));
    let base = contract(model.clone(), points());
    let with_loss = |loss| train::Contract {
        loss,
        ..base.clone()
    };
    let cases = [
        with_loss(loss_graph(true, 1, 2, SubProfile::F32)),
        with_loss(loss_graph(false, 2, 2, SubProfile::F32)),
        with_loss(loss_graph(false, 1, 0, SubProfile::F32)),
        with_loss(loss_graph(false, 1, 2, SubProfile::I64)),
    ];
    for c in &cases {
        assert_eq!(code(run(c, 1)), "E-INTERFACE");
        assert_eq!(
            train::admit(&c.model, &c.loss, &c.params, &c.deriv)
                .err()
                .map(|d| d.code_string())
                .as_deref(),
            Some("E-INTERFACE")
        );
    }
    let mut extra_target = base.clone();
    extra_target.data[1].targets.push(0.0);
    assert_eq!(code(run(&extra_target, 1)), "E-INTERFACE");
    let mut missing_input = base.clone();
    missing_input.data[0].inputs.clear();
    assert_eq!(code(run(&missing_input, 1)), "E-INTERFACE");
    assert_eq!(code(run(&base, 1)), "trained");
}

#[test]
fn budgets_bound_training_before_any_step() {
    let c = contract(checked(linear(false, false)), points());
    let with = |b: Budgets, steps| {
        code(train::train(
            &IDS,
            &c,
            None,
            steps,
            &mut TrainControl {
                budgets: b,
                ..TrainControl::default()
            },
        ))
    };
    let d = Budgets::default();
    assert_eq!(
        with(
            Budgets {
                optimizer_steps: 3,
                ..d.clone()
            },
            5
        ),
        "E-BUDGET-STEPS"
    );
    assert_eq!(
        with(
            Budgets {
                optimizer_steps: 3,
                ..d.clone()
            },
            3
        ),
        "trained"
    );
    assert_eq!(
        with(
            Budgets {
                optimizer_steps: 1_000_001,
                ..d.clone()
            },
            1
        ),
        "E-PROFILE"
    );
    assert_eq!(
        with(
            Budgets {
                backward_scratch: 8,
                ..d.clone()
            },
            1
        ),
        "E-BUDGET-BACKWARD-SCRATCH"
    );
    assert_eq!(
        with(
            Budgets {
                snapshot_bytes: 4,
                ..d.clone()
            },
            1
        ),
        "E-BUDGET-SNAPSHOT-BYTES"
    );
    assert_eq!(
        with(
            Budgets {
                snapshot_bytes: 8,
                ..d
            },
            1
        ),
        "trained"
    );
}

#[test]
fn replay_starts_from_the_recorded_parent() {
    let c = contract(checked(linear(false, false)), points());
    let mut none = TrainControl::default();
    let s_n = run(&c, 4).expect("trains");
    let child = train::train(&IDS, &c, Some(&s_n.snapshot), 3, &mut none).expect("trains on");
    assert_eq!(child.provenance.parent, s_n.snapshot.id());
    assert_eq!(s_n.provenance.parent, s_n.initial.id());

    let again = train::replay(&IDS, &c, &child.provenance, Some(&s_n.snapshot), &mut none)
        .expect("replays");
    assert_eq!(again.snapshot, child.snapshot);
    assert_eq!(again.provenance, child.provenance);
    assert!(
        train::replay(&IDS, &c, &s_n.provenance, None, &mut none).is_ok(),
        "seed replay"
    );

    let mut refuse = |c: &train::Contract, p: &train::Provenance, parent| {
        code(train::replay(&IDS, c, p, parent, &mut none))
    };
    assert_eq!(refuse(&c, &child.provenance, None), "E-SNAPSHOT-STALE");
    assert_eq!(
        refuse(&c, &child.provenance, Some(&child.snapshot)),
        "E-SNAPSHOT-STALE"
    );
    let mut data = c.clone();
    data.data[0].targets[0] = 9.0;
    assert_eq!(
        refuse(&data, &child.provenance, Some(&s_n.snapshot)),
        "E-SNAPSHOT-STALE"
    );
    let mut seed = c.clone();
    seed.seed ^= 1;
    assert_eq!(refuse(&seed, &s_n.provenance, None), "E-SNAPSHOT-STALE");
    let mut lr = c.clone();
    lr.adam.lr = 0.03;
    assert_eq!(refuse(&lr, &s_n.provenance, None), "E-SNAPSHOT-STALE");
    let mut deriv = c.clone();
    deriv
        .deriv
        .boolean
        .insert(NodeId(3), BooleanPolicy::StraightThrough);
    assert_eq!(refuse(&deriv, &s_n.provenance, None), "E-SNAPSHOT-STALE");
    let mut forged = child.provenance.clone();
    forged.result = [9; 32];
    assert_eq!(
        refuse(&c, &forged, Some(&s_n.snapshot)),
        "E-SNAPSHOT-REPLAY"
    );
    let mut state = child.provenance.clone();
    state.optimizer_state = [3; 32];
    assert_eq!(refuse(&c, &state, Some(&s_n.snapshot)), "E-SNAPSHOT-REPLAY");
}

#[test]
fn adam_values_outside_the_optimizer_domain_are_refused_by_train_and_replay() {
    let c = contract(checked(linear(false, false)), points());
    let t = run(&c, 2).expect("trains");
    let edits: [AdamEdit; 8] = [
        ("lr 0", |a| a.lr = 0.0),
        ("lr < 0", |a| a.lr = -0.02),
        ("lr NaN", |a| a.lr = f64::NAN),
        ("beta1 1", |a| a.beta1 = 1.0),
        ("beta1 < 0", |a| a.beta1 = -0.5),
        ("beta2 inf", |a| a.beta2 = f64::INFINITY),
        ("eps 0", |a| a.eps = 0.0),
        ("eps inf", |a| a.eps = f64::INFINITY),
    ];
    for (name, edit) in edits {
        let mut bad = c.clone();
        edit(&mut bad.adam);
        let e = run(&bad, 1).expect_err(name);
        assert_eq!(e.diagnostic.code_string(), "E-PROFILE", "{name}");
        assert!(e.last.is_none(), "{name}: refused before S0");
        // A provenance record naming the same values replays nothing.
        let mut p = t.provenance.clone();
        edit(&mut p.optimizer);
        assert_eq!(
            code(train::replay(
                &IDS,
                &bad,
                &p,
                None,
                &mut TrainControl::default()
            )),
            "E-PROFILE",
            "{name}"
        );
    }
    // The domain's edges are admitted: zero (also `-0`) betas and the least
    // positive learning rate.
    let mut edge = c;
    edge.adam.beta1 = 0.0;
    edge.adam.beta2 = -0.0;
    edge.adam.lr = f64::from_bits(1);
    run(&edge, 1).expect("inside the domain");
}

#[test]
fn opt_ins_must_name_a_node_they_apply_to_in_train_and_replay() {
    // `linear`: %0 input, %1 and %2 params, %3 f32.mul, %4 f32.add.
    let c = contract(checked(linear(false, false)), points());
    let t = run(&c, 2).expect("trains");
    let boolean = |n: u32, p: BooleanPolicy| {
        let mut d = DerivOptions::default();
        d.boolean.insert(NodeId(n), p);
        d
    };
    let index = |n: u32, p: grad::IndexPolicy| {
        let mut d = DerivOptions::default();
        d.index.insert(NodeId(n), p);
        d
    };
    let cases = [
        (
            boolean(4_000_000_000, BooleanPolicy::StraightThrough),
            "E-DERIV-BOOLEAN",
        ),
        (
            boolean(3, BooleanPolicy::StraightThrough),
            "E-DERIV-BOOLEAN",
        ),
        (boolean(3, BooleanPolicy::Refuse), "E-DERIV-BOOLEAN"),
        (
            boolean(4, BooleanPolicy::ClampedLinear(0.5)),
            "E-DERIV-BOOLEAN",
        ),
        (index(3, grad::IndexPolicy::Zero), "E-DERIV-INDEX"),
        (
            index(99, grad::IndexPolicy::BackwardDifference),
            "E-DERIV-INDEX",
        ),
    ];
    for (deriv, want) in cases {
        let mut bad = c.clone();
        bad.deriv = deriv.clone();
        assert_eq!(code(run(&bad, 1)), want, "{deriv:?}");
        // Replay re-admits recorded opt-ins exactly as training does.
        let mut p = t.provenance.clone();
        p.deriv = deriv;
        assert_eq!(
            code(train::replay(
                &IDS,
                &bad,
                &p,
                None,
                &mut TrainControl::default()
            )),
            want
        );
    }
}
