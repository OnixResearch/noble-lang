//! B5: train F1 with `adam/0`, a recorded seed and 500 steps against a fixed
//! 64-point target set; only parameter bytes change; S₀ is stable; replay is
//! bitwise; wrong/stale identities and old-subject evidence are refused.

use noble_ngraph_experiment::eval::{f32_inputs, reference, Control};
use noble_ngraph_experiment::evidence::{capture, EvidenceRecord, NotApplicable};
use noble_ngraph_experiment::fixtures;
use noble_ngraph_experiment::ir::hex;
use noble_ngraph_experiment::snapshot::{layout_key, ParamSnapshot};
use noble_ngraph_experiment::suites::{b5_contract, B5_DATA_SEED, B5_SEED, B5_STEPS};
use noble_ngraph_experiment::text;
use noble_ngraph_experiment::train::{self, data_digest, TrainControl};
use noble_ngraph_experiment::{check, Code, Obs};

fn outputs_on(c: &train::Contract, s: &ParamSnapshot) -> Vec<Obs> {
    c.data
        .iter()
        .map(|p| {
            reference(
                &c.model,
                &f32_inputs(&p.inputs),
                Some(s),
                &Control::default(),
            )
            .obs
        })
        .collect()
}

#[test]
fn train_f1_publishes_only_new_param_snapshots() {
    let c = b5_contract(B5_SEED, B5_DATA_SEED);
    let id_before = c.model.id();
    let text_before = text::print(c.model.graph());
    let data_before = data_digest(&c.data);
    let s0 =
        ParamSnapshot::new(&c.model, &train::init_params(B5_SEED, c.params.len())).expect("S0");
    let s0_bytes = s0.bytes().to_vec();
    let s0_out = outputs_on(&c, &s0);

    let t = train::train(&c, None, B5_STEPS, &TrainControl::default()).expect("trains");
    assert_eq!(t.initial, s0);
    let (first, last) = (t.losses[0], *t.losses.last().expect("losses"));
    assert!(last <= 0.1 * first, "loss {first} → {last}");

    // Identity and every non-param byte unchanged.
    assert_eq!(c.model.id(), id_before);
    assert_eq!(c.model.graph().logical_id(), id_before);
    assert_eq!(text::print(c.model.graph()), text_before);
    assert_eq!(data_digest(&c.data), data_before);
    assert_eq!(t.snapshot.graph(), id_before);
    assert_ne!(t.snapshot.bytes(), s0.bytes());

    // S₀ still evaluates bitwise-identically and its bytes/id are unchanged.
    assert_eq!(t.initial.bytes(), &s0_bytes[..]);
    assert_eq!(outputs_on(&c, &t.initial), s0_out);
    assert_eq!(
        ParamSnapshot::from_parts(s0.graph(), s0.layout(), s0.slots().to_vec(), s0_bytes).id(),
        s0.id()
    );

    // Replay from provenance reproduces the final snapshot bytes.
    let replayed = train::replay(&c, &t.provenance).expect("replays");
    assert_eq!(replayed.bytes(), t.snapshot.bytes());
    assert_eq!(hex(&replayed.id()), t.provenance.result);
    assert_eq!(t.provenance.steps, B5_STEPS);
    assert_eq!(t.provenance.seed, B5_SEED);
}

#[test]
fn wrong_graph_layout_and_stale_snapshots_are_refused() {
    let c = b5_contract(B5_SEED, B5_DATA_SEED);
    let s = ParamSnapshot::new(&c.model, &fixtures::F1_FIXED).expect("snapshot");
    let x = f32_inputs(&[0.25, -0.5]);
    // Wrong graph: an F2 snapshot.
    let f2 = check(fixtures::f2()).expect("F2");
    let s2 = ParamSnapshot::new(&f2, &[1.0, 2.0, 3.0, 4.0]).expect("F2 snapshot");
    let r = reference(&c.model, &x, Some(&s2), &Control::default());
    assert!(
        matches!(&r.obs, Obs::Refused(d) if d.code == Code::SnapshotStale),
        "{r:?}"
    );
    assert_eq!(r.evaluated_nodes, 0);
    // Wrong layout: same graph id, permuted param slots.
    let mut slots = s.slots().to_vec();
    slots.swap(0, 1);
    let bad = ParamSnapshot::from_parts(
        s.graph(),
        layout_key(c.model.graph()),
        slots,
        s.bytes().to_vec(),
    );
    let r = reference(&c.model, &x, Some(&bad), &Control::default());
    assert!(
        matches!(&r.obs, Obs::Refused(d) if d.code == Code::SnapshotLayout),
        "{r:?}"
    );
    // Stale: a snapshot of a previous revision of the graph (different subject text).
    let mut g_old = fixtures::f1();
    g_old.subject = noble_ngraph_experiment::ir::Subject::authored("issue14/F1 previous revision");
    let old = check(g_old).expect("old F1");
    let s_old = ParamSnapshot::new(&old, &fixtures::F1_FIXED).expect("old snapshot");
    let r = reference(&c.model, &x, Some(&s_old), &Control::default());
    assert!(
        matches!(&r.obs, Obs::Refused(d) if d.code == Code::SnapshotStale),
        "{r:?}"
    );
    // Training refuses a stale initial snapshot too.
    let e = train::train(&c, Some(&s_old), 1, &TrainControl::default()).unwrap_err();
    assert_eq!(e.diagnostic.code, Code::SnapshotStale);
}

#[test]
fn old_subject_evidence_is_refused_for_a_captured_subject() {
    let c = b5_contract(B5_SEED, B5_DATA_SEED);
    let s = ParamSnapshot::new(&c.model, &fixtures::F1_FIXED).expect("snapshot");
    let ev = EvidenceRecord::new("L2/L3/L4 agree on the B5 data", &c.model, Some(&s));
    assert_eq!(ev.applies(&c.model, Some(&s)), Ok(()));
    let captured = check(capture(&c.model, &s).expect("captures")).expect("captured graph checks");
    assert_ne!(captured.id(), c.model.id());
    assert_eq!(
        ev.applies(&captured, None),
        Err(NotApplicable::OtherSubject)
    );
    // The captured subject evaluates like the original with its snapshot.
    for p in &c.data {
        let a = reference(
            &c.model,
            &f32_inputs(&p.inputs),
            Some(&s),
            &Control::default(),
        )
        .obs;
        let b = reference(&captured, &f32_inputs(&p.inputs), None, &Control::default()).obs;
        assert!(a.obs_eq(&b));
    }
    // A different snapshot of the same graph is not covered either.
    let s1 = ParamSnapshot::new(&c.model, &train::init_params(7, c.params.len())).expect("s1");
    assert_eq!(
        ev.applies(&c.model, Some(&s1)),
        Err(NotApplicable::OtherSnapshot)
    );
}

#[test]
fn nonfinite_update_publishes_nothing() {
    let mut c = b5_contract(B5_SEED, B5_DATA_SEED);
    c.data[3].targets[0] = f32::INFINITY;
    let e = train::train(&c, None, 5, &TrainControl::default()).unwrap_err();
    assert_eq!(e.diagnostic.code, Code::NonfiniteUpdate);
    assert_eq!(e.published_steps, 0);
    let s0 =
        ParamSnapshot::new(&c.model, &train::init_params(B5_SEED, c.params.len())).expect("S0");
    assert_eq!(e.last.as_ref().map(ParamSnapshot::id), Some(s0.id()));
}
