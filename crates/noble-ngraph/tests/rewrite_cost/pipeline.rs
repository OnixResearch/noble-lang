//! Paired, fail-closed in-memory rewrite pipeline measurements.

#[path = "pipeline/attempt.rs"]
mod attempt;
#[path = "pipeline/json.rs"]
mod json;
#[path = "pipeline/source.rs"]
mod source;
#[path = "pipeline/stats.rs"]
mod stats;

use noble_ngraph::eval::Obs;
use noble_ngraph::ir::Graph;

use self::attempt::{AcceptedArtifact, FreshRun, PathDetails, RefusalInfo, Stage};
use self::attempt::{EXECUTED, EXECUTION_REFUSED, INPUT_REFUSED};
use self::source::Reference;
use self::stats::{Samples, Summary};

const PRIMING_PAIRS: usize = 64;

pub(super) struct Measurement {
    pub(super) json: String,
    pub(super) disposition: &'static str,
    pub(super) candidate_execution_calls: usize,
    pub(super) baseline_execution_calls: usize,
}

struct PathAggregate {
    stage_samples: [Samples; attempt::STAGES.len()],
    fresh_total: Samples,
    warm_execution: Samples,
    disposition: &'static str,
    refusal: Option<RefusalInfo>,
    details: PathDetails,
    artifact: Option<AcceptedArtifact>,
    priming_execution_calls: usize,
    fresh_execution_calls: usize,
    warm_execution_calls: usize,
    warm_refusal: Option<RefusalInfo>,
}

impl PathAggregate {
    fn new(samples: usize, source_id: &str, graph_retained_bytes: u64) -> Self {
        Self {
            stage_samples: std::array::from_fn(|_| Samples::with_capacity(samples)),
            fresh_total: Samples::with_capacity(samples),
            warm_execution: Samples::with_capacity(samples),
            disposition: INPUT_REFUSED,
            refusal: None,
            details: PathDetails {
                graph_logical_id: Some(source_id.to_owned()),
                graph_retained_bytes: Some(graph_retained_bytes),
                ..PathDetails::default()
            },
            artifact: None,
            priming_execution_calls: 0,
            fresh_execution_calls: 0,
            warm_execution_calls: 0,
            warm_refusal: None,
        }
    }

    fn record_priming(&mut self, run: FreshRun) {
        if run.execution_called {
            self.priming_execution_calls = self.priming_execution_calls.saturating_add(1);
        }
        drop(run);
    }

    fn record(&mut self, run: FreshRun) {
        self.fresh_total.push_us(run.total_us);
        self.stage_samples[Stage::FullFreshAttempt.index()].push_us(run.total_us);
        for (stage, duration_us) in attempt::STAGES.into_iter().zip(run.stage_durations) {
            if let Some(duration_us) = duration_us {
                self.stage_samples[stage.index()].push_us(duration_us);
            }
        }
        if run.execution_called {
            self.fresh_execution_calls = self.fresh_execution_calls.saturating_add(1);
        }
        self.disposition = run.disposition;
        if self.refusal.is_none() {
            self.refusal = run.refusal;
        }
        self.details = run.details;
        if let Some(artifact) = run.artifact {
            self.artifact = Some(artifact);
        }
    }

    fn run_warm(&mut self, expected: Option<&Obs>, samples: usize, scope: &'static str) {
        let Some(artifact) = self.artifact.as_mut() else {
            return;
        };
        let warm = attempt::warm::run(artifact, expected, samples, scope);
        for duration_us in &warm.duration_us {
            self.warm_execution.push_us(*duration_us);
        }
        self.warm_execution_calls = self.warm_execution_calls.saturating_add(warm.calls);
        self.details = warm.details;
        if !warm.executed {
            if self.refusal.is_none() {
                self.refusal = warm.refusal.clone();
            }
            self.warm_refusal = warm.refusal;
            self.disposition = EXECUTION_REFUSED;
        }
    }

    fn execution_calls(&self) -> usize {
        self.priming_execution_calls
            .saturating_add(self.fresh_execution_calls)
            .saturating_add(self.warm_execution_calls)
    }

    fn stage_summary(&mut self, stage: Stage) -> Option<Summary> {
        self.stage_samples[stage.index()].summary()
    }

    fn fresh_summary(&mut self) -> Option<Summary> {
        self.fresh_total.summary()
    }

    fn warm_summary(&mut self) -> Option<Summary> {
        self.warm_execution.summary()
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; this renderer formats runtime refusal details into owned strings."
    )]
    fn stage_reason(&self, stage: Stage) -> String {
        if let Some(refusal) = &self.refusal {
            if refusal.stage == stage.name() {
                return std::format!("{}: {}", refusal.code, refusal.detail);
            }
            return std::format!(
                "not reached because the attempt refused at {}",
                refusal.stage
            );
        }
        if let Some(refusal) = &self.warm_refusal {
            if stage == Stage::WarmExecution {
                return std::format!("{}: {}", refusal.code, refusal.detail);
            }
        }
        "not reached by any measured attempt".to_owned()
    }
}

fn paired_attempt(
    graph: &Graph,
    source: &Reference,
    proposer: noble_ngraph::cert::Proposer,
    ordinal: usize,
    should_retain: bool,
) -> (FreshRun, FreshRun) {
    if ordinal.is_multiple_of(2) {
        let baseline = attempt::fresh::run(graph, source, None, should_retain);
        let certified = attempt::fresh::run(graph, source, Some(proposer), should_retain);
        (baseline, certified)
    } else {
        let certified = attempt::fresh::run(graph, source, Some(proposer), should_retain);
        let baseline = attempt::fresh::run(graph, source, None, should_retain);
        (baseline, certified)
    }
}

pub(super) fn measure(
    graph: &Graph,
    proposer: noble_ngraph::cert::Proposer,
    samples: usize,
) -> Measurement {
    assert!(samples > 0, "pipeline samples must be positive");
    let source = source::observe(graph);
    let expected = source.observation.as_ref();
    let mut baseline = PathAggregate::new(
        samples,
        &source.graph_logical_id,
        source.graph_retained_bytes,
    );
    let mut certified = PathAggregate::new(
        samples,
        &source.graph_logical_id,
        source.graph_retained_bytes,
    );
    for pair in 0..PRIMING_PAIRS {
        let (base, candidate) = paired_attempt(graph, &source, proposer, pair, false);
        baseline.record_priming(base);
        certified.record_priming(candidate);
    }
    for sample in 0..samples {
        let (base, candidate) = paired_attempt(graph, &source, proposer, sample, true);
        baseline.record(base);
        certified.record(candidate);
    }
    baseline.run_warm(expected, samples, "baseline_warm_execution");
    certified.run_warm(expected, samples, "certified_warm_execution");
    let disposition = if certified.disposition == EXECUTED {
        baseline.disposition
    } else {
        certified.disposition
    };
    let candidate_execution_calls = certified.execution_calls();
    let baseline_execution_calls = baseline.execution_calls();
    let json = json::render(&source, &mut baseline, &mut certified, samples);
    Measurement {
        json,
        disposition,
        candidate_execution_calls,
        baseline_execution_calls,
    }
}
