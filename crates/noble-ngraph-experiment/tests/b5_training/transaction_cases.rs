use noble_ngraph_experiment::opt::{
    run_trial, AttemptEvent, AttemptLog, Candidate, Checkpoint, Counters, FaultInjection,
    FrozenDataset, FrozenRow, OptSnapshot, RngCheckpoint, SnapshotContext, TransitionInput,
    TrialFailure, TrialStatus,
};

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
    assert_eq!(
        snapshot.parameters().len(),
        2,
        "fixture has two parameter lanes"
    );
    assert!(
        snapshot.state_bytes().is_empty(),
        "State0 has no persistent bytes"
    );
    Checkpoint::new(
        snapshot,
        RngCheckpoint {
            seed: 0x0058_2026,
            draws: 0,
        },
        0,
        Counters::default(),
    )
    .expect("whole-trial checkpoint")
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

fn assert_fault_events(
    records: &[noble_ngraph_experiment::opt::AttemptRecord],
    after_steps: usize,
) {
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
    expected_parent_id: noble_ngraph_experiment::opt::Digest,
    result: &noble_ngraph_experiment::opt::TrialResult,
) {
    let provenance = result.checkpoint.snapshot.provenance();
    assert_eq!(provenance.optimizer_code, candidate.id());
    assert_eq!(provenance.parent_snapshot, Some(expected_parent_id));
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
    let prefix = run_trial(
        &candidate,
        Some(&parent),
        &dataset,
        2,
        FaultInjection::None,
        &mut AttemptLog::default(),
    )
    .expect("prefix trial completes");
    let result = run_trial(
        &candidate,
        Some(&prefix.checkpoint),
        &dataset,
        1,
        FaultInjection::None,
        &mut AttemptLog::default(),
    )
    .expect("final trial step completes");
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
    assert_opt_provenance(
        &candidate,
        &dataset,
        &parent,
        prefix.checkpoint.snapshot.id(),
        &result,
    );
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
