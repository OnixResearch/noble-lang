#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::missing_const_fn,
    tigerstyle::explicit_defaults,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! B5: train F1 with `adam/0`, a recorded seed and 500 steps against a fixed
//! 64-point target set; only parameter bytes change; S₀ is stable; replay is
//! bitwise; wrong/stale identities and old-subject evidence are refused.

use noble_ngraph::eval::{f32_inputs, reference, Control};
use noble_ngraph::format::graph;
use noble_ngraph::snapshot::{layout_key, ParamSnapshot};
use noble_ngraph::train::{self, data_digest, TrainControl};
use noble_ngraph::{check, Code, Obs};
use noble_ngraph_experiment::evidence::{capture, EvidenceRecord, NotApplicable};
use noble_ngraph_experiment::fixtures;
use noble_ngraph_experiment::opt::{
    run_trial, AttemptEvent, AttemptLog, Candidate, Checkpoint, Counters, FaultInjection,
    FrozenDataset, FrozenRow, OptSnapshot, RngCheckpoint, SnapshotContext, TransitionInput,
    TrialFailure, TrialStatus,
};
use noble_ngraph_experiment::scheme::BLAKE3;
use noble_ngraph_experiment::suites::{b5_contract, B5_DATA_SEED, B5_SEED, B5_STEPS};

fn outputs_on(c: &train::Contract, s: &ParamSnapshot) -> Vec<Obs> {
    c.data
        .iter()
        .map(|p| {
            reference(
                &c.model,
                &f32_inputs(&p.inputs),
                Some(s),
                &mut Control::default(),
            )
            .obs
        })
        .collect()
}

#[test]
fn train_f1_publishes_only_new_param_snapshots() {
    let c = b5_contract(B5_SEED, B5_DATA_SEED);
    let id_before = c.model.id();
    let text_before = graph::print(c.model.graph());
    let data_before = data_digest(&BLAKE3, &c.data);
    let s0 = ParamSnapshot::new(
        &BLAKE3,
        &c.model,
        &train::init_params(B5_SEED, c.params.len()),
    )
    .expect("S0");
    let s0_bytes = s0.bytes().to_vec();
    let s0_out = outputs_on(&c, &s0);

    let t =
        train::train(&BLAKE3, &c, None, B5_STEPS, &mut TrainControl::default()).expect("trains");
    assert_eq!(t.initial, s0);
    let (first, last) = (t.losses[0], *t.losses.last().expect("losses"));
    assert!(last <= 0.1 * first, "loss {first} → {last}");

    // Identity and every non-param byte unchanged.
    assert_eq!(c.model.id(), id_before);
    assert_eq!(c.model.graph().logical_id(&BLAKE3), id_before);
    assert_eq!(graph::print(c.model.graph()), text_before);
    assert_eq!(data_digest(&BLAKE3, &c.data), data_before);
    assert_eq!(t.snapshot.graph(), id_before);
    assert_ne!(t.snapshot.bytes(), s0.bytes());

    // S₀ still evaluates bitwise-identically and its bytes/id are unchanged.
    assert_eq!(t.initial.bytes(), &s0_bytes[..]);
    assert_eq!(outputs_on(&c, &t.initial), s0_out);
    assert_eq!(
        ParamSnapshot::from_parts(
            &BLAKE3,
            s0.graph(),
            s0.layout(),
            s0.slots().to_vec(),
            s0_bytes
        )
        .id(),
        s0.id()
    );

    // Replay from provenance reproduces the final snapshot bytes.
    let replayed = train::replay(
        &BLAKE3,
        &c,
        &t.provenance,
        None,
        &mut TrainControl::default(),
    )
    .expect("replays")
    .snapshot;
    assert_eq!(replayed.bytes(), t.snapshot.bytes());
    assert_eq!(replayed.id(), t.provenance.result);
    assert_eq!(t.provenance.steps, B5_STEPS);
    assert_eq!(t.provenance.seed, B5_SEED);
}

