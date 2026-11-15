use super::*;

pub(super) fn shuffle_first(run: &EvaluationRun<'_>) -> Result<ModeOutcome, String> {
    let spec = run.metadata.spec;
    let library = run.metadata.library;
    let context = run
        .fit
        .training_rows
        .contexts
        .iter()
        .find(|context| {
            context.family == FAMILY_NAMES[spec.family]
                && context.library_revision == *library.revision()
                && context.library_identity == library_identity(library)
        })
        .ok_or_else(|| format!("shuffle context absent for {}", spec.id()))?;
    let shuffle_id = shuffled_model_identity(run.metadata.seed, run.model_id);
    let mut ranker = ShuffledRanker {
        model_identity: shuffle_id,
        contexts: run.fit.training_rows.contexts.clone(),
        active_identity: std::cell::RefCell::new(None),
        scores: shuffled_scores(context, run.metadata.seed),
        score_calls: 0,
        scored: Vec::new(),
    };
    let mut attempts = Vec::new();
    let mut observer = |outcome: &chunks::CandidateOutcome| attempts.push(outcome.clone());
    let mut ranker_session = chunks::RankerSession::new(&mut ranker, shuffle_id);
    let ranked = chunks::complete_ranked_observed(
        run.session,
        library,
        &request(spec, library),
        &mut ranker_session,
        &mut observer,
        monotonic_now_ns,
    );
    debug_assert_eq!(context.library_identity, library_identity(library));
    debug_assert_eq!(context.library_revision, *library.revision());
    let scorer = ScorerStats {
        scored: std::mem::take(&mut ranker.scored),
        score_calls: ranker.score_calls,
        scorer_elapsed_ns: 0,
        score_evaluated_nodes: 0,
        score_scratch_peak_bytes: 0,
        model_load_ns: 0,
    };
    Ok(ModeOutcome::ranked(ranked, attempts, scorer))
}
