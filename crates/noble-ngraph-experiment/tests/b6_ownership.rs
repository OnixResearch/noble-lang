//! B6: retained owners outlive invocations; duplicate owners release in any
//! order; scratch is released exactly once on normal return, guard failure,
//! preflight refusal, exhaustion and cancellation; no partial snapshot is
//! published and retained digests do not change.
//!
//! Instrumentation is the crate's scratch-lease counters (a counting global
//! allocator would need `unsafe`, which the workspace forbids).

use std::sync::Arc;

use noble_ngraph_experiment::compiled::Compiled;
use noble_ngraph_experiment::eval::{f32_inputs, reference, Control, ScratchCounters};
use noble_ngraph_experiment::fixtures::{self, F1_FIXED};
use noble_ngraph_experiment::snapshot::ParamSnapshot;
use noble_ngraph_experiment::suites::{b5_contract, B5_DATA_SEED, B5_SEED};
use noble_ngraph_experiment::train::{self, TrainControl};
use noble_ngraph_experiment::{check, BudgetKind, CheckedGraph, Code, Obs};

fn f1() -> CheckedGraph {
    check(fixtures::f1()).expect("F1")
}

fn recomputed_id(s: &ParamSnapshot) -> [u8; 32] {
    ParamSnapshot::from_parts(
        s.graph(),
        s.layout(),
        s.slots().to_vec(),
        s.bytes().to_vec(),
    )
    .id()
}

#[test]
fn retained_snapshot_is_used_after_the_invocation_exits() {
    fn invocation() -> (CheckedGraph, ParamSnapshot) {
        let c = b5_contract(B5_SEED, B5_DATA_SEED);
        let t = train::train(&c, None, 3, &TrainControl::default()).expect("trains");
        (c.model.clone(), t.snapshot)
    }
    let (g, s) = invocation();
    let id = s.id();
    let r = reference(
        &g,
        &f32_inputs(&[0.5, -0.25]),
        Some(&s),
        &Control::default(),
    );
    assert!(matches!(r.obs, Obs::Normal(_)));
    assert_eq!(recomputed_id(&s), id);
}

#[test]
fn duplicate_owners_release_in_both_orders() {
    let g = f1();
    for order in [[0usize, 1], [1, 0]] {
        let s = ParamSnapshot::new(&g, &F1_FIXED).expect("snapshot");
        let id = s.id();
        let weak = Arc::downgrade(&s.bytes_arc());
        let mut owners = [Some(s.clone()), Some(s)];
        owners[order[0]] = None;
        let survivor = owners[order[1]].as_ref().expect("survivor");
        assert!(
            weak.upgrade().is_some(),
            "bytes live while one owner remains"
        );
        assert_eq!(recomputed_id(survivor), id, "survivor digest unchanged");
        let r = reference(
            &g,
            &f32_inputs(&[1.0, 2.0]),
            Some(survivor),
            &Control::default(),
        );
        assert!(matches!(r.obs, Obs::Normal(_)));
        owners[order[1]] = None;
        assert!(
            weak.upgrade().is_none(),
            "released exactly when the last owner drops"
        );
    }
    // Graph owners: a checked graph and a compiled evaluator share the graph.
    for first_checked in [true, false] {
        let c = f1();
        let weak = Arc::downgrade(&c.graph_arc());
        let comp = Compiled::new(&c);
        if first_checked {
            drop(c);
            assert!(weak.upgrade().is_some());
            drop(comp);
        } else {
            drop(comp);
            assert!(weak.upgrade().is_some());
            drop(c);
        }
        assert!(weak.upgrade().is_none());
    }
}

