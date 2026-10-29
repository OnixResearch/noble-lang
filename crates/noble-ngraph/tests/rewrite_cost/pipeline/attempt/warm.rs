//! Reused-workspace execution samples.

use noble_ngraph::eval::{Control, Obs, ScratchCounters};

use super::{
    elapsed_us, observation_reason, outputs, AcceptedArtifact, ScratchObservation, WarmRun,
};

const PRIMING_WARM_CALLS: usize = 64;

fn evaluate_once(artifact: &mut AcceptedArtifact, counters: &mut ScratchCounters) -> Obs {
    let mut control = Control {
        scratch: Some(counters),
        ..Control::default()
    };
    let report = std::hint::black_box(artifact.compiled.evaluate_in(
        &mut artifact.workspace,
        &[],
        None,
        &mut control,
    ));
    report.obs
}

#[expect(
    tigerstyle::ambient_clock,
    reason = "Owner: noble-maintainers; this test-only lane directly times real warm execution calls."
)]
pub(in crate::pipeline) fn run(
    artifact: &mut AcceptedArtifact,
    expected: Option<&Obs>,
    samples: usize,
    scope: &'static str,
) -> WarmRun {
    let mut duration_us = Vec::with_capacity(samples);
    let mut counters = ScratchCounters::default();
    let mut details = artifact.details.clone();
    let mut calls = 0usize;

    // Prime the retained workspace without recording those durations. These are real calls and
    // remain visible in the scratch and evaluation-call observations.
    for _ in 0..PRIMING_WARM_CALLS {
        let observed = evaluate_once(artifact, &mut counters);
        calls = calls.saturating_add(1);
        if !super::agreement(&observed, expected) {
            let mut reason = observation_reason(&observed);
            reason.stage = super::Stage::WarmExecution.name();
            details.warm_scratch = Some(ScratchObservation::from(scope, &counters));
            details.warm_output = outputs(&observed);
            return WarmRun {
                duration_us,
                executed: false,
                refusal: Some(reason),
                calls,
                details,
            };
        }
    }

    for _ in 0..samples {
        let started = std::time::Instant::now();
        let observed = evaluate_once(artifact, &mut counters);
        calls = calls.saturating_add(1);
        let elapsed = elapsed_us(started);
        duration_us.push(elapsed);

        // Stop the clock before semantic agreement. A refused or non-agreeing warm call cannot be
        // a successful execution, even though the call itself remains in the observed sample.
        if !super::agreement(&observed, expected) {
            let mut reason = observation_reason(&observed);
            reason.stage = super::Stage::WarmExecution.name();
            details.warm_scratch = Some(ScratchObservation::from(scope, &counters));
            details.warm_output = outputs(&observed);
            return WarmRun {
                duration_us,
                executed: false,
                refusal: Some(reason),
                calls,
                details,
            };
        }
        details.warm_output = outputs(&observed);
    }
    details.warm_scratch = Some(ScratchObservation::from(scope, &counters));
    artifact.details = details.clone();
    WarmRun {
        duration_us,
        executed: true,
        refusal: None,
        calls,
        details,
    }
}

#[test]
fn an_output_mismatch_stops_before_measured_samples() {
    let graph = crate::corpus_graph("single-fold");
    let checked = noble_ngraph::check(&crate::common::IDS, graph).expect("admitted source");
    let compiled = noble_ngraph::compiled::Compiled::new(&crate::common::IDS, &checked);
    let mut artifact = AcceptedArtifact {
        workspace: compiled.workspace(),
        compiled,
        details: super::PathDetails::default(),
    };
    let mismatched = Obs::Normal(vec![noble_ngraph::Value::I64(999)]);
    let result = run(&mut artifact, Some(&mismatched), 2, "mismatched_warm");
    assert!(!result.executed);
    assert_eq!(result.calls, 1);
    assert!(result.duration_us.is_empty());
    assert_eq!(
        result.refusal.expect("warm refusal").stage,
        "warm_execution"
    );
}