#[test]
fn wrong_graph_layout_and_stale_snapshots_are_refused() {
    let c = b5_contract(B5_SEED, B5_DATA_SEED);
    let s = ParamSnapshot::new(&BLAKE3, &c.model, &fixtures::F1_FIXED).expect("snapshot");
    let x = f32_inputs(&[0.25, -0.5]);
    // Wrong graph: an F2 snapshot.
    let f2 = check(&BLAKE3, fixtures::f2()).expect("F2");
    let s2 = ParamSnapshot::new(&BLAKE3, &f2, &[1.0, 2.0, 3.0, 4.0]).expect("F2 snapshot");
    let r = reference(&c.model, &x, Some(&s2), &mut Control::default());
    assert!(
        matches!(&r.obs, Obs::Refused(d) if d.code == Code::SnapshotStale),
        "{r:?}"
    );
    assert_eq!(r.evaluated_nodes, 0);
    // Wrong layout: same graph id, permuted param slots.
    let mut slots = s.slots().to_vec();
    slots.swap(0, 1);
    let bad = ParamSnapshot::from_parts(
        &BLAKE3,
        s.graph(),
        layout_key(&BLAKE3, c.model.graph()),
        slots,
        s.bytes().to_vec(),
    );
    let r = reference(&c.model, &x, Some(&bad), &mut Control::default());
    assert!(
        matches!(&r.obs, Obs::Refused(d) if d.code == Code::SnapshotLayout),
        "{r:?}"
    );
    // Stale: a snapshot of a previous revision of the graph (different subject text).
    let mut g_old = fixtures::f1();
    g_old.subject = noble_ngraph::ir::Subject::authored(&BLAKE3, "issue14/F1 previous revision");
    let old = check(&BLAKE3, g_old).expect("old F1");
    let s_old = ParamSnapshot::new(&BLAKE3, &old, &fixtures::F1_FIXED).expect("old snapshot");
    let r = reference(&c.model, &x, Some(&s_old), &mut Control::default());
    assert!(
        matches!(&r.obs, Obs::Refused(d) if d.code == Code::SnapshotStale),
        "{r:?}"
    );
    // Training refuses a stale initial snapshot too.
    let e = train::train(&BLAKE3, &c, Some(&s_old), 1, &mut TrainControl::default()).unwrap_err();
    assert_eq!(e.diagnostic.code, Code::SnapshotStale);
}

#[test]
fn old_subject_evidence_is_refused_for_a_captured_subject() {
    let c = b5_contract(B5_SEED, B5_DATA_SEED);
    let s = ParamSnapshot::new(&BLAKE3, &c.model, &fixtures::F1_FIXED).expect("snapshot");
    let ev = EvidenceRecord::new("L2/L3/L4 agree on the B5 data", &c.model, Some(&s));
    assert_eq!(ev.applies(&c.model, Some(&s)), Ok(()));
    let captured =
        check(&BLAKE3, capture(&c.model, &s).expect("captures")).expect("captured graph checks");
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
            &mut Control::default(),
        )
        .obs;
        let b = reference(
            &captured,
            &f32_inputs(&p.inputs),
            None,
            &mut Control::default(),
        )
        .obs;
        assert!(a.obs_eq(&b));
    }
    // A different snapshot of the same graph is not covered either.
    let s1 =
        ParamSnapshot::new(&BLAKE3, &c.model, &train::init_params(7, c.params.len())).expect("s1");
    assert_eq!(
        ev.applies(&c.model, Some(&s1)),
        Err(NotApplicable::OtherSnapshot)
    );
}

#[test]
fn nonfinite_update_publishes_nothing() {
    let mut c = b5_contract(B5_SEED, B5_DATA_SEED);
    c.data[3].targets[0] = f32::INFINITY;
    let e = train::train(&BLAKE3, &c, None, 5, &mut TrainControl::default()).unwrap_err();
    assert_eq!(e.diagnostic.code, Code::NonfiniteUpdate);
    assert_eq!(e.published_steps, 0);
    let s0 = ParamSnapshot::new(
        &BLAKE3,
        &c.model,
        &train::init_params(B5_SEED, c.params.len()),
    )
    .expect("S0");
    assert_eq!(e.last.as_ref().map(ParamSnapshot::id), Some(s0.id()));
}

const OPT_SGD: &str = r#"{
  "format":"ngraph-opt/1","n":2,
  "inputs":["p","g","eta","step","state"],"state":"State0","effects":[],
  "budgets":{"nodes":16,"state_bytes":0,"work":128,"output_lanes":64},
  "nodes":[
    {"id":0,"op":"broadcast","operands":["$eta"],"type":"F32[n]"},
    {"id":1,"op":"Binary32.mul","operands":[0,"$g"],"type":"F32[n]"},
    {"id":2,"op":"Binary32.sub","operands":["$p",1],"type":"F32[n]"}
  ],"outputs":{"parameters":2,"state":"State0"}
}"#;

