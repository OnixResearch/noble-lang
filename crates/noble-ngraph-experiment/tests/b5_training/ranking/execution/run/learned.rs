use super::*;

pub(super) fn score_first(run: &EvaluationRun<'_>) -> Result<ModeOutcome, String> {
    let started = std::time::Instant::now();
    let (graph, snapshot) = reload_model(run.graph_text, run.fit.snapshot.bytes())?;
    let load_ns = elapsed_ns(started);
    if snapshot.id() != run.fit.snapshot.id() || graph.id() != run.fit.graph.id() {
        return Err(String::from(
            "reloaded model identity differs from the frozen trainer output",
        ));
    }
    let mut ranker = ModelRanker {
        model_identity: *run.model_id,
        graph: &graph,
        snapshot: &snapshot,
        contexts: &run.fit.training_rows.contexts,
        active_identity: std::cell::RefCell::new(None),
        score_calls: 0,
        scorer_elapsed_ns: 0,
        evaluated_nodes: 0,
        scratch: noble_ngraph::eval::ScratchCounters::default(),
        scored: Vec::new(),
    };
    let mut attempts = Vec::new();
    let mut observer = |outcome: &chunks::CandidateOutcome| attempts.push(outcome.clone());
    let mut ranker_session = chunks::RankerSession::new(&mut ranker, *run.model_id);
    let ranked = chunks::complete_ranked_observed(
        run.session,
        run.metadata.library,
        &request(run.metadata.spec, run.metadata.library),
        &mut ranker_session,
        &mut observer,
        monotonic_now_ns,
    );
    debug_assert_eq!(snapshot.id(), run.fit.snapshot.id());
    debug_assert_eq!(graph.id(), run.fit.graph.id());
    let scorer = ScorerStats {
        scored: std::mem::take(&mut ranker.scored),
        score_calls: ranker.score_calls,
        scorer_elapsed_ns: ranker.scorer_elapsed_ns,
        score_evaluated_nodes: ranker.evaluated_nodes,
        score_scratch_peak_bytes: ranker.scratch.peak_bytes,
        model_load_ns: load_ns,
    };
    Ok(ModeOutcome::ranked(ranked, attempts, scorer))
}
