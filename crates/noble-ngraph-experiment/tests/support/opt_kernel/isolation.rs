use noble_ngraph_experiment::opt::{
    evaluate_frozen, Candidate, EvaluationRow, FrozenCandidate, FrozenEvaluation, StepDescriptor,
    TransitionInput,
};

fn evaluation_rows() -> Vec<EvaluationRow> {
    vec![
        EvaluationRow {
            parameters: vec![0x3f80_0000, 0x4000_0000],
            gradients: vec![0x3f80_0000, 0xc000_0000],
            eta: 0x3f00_0000,
            step: StepDescriptor::new(1, 0, 0).expect("descriptor"),
        },
        EvaluationRow {
            parameters: vec![0x4040_0000, 0x3f80_0000],
            gradients: vec![0x4000_0000, 0x3f80_0000],
            eta: 0x3e80_0000,
            step: StepDescriptor::new(1, 1, 1).expect("descriptor"),
        },
    ]
}

fn transition_input(row: &EvaluationRow) -> TransitionInput<'_> {
    TransitionInput {
        parameters: &row.parameters,
        gradients: &row.gradients,
        eta: row.eta,
        step: row.step,
        state: &[],
    }
}

fn assert_restarted(rows: &[noble_ngraph_experiment::opt::EvaluationRowResult], state: &[u8]) {
    let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "opt_kernel::isolation::subprocess_reload_probe",
            "--exact",
            "--nocapture",
        ])
        .output()
        .expect("spawn clean test process");
    assert!(
        output.status.success(),
        "restart probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains(&format!("restart-observation=({rows:?}, {state:?})")),
        "restarted evaluation differs: {}",
        String::from_utf8_lossy(&output.stdout),
    );
}

#[test]
fn frozen_candidate_results_ignore_unrelated_candidates_order_and_process_restart() {
    let candidate =
        Candidate::parse(super::SGD_ENVELOPE.as_bytes()).expect("fixed candidate admits");
    let frozen = FrozenCandidate::freeze(&candidate);
    let evaluation = FrozenEvaluation::freeze(evaluation_rows());
    let first = evaluate_frozen(&frozen, &evaluation).expect("first frozen evaluation");
    let repeated_input = evaluation_rows().remove(0);
    let before_unrelated = candidate
        .evaluate(&transition_input(&repeated_input))
        .expect("explicit transition");
    assert!(before_unrelated.state.is_empty());

    let unrelated = super::SGD_ENVELOPE.replace(
        "\"parameters\":2,\"state\":\"State0\"",
        "\"parameters\":0,\"state\":\"State0\"",
    );
    let unrelated = Candidate::parse(unrelated.as_bytes()).expect("unrelated candidate admits");
    let _ = evaluate_frozen(&FrozenCandidate::freeze(&unrelated), &evaluation)
        .expect("unrelated candidate evaluation");
    let repeated = evaluate_frozen(&frozen, &evaluation).expect("repeated frozen evaluation");
    assert_eq!(first, repeated);
    assert_ne!(first.rows[0].candidate, first.rows[1].candidate);
    let after_unrelated = candidate
        .evaluate(&transition_input(&repeated_input))
        .expect("explicit transition after unrelated candidate");
    assert_eq!(before_unrelated.parameters, after_unrelated.parameters);
    assert_eq!(before_unrelated.state, after_unrelated.state);

    let mut reversed_rows = evaluation_rows();
    reversed_rows.reverse();
    let reversed = evaluate_frozen(&frozen, &FrozenEvaluation::freeze(reversed_rows))
        .expect("reordered frozen evaluation");
    assert_eq!(
        reversed.rows,
        first.rows.iter().rev().cloned().collect::<Vec<_>>()
    );

    assert_restarted(&first.rows, &before_unrelated.state);
}

#[test]
fn subprocess_reload_probe() {
    let candidate = Candidate::parse(super::SGD_ENVELOPE.as_bytes()).expect("candidate reloaded");
    let frozen = FrozenCandidate::from_parts(
        std::sync::Arc::from(super::SGD_ENVELOPE.as_bytes()),
        candidate.id(),
    )
    .expect("exact candidate bytes verify after process start");
    let result = evaluate_frozen(&frozen, &FrozenEvaluation::freeze(evaluation_rows()))
        .expect("restarted frozen evaluation");
    assert_eq!(result.rows.len(), 2, "both frozen rows must be evaluated");
    assert!(
        result.rows.iter().all(|row| row.agrees),
        "every restarted row must agree with the direct reference"
    );
    let input = evaluation_rows().remove(0);
    let transition = candidate
        .evaluate(&transition_input(&input))
        .expect("restarted transition");
    assert!(
        transition.state.is_empty(),
        "State0 remains empty after reload"
    );
    println!(
        "restart-observation=({:?}, {:?})",
        result.rows, transition.state
    );
}