fn opt_trial_fixture() -> (Candidate, FrozenDataset, Checkpoint) {
    let candidate = opt_candidate();
    let dataset = opt_dataset();
    let checkpoint = opt_checkpoint(&candidate, &dataset);
    (candidate, dataset, checkpoint)
}

fn opt_candidate() -> Candidate {
    Candidate::parse(OPT_SGD.as_bytes()).expect("candidate admits")
}

fn opt_dataset() -> FrozenDataset {
    FrozenDataset::freeze(vec![
        FrozenRow {
            gradients: vec![0x3f80_0000, 0xc000_0000],
            eta: 0x3f00_0000,
        },
        FrozenRow {
            gradients: vec![0x4000_0000, 0x3f80_0000],
            eta: 0x3e80_0000,
        },
    ])
}

fn opt_checkpoint(candidate: &Candidate, dataset: &FrozenDataset) -> Checkpoint {
    let context = SnapshotContext {
        graph: [1; 32],
        loss: [2; 32],
        derivative: [3; 32],
        data_order: dataset.id(),
        seed: 0x0058_2026,
        tools: "rustc-test; noble-ngraph/0; lean-4.35.0".to_owned(),
    };
    let snapshot = OptSnapshot::root(candidate, &[0x3f80_0000, 0x4000_0000], &[], context)
        .expect("initial immutable snapshot");
    let checkpoint = Checkpoint::new(
        snapshot,
        RngCheckpoint {
            seed: 0x0058_2026,
            draws: 0,
        },
        0,
        Counters::default(),
    )
    .expect("whole-trial checkpoint");
    checkpoint
}

fn run_faulted_trial(
    candidate: &Candidate,
    parent: &Checkpoint,
    dataset: &FrozenDataset,
    fault: FaultInjection,
    attempts: &mut AttemptLog,
) -> noble_ngraph_experiment::opt::TrialResult {
    run_trial(candidate, Some(parent), dataset, 4, fault, attempts)
        .expect("injected fault rolls back")
}

fn assert_restored(result: &noble_ngraph_experiment::opt::TrialResult, parent: &Checkpoint) {
    assert_eq!(result.checkpoint, *parent);
    assert_eq!(
        result.checkpoint.snapshot.parameter_bytes(),
        parent.snapshot.parameter_bytes()
    );
    assert_eq!(result.checkpoint.snapshot.id(), parent.snapshot.id());
    assert_eq!(result.checkpoint.rng, parent.rng);
    assert_eq!(result.checkpoint.data_cursor, parent.data_cursor);
    assert_eq!(result.checkpoint.counters, parent.counters);
}

fn assert_fault_events(records: &[noble_ngraph_experiment::opt::AttemptRecord], after_steps: usize) {
    assert!(matches!(&records[0].event, AttemptEvent::StepStaged { .. }));
    assert!(
        matches!(&records[1].event, AttemptEvent::FaultInjected { after_steps: observed } if *observed == after_steps)
    );
    assert!(matches!(&records[2].event, AttemptEvent::RolledBack { .. }));
}

fn assert_opt_provenance(
    candidate: &Candidate,
    dataset: &FrozenDataset,
    parent: &Checkpoint,
    result: &noble_ngraph_experiment::opt::TrialResult,
) {
    let provenance = result.checkpoint.snapshot.provenance();
    assert_eq!(provenance.optimizer_code, candidate.id());
    assert_eq!(provenance.parent_snapshot, Some(parent.snapshot.id()));
    assert_ne!(
        provenance.parent_parameters,
        parent.snapshot.provenance().parent_parameters
    );
    assert_eq!(provenance.graph, [1; 32]);
    assert_eq!(provenance.loss, [2; 32]);
    assert_eq!(provenance.derivative, [3; 32]);
    assert_eq!(provenance.data_order, dataset.id());
    assert_eq!(provenance.seed, 0x0058_2026);
    assert_eq!(
        provenance.numeric_semantics,
        "Binary32/RNE/two-roundings/no-FMA/no-reassociation/gradual-underflow/nonfinite-refused/v1"
    );
    assert_eq!(provenance.tools, "rustc-test; noble-ngraph/0; lean-4.35.0");
    assert_eq!(provenance.step.map(|step| step.step_index), Some(2));
}