#[test]
fn scratch_is_released_exactly_once_on_every_exit() {
    let g = f1();
    let s = ParamSnapshot::new(&g, &F1_FIXED).expect("snapshot");
    let (gid, sid) = (g.id(), s.id());
    let comp = Compiled::new(&g);
    let x = f32_inputs(&[0.5, 1.5]);
    let layers = g.layer_count();
    assert!(layers > 3);
    let mut seen = Vec::new();
    for k in 0..layers {
        let cancel = move |l: u32| l == k;
        for use_compiled in [false, true] {
            let counters = ScratchCounters::default();
            let control = Control {
                cancel: Some(&cancel),
                scratch: Some(&counters),
                ..Control::default()
            };
            let r = if use_compiled {
                comp.evaluate(&x, Some(&s), &control)
            } else {
                reference(&g, &x, Some(&s), &control)
            };
            assert_eq!(r.obs, Obs::Cancelled, "layer {k}");
            assert_eq!(
                counters.snapshot(),
                (1, 1, 0),
                "layer {k} compiled={use_compiled}"
            );
            seen.push(r.evaluated_nodes);
        }
    }
    // Exhaustion (fuel) mid-evaluation.
    for use_compiled in [false, true] {
        let counters = ScratchCounters::default();
        let control = Control {
            fuel: Some(20),
            scratch: Some(&counters),
            ..Control::default()
        };
        let r = if use_compiled {
            comp.evaluate(&x, Some(&s), &control)
        } else {
            reference(&g, &x, Some(&s), &control)
        };
        assert_eq!(r.obs, Obs::Exhausted(BudgetKind::Fuel));
        assert_eq!(counters.snapshot(), (1, 1, 0));
    }
    // Guard failure and preflight refusal (no scratch acquired at all).
    let guarded = check(
        noble_ngraph_experiment::text::parse(
            "ngraph-ir/0 ngraph-f32/0\nslot input f32\n%0 = input 0\n%1 = f32.add %0 %0\n%2 = guard %1 bit\n%3 = f32.select %1 %0 %1\nout %3\n",
            "b6 guard",
        )
        .expect("parses"),
    )
    .expect("checks");
    let gc = Compiled::new(&guarded);
    for use_compiled in [false, true] {
        let counters = ScratchCounters::default();
        let control = Control {
            scratch: Some(&counters),
            ..Control::default()
        };
        let ev = |inp: &[f32]| {
            if use_compiled {
                gc.evaluate(&f32_inputs(inp), None, &control)
            } else {
                reference(&guarded, &f32_inputs(inp), None, &control)
            }
        };
        assert!(matches!(ev(&[0.25]).obs, Obs::GuardFailed { .. }));
        assert_eq!(counters.snapshot(), (1, 1, 0));
        assert!(matches!(&ev(&[]).obs, Obs::Refused(d) if d.code == Code::Interface));
        assert_eq!(
            counters.snapshot(),
            (1, 1, 0),
            "preflight refusal acquires nothing"
        );
        assert!(matches!(ev(&[0.5]).obs, Obs::Normal(_)));
        assert_eq!(counters.snapshot(), (2, 2, 0));
    }
    assert_eq!(
        (g.id(), recomputed_id(&s)),
        (gid, sid),
        "retained digests unchanged"
    );
}

#[test]
fn reused_workspace_returns_fresh_results() {
    let g = f1();
    let s = ParamSnapshot::new(&g, &F1_FIXED).expect("snapshot");
    let comp = Compiled::new(&g);
    let mut w = comp.workspace();
    let a = comp.evaluate_in(
        &mut w,
        &f32_inputs(&[0.5, 1.5]),
        Some(&s),
        &Control::default(),
    );
    let a_copy = a.obs.clone();
    let b = comp.evaluate_in(
        &mut w,
        &f32_inputs(&[-1.0, 0.25]),
        Some(&s),
        &Control::default(),
    );
    assert_eq!(
        a.obs, a_copy,
        "an earlier result is not overwritten by the next call"
    );
    assert!(!a.obs.obs_eq(&b.obs));
}

#[test]
fn cancelled_or_failed_training_publishes_no_partial_snapshot() {
    let c = b5_contract(B5_SEED, B5_DATA_SEED);
    let full = train::train(&c, None, 4, &TrainControl::default()).expect("trains");
    let three = train::train(&c, None, 3, &TrainControl::default()).expect("trains");
    let counters = ScratchCounters::default();
    let cancel = |t: u64| t == 4;
    let e = train::train(
        &c,
        None,
        10,
        &TrainControl {
            cancel_before_step: Some(&cancel),
            scratch: Some(&counters),
        },
    )
    .unwrap_err();
    assert_eq!(e.diagnostic.code, Code::Cancelled);
    assert_eq!(e.published_steps, 3);
    let last = e.last.expect("last published");
    assert_eq!(
        last.bytes(),
        three.snapshot.bytes(),
        "last published = after step 3"
    );
    assert_ne!(
        last.bytes(),
        full.snapshot.bytes(),
        "step 4 never published"
    );
    let (acq, rel, live) = counters.snapshot();
    assert_eq!((acq, rel, live), (3, 3, 0));
    assert_eq!(recomputed_id(&three.snapshot), three.snapshot.id());
}
