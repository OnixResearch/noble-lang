//! Fresh baseline/certified execution through the same admission and backend stages.

use noble_ngraph::cert::{self, Proposer};
use noble_ngraph::check::{self, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{Control, Obs, ScratchCounters};
use noble_ngraph::ir::Graph;
use noble_ngraph::layout;

use super::{AcceptedArtifact, FreshRun, ScratchObservation, Stage};
use crate::common::IDS;
use crate::pipeline::Reference;

struct Completed {
    checked: CheckedGraph,
    compiled: Compiled,
    observation: Obs,
    scratch: ScratchObservation,
}

#[expect(
    tigerstyle::ambient_clock,
    reason = "Owner: noble-maintainers; this test-only helper times each real library stage."
)]
fn timed<T>(run: &mut FreshRun, stage: Stage, action: impl FnOnce() -> T) -> T {
    let started = std::time::Instant::now();
    let result = std::hint::black_box(action());
    run.stage_durations[stage.index()] = Some(super::elapsed_us(started));
    result
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; graph admission invokes a runtime checker inside the monotonic-clock timing helper."
)]
fn admission(
    graph: Graph,
    stage: Stage,
    disposition: &'static str,
    run: &mut FreshRun,
) -> Option<CheckedGraph> {
    match timed(run, stage, || check::check(&IDS, graph)) {
        Ok(checked) => Some(checked),
        Err(diagnostic) => {
            run.refuse(disposition, super::diagnostic_reason(stage, &diagnostic));
            None
        }
    }
}

fn admit(graph: Graph, proposer: Option<Proposer>, run: &mut FreshRun) -> Option<CheckedGraph> {
    let checked = admission(graph, Stage::InputAdmission, super::INPUT_REFUSED, run)?;
    let Some(proposer) = proposer else {
        return Some(checked);
    };
    let (derivation, candidate) = timed(run, Stage::ProposalSearch, || {
        cert::propose(&IDS, proposer, checked.graph())
    });
    run.details.certificate = Some(super::certificate_info(&derivation));
    let verdict = timed(run, Stage::CertificateChecking, || {
        cert::check(&IDS, checked.graph(), &candidate, &derivation)
    });
    if let Err(refusal) = verdict {
        run.refuse(
            super::CERTIFICATE_REFUSED,
            super::certificate_reason(&refusal),
        );
        return None;
    }
    admission(
        candidate,
        Stage::CandidateAdmission,
        super::CANDIDATE_REFUSED,
        run,
    )
}

fn execute(checked: CheckedGraph, run: &mut FreshRun) -> Option<Completed> {
    let compiled = timed(run, Stage::BackendPreparation, || {
        Compiled::new(&IDS, &checked)
    });
    let verdict = timed(run, Stage::ScheduleValidation, || {
        layout::check(&checked, &compiled.packed_layout())
    });
    if let Err(refusal) = verdict {
        run.refuse(super::LAYOUT_REFUSED, super::layout_reason(refusal));
        return None;
    }
    let mut counters = ScratchCounters::default();
    run.execution_called = true;
    let report = timed(run, Stage::FirstExecution, || {
        let mut control = Control {
            scratch: Some(&mut counters),
            ..Control::default()
        };
        compiled.evaluate(&[], None, &mut control)
    });
    let scratch = ScratchObservation::from("fresh_first_execution", &counters);
    if report.obs.outputs().is_none() {
        run.details.fresh_scratch = Some(scratch);
        run.refuse(
            super::EXECUTION_REFUSED,
            super::observation_reason(&report.obs),
        );
        return None;
    }
    Some(Completed {
        checked,
        compiled,
        observation: report.obs,
        scratch,
    })
}

#[expect(
    tigerstyle::ambient_clock,
    reason = "Owner: noble-maintainers; this test-only lane directly times real fresh attempts, including refused-path cleanup."
)]
pub(in crate::pipeline) fn run(
    graph: &Graph,
    source: &Reference,
    proposer: Option<Proposer>,
    should_retain: bool,
) -> FreshRun {
    let owned = std::hint::black_box(graph.clone());
    let mut run = FreshRun {
        total_us: 0.0,
        stage_durations: [None; super::STAGES.len()],
        disposition: super::INPUT_REFUSED,
        refusal: None,
        execution_called: false,
        details: super::source_details(&source.graph_logical_id, source.graph_retained_bytes),
        artifact: None,
    };
    let started = std::time::Instant::now();
    let completed = admit(owned, proposer, &mut run).and_then(|checked| execute(checked, &mut run));
    run.total_us = super::elapsed_us(started);
    if let Some(completed) = completed {
        assert!(
            super::agreement(&completed.observation, source.observation.as_ref()),
            "compiled/source disagreement"
        );
        run.disposition = super::EXECUTED;
        run.details = super::accepted_details(
            run.details,
            &completed.checked,
            &completed.compiled,
            &completed.observation,
            completed.scratch,
        );
        if should_retain {
            run.artifact = Some(AcceptedArtifact {
                workspace: completed.compiled.workspace(),
                compiled: completed.compiled,
                details: run.details.clone(),
            });
        }
    }
    run
}