#[test]
fn opt_snapshot_provenance_binds_artifacts_and_keeps_parents_stable() {
    let (candidate, dataset, parent) = opt_trial_fixture();
    let parent_id = parent.snapshot.id();
    let parent_bytes = parent.snapshot.parameter_bytes();
    let parent_state = parent.snapshot.state_bytes().to_vec();
    let result = run_trial(
        &candidate,
        Some(&parent),
        &dataset,
        3,
        FaultInjection::None,
        &mut AttemptLog::default(),
    )
    .expect("trial completes");
    assert_eq!(result.status, TrialStatus::Completed);
    assert_ne!(result.checkpoint.snapshot.id(), parent_id);
    assert_eq!(parent.snapshot.id(), parent_id);
    assert_eq!(parent.snapshot.parameter_bytes(), parent_bytes);
    assert_eq!(parent.snapshot.state_bytes(), &parent_state);
    assert!(result.checkpoint.snapshot.state_bytes().is_empty());
    assert_eq!(
        result.checkpoint.counters,
        Counters {
            updates: 3,
            work: 12
        }
    );
    assert_eq!(result.checkpoint.data_cursor, 3);
    assert_eq!(result.checkpoint.rng.draws, 3);
    assert_opt_provenance(&candidate, &dataset, &parent, &result);
}

#[test]
fn opt_early_and_late_faults_restore_every_checkpoint_field_and_keep_attempts() {
    let (candidate, dataset, parent) = opt_trial_fixture();
    let mut attempts = AttemptLog::default();
    let early = run_faulted_trial(
        &candidate,
        &parent,
        &dataset,
        FaultInjection::AfterSteps(1),
        &mut attempts,
    );
    assert_eq!(
        early.status,
        TrialStatus::RolledBack(TrialFailure::InjectedFault { after_steps: 1 })
    );
    assert_restored(&early, &parent);
    let early_attempts = attempts.records().to_vec();
    assert_fault_events(&early_attempts, 1);

    let late = run_faulted_trial(
        &candidate,
        &parent,
        &dataset,
        FaultInjection::AfterSteps(3),
        &mut attempts,
    );
    assert_eq!(
        late.status,
        TrialStatus::RolledBack(TrialFailure::InjectedFault { after_steps: 3 })
    );
    assert_restored(&late, &parent);
    assert_eq!(&attempts.records()[..early_attempts.len()], &early_attempts);
    let late_events = &attempts.records()[early_attempts.len()..];
    assert_eq!(
        late_events
            .iter()
            .filter(|row| matches!(&row.event, AttemptEvent::StepStaged { .. }))
            .count(),
        3
    );
    assert!(matches!(
        &late_events[3].event,
        AttemptEvent::FaultInjected { after_steps: 3 }
    ));
    assert!(matches!(
        &late_events[4].event,
        AttemptEvent::RolledBack { .. }
    ));
}

#[test]
fn opt_missing_checkpoint_fails_closed_and_step_order_is_checked() {
    let (candidate, dataset, parent) = opt_trial_fixture();
    let mut attempts = AttemptLog::default();
    let missing = run_trial(
        &candidate,
        None,
        &dataset,
        2,
        FaultInjection::None,
        &mut attempts,
    )
    .expect_err("missing checkpoint refuses");
    assert_eq!(
        missing.class,
        noble_ngraph_experiment::opt::RefusalClass::Internal
    );
    assert_eq!(missing.location, "$.checkpoint");
    assert_eq!(attempts.records().len(), 1);
    assert_eq!(
        attempts.records()[0].event,
        AttemptEvent::CheckpointUnavailable
    );

    let transition = TransitionInput {
        parameters: parent.snapshot.parameters(),
        gradients: &[0x3f80_0000, 0x3f80_0000],
        eta: 0x3f00_0000,
        step: noble_ngraph_experiment::opt::StepDescriptor::new(1, 0, 0).expect("descriptor"),
        state: &[],
    };
    let output = candidate.evaluate(&transition).expect("candidate step");
    let out_of_order =
        noble_ngraph_experiment::opt::StepDescriptor::new(1, 1, 0).expect("descriptor");
    let refusal = OptSnapshot::publish(&candidate, &parent.snapshot, &output, out_of_order)
        .expect_err("backward/skipped step refused");
    assert_eq!(
        refusal.class,
        noble_ngraph_experiment::opt::RefusalClass::Unsupported
    );
    assert_eq!(refusal.location, "$.inputs.step.step_index");
}
