use super::*;

pub(crate) fn run_completion_mode(run: EvaluationRun<'_>) -> Result<Value, String> {
    let EvaluationRun {
        metadata,
        session,
        fit,
        model_id,
        graph_text,
        model_payload_bytes,
    } = run;
    let EvaluationMeta {
        mode,
        repetition,
        seed,
        spec,
        library,
    } = metadata;
    let mut attempts = Vec::new();
    let mut observer = |outcome: &chunks::CandidateOutcome| attempts.push(outcome.clone());
    let mut model_load_ns = 0u64;
    let mut score_calls = 0u32;
    let mut scorer_elapsed_ns = 0u64;
    let mut score_evaluated_nodes = 0u64;
    let mut score_scratch_peak_bytes = 0u64;
    let mut invalid_scores = 0u32;
    let mut disposition = None;
    let mut ranking_identity = None;
    let ranked_candidates: Vec<chunks::CandidateId>;
    let candidate_row_count: usize;
    let mut scored = Vec::new();
    let started = std::time::Instant::now();
    let request = request(spec, library);
    let response = match mode {
        EvaluationMode::PrimitiveOnly => {
            let prepared = session
                .prepare(
                    &spec.primitive_source,
                    &spec.stack_in,
                    source_limits(request.budget),
                )
                .map_err(|error| {
                    format!("primitive preparation {} refused: {error:?}", spec.id())
                })?;
            if prepared.output() != spec.stack_out {
                return Err(format!(
                    "primitive output interface mismatch for {}",
                    spec.id()
                ));
            }
            let mut outputs_match = true;
            let primitive = primitive_envelope(spec);
            for case in &spec.cases {
                outputs_match &=
                    independent_recipe_output(&primitive, &case.inputs)? == case.outputs;
            }
            if !outputs_match {
                return Err(format!("primitive fixture mismatch for {}", spec.id()));
            }
            let primitive_outcome = json!({
                "task_id": spec.id(),
                "family": FAMILY_NAMES[spec.family],
                "split": "test",
                "repetition": repetition,
                "seed": format!("0x{seed:016x}"),
                "mode": mode.name(),
                "response_outcome": "primitive-prepared",
                "structural_acceptance": true,
                "finite_fixture_success": true,
                "fixture_agreement": true,
                "independent_replay": true,
                "candidate_rows": 0,
                "latency_ns": elapsed_ns(started),
                "report": {
                    "source_bytes": spec.primitive_source.len(),
                    "retained_bytes": 0,
                    "maximum_expanded_program_nodes": null
                },
                "work": {
                    "total_charged_work": null,
                    "total_shared_work_definition": "not exposed by Session::prepare",
                    "feature_extraction": 0,
                    "score_evaluation": 0,
                    "queue_management": 0,
                    "candidate_proposals": 0,
                    "candidate_checks": 0,
                    "expanded_nodes": 0,
                    "maximum_expanded_program_nodes": null,
                    "dependency_work": 0,
                    "instantiation_work": 0,
                    "source_preparation_work": null,
                    "kernel_check_work": null,
                    "fixture_steps": null,
                    "unmeasured": ["primitive preparation work counter is private to noble-contracts"]
                }
            });
            return Ok(primitive_outcome);
        }
        EvaluationMode::FixedChunkShortlex => {
            let response = chunks::complete_observed(
                session,
                library,
                &request,
                &mut observer,
                monotonic_now_ns,
            );
            candidate_row_count = chunks::candidate_queue(
                library,
                chunks::CandidateLimit::from(request.budget.candidates),
            )
            .candidates()
            .len();
            ranked_candidates = attempts.iter().map(|outcome| outcome.candidate).collect();
            response
        }
        EvaluationMode::LearnedRanking => {
            let load_started = std::time::Instant::now();
            let (graph, snapshot) = reload_model(graph_text, fit.snapshot.bytes())?;
            model_load_ns = elapsed_ns(load_started);
            if snapshot.id() != fit.snapshot.id() || graph.id() != fit.graph.id() {
                return Err(String::from(
                    "reloaded model identity differs from the frozen trainer output",
                ));
            }
            let mut ranker = ModelRanker {
                model_identity: *model_id,
                graph: &graph,
                snapshot: &snapshot,
                contexts: &fit.training_rows.contexts,
                active_identity: std::cell::RefCell::new(None),
                score_calls: 0,
                scorer_elapsed_ns: 0,
                evaluated_nodes: 0,
                scratch: noble_ngraph::eval::ScratchCounters::default(),
                scored: Vec::new(),
            };
            let mut ranker_session = chunks::RankerSession::new(&mut ranker, *model_id);
            let ranked = chunks::complete_ranked_observed(
                session,
                library,
                &request,
                &mut ranker_session,
                &mut observer,
                monotonic_now_ns,
            );
            disposition = Some(ranked.disposition);
            ranking_identity = ranked.identity;
            invalid_scores = ranked.invalid_scores;
            ranked_candidates = ranked.ranked_candidates;
            candidate_row_count = ranked_candidates.len();
            score_calls = ranker.score_calls;
            scorer_elapsed_ns = ranker.scorer_elapsed_ns;
            score_evaluated_nodes = ranker.evaluated_nodes;
            score_scratch_peak_bytes = ranker.scratch.peak_bytes;
            scored = std::mem::take(&mut ranker.scored);
            ranked.response
        }
        EvaluationMode::ShuffledControl => {
            let context = fit
                .training_rows
                .contexts
                .iter()
                .find(|context| {
                    context.family == FAMILY_NAMES[spec.family]
                        && context.library_revision == *library.revision()
                        && context.library_identity == library_identity(library)
                })
                .ok_or_else(|| format!("shuffle context absent for {}", spec.id()))?;
            let shuffle_id = shuffled_model_identity(seed, model_id);
            let mut ranker = ShuffledRanker {
                model_identity: shuffle_id,
                contexts: fit.training_rows.contexts.clone(),
                active_identity: std::cell::RefCell::new(None),
                scores: shuffled_scores(context, seed),
                score_calls: 0,
                scored: Vec::new(),
            };
            let mut ranker_session = chunks::RankerSession::new(&mut ranker, shuffle_id);
            let ranked = chunks::complete_ranked_observed(
                session,
                library,
                &request,
                &mut ranker_session,
                &mut observer,
                monotonic_now_ns,
            );
            disposition = Some(ranked.disposition);
            ranking_identity = ranked.identity;
            invalid_scores = ranked.invalid_scores;
            ranked_candidates = ranked.ranked_candidates;
            candidate_row_count = ranked_candidates.len();
            score_calls = ranker.score_calls;
            scored = std::mem::take(&mut ranker.scored);
            ranked.response
        }
    };
    evaluation_row(EvaluationRecord {
        metadata,
        response,
        attempts,
        disposition,
        invalid_scores,
        ranked_candidates,
        candidate_row_count,
        ranking_identity,
        scored,
        score_calls,
        scorer_elapsed_ns,
        score_evaluated_nodes,
        score_scratch_peak_bytes,
        model_load_ns,
        model_payload_bytes: if matches!(mode, EvaluationMode::LearnedRanking) {
            model_payload_bytes
        } else {
            0
        },
        started,
    })
}
