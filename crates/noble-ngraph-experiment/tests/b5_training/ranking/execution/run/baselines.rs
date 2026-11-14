use super::*;

pub(super) fn run_primitive(run: &EvaluationRun<'_>, started: std::time::Instant) -> Result<Value, String> {
    let EvaluationMeta { mode, repetition, seed, spec, .. } = run.metadata;
    let request = request(spec, run.metadata.library);
    let prepared = run.session.prepare(&spec.primitive_source, &spec.stack_in,
        source_limits(request.budget))
        .map_err(|error| format!("primitive preparation {} refused: {error:?}", spec.id()))?;
    if prepared.output() != spec.stack_out {
        return Err(format!("primitive output interface mismatch for {}", spec.id()));
    }
    let primitive = primitive_envelope(spec);
    let mut is_output_match = true;
    for case in &spec.cases {
        is_output_match &= independent_recipe_output(&primitive, &case.inputs)? == case.outputs;
    }
    if !is_output_match {
        return Err(format!("primitive fixture mismatch for {}", spec.id()));
    }
    debug_assert_eq!(prepared.output(), spec.stack_out);
    debug_assert!(source_limits(request.budget).bytes <= Budget::PROFILE.source_bytes);
    Ok(json!({
        "task_id": spec.id(), "family": FAMILY_NAMES[spec.family], "split": "test",
        "repetition": repetition, "seed": format!("0x{seed:016x}"), "mode": mode.name(),
        "response_outcome": "primitive-prepared", "structural_acceptance": true,
        "finite_fixture_success": true, "fixture_agreement": true,
        "independent_replay": true, "candidate_rows": 0,
        "latency_ns": elapsed_ns(started),
        "report": {"source_bytes": spec.primitive_source.len(), "retained_bytes": 0,
            "maximum_expanded_program_nodes": null},
        "work": {
            "total_charged_work": null,
            "total_shared_work_definition": "not exposed by Session::prepare",
            "feature_extraction": 0, "score_evaluation": 0,
            "queue_management": 0, "candidate_proposals": 0,
            "candidate_checks": 0, "expanded_nodes": 0,
            "maximum_expanded_program_nodes": null, "dependency_work": 0,
            "instantiation_work": 0, "source_preparation_work": null,
            "kernel_check_work": null, "fixture_steps": null,
            "unmeasured": ["primitive preparation work counter is private to noble-contracts"]
        }
    }))
}

pub(super) fn run_fixed(run: &EvaluationRun<'_>) -> ModeOutcome {
    let mut attempts = Vec::new();
    let mut observer = |outcome: &chunks::CandidateOutcome| attempts.push(outcome.clone());
    let request = request(run.metadata.spec, run.metadata.library);
    let response = chunks::complete_observed(run.session, run.metadata.library,
        &request, &mut observer, monotonic_now_ns);
    let candidate_row_count = chunks::candidate_queue(run.metadata.library,
        chunks::CandidateLimit::from(request.budget.candidates)).candidates().len();
    debug_assert!(attempts.len() <= candidate_row_count);
    debug_assert!(candidate_row_count <= usize::try_from(chunks::MAX_CANDIDATES).expect("fixed profile"));
    ModeOutcome::fixed(response, attempts, candidate_row_count)
}
