use super::*;

#[derive(Clone, Copy)]
pub(crate) struct EvaluationMeta<'a> {
    pub(crate) mode: EvaluationMode,
    pub(crate) repetition: usize,
    pub(crate) seed: u64,
    pub(crate) spec: &'a TaskSpec,
    pub(crate) library: &'a Library,
}

pub(crate) struct EvaluationRun<'a> {
    pub(crate) metadata: EvaluationMeta<'a>,
    pub(crate) session: &'a noble_contracts::source::Session,
    pub(crate) fit: &'a ModelFit,
    pub(crate) model_id: &'a [u8; chunks::REVISION_BYTES],
    pub(crate) graph_text: &'a str,
    pub(crate) model_payload_bytes: u64,
}

pub(crate) struct EvaluationRecord<'a> {
    pub(crate) metadata: EvaluationMeta<'a>,
    pub(crate) response: chunks::Response,
    pub(crate) attempts: Vec<chunks::CandidateOutcome>,
    pub(crate) disposition: Option<chunks::RankingDisposition>,
    pub(crate) invalid_scores: u32,
    pub(crate) ranked_candidates: Vec<chunks::CandidateId>,
    pub(crate) candidate_row_count: usize,
    pub(crate) ranking_identity: Option<chunks::RankingIdentity>,
    pub(crate) scored: Vec<ScoredCandidate>,
    pub(crate) score_calls: u32,
    pub(crate) scorer_elapsed_ns: u64,
    pub(crate) score_evaluated_nodes: u64,
    pub(crate) score_scratch_peak_bytes: u64,
    pub(crate) model_load_ns: u64,
    pub(crate) model_payload_bytes: u64,
    pub(crate) started: std::time::Instant,
}

pub(crate) fn evaluation_row(record: EvaluationRecord<'_>) -> Result<Value, String> {
    let EvaluationRecord {
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
        model_payload_bytes,
        started,
    } = record;
    let EvaluationMeta {
        mode,
        repetition,
        seed,
        spec,
        library,
    } = metadata;
    let replay_started = std::time::Instant::now();
    let independent_replay = independent_replay_agrees(&response, library, spec)?;
    let independent_replay_ns = elapsed_ns(replay_started);
    let report = report_of(&response);
    let fixture_agreement = report.fixtures.as_ref().map(|fixtures| fixtures.agreement);
    let structural_acceptance = matches!(&response, chunks::Response::Accepted { .. });
    let finite_fixture_success = fixture_agreement.unwrap_or(false);
    let fallback = disposition.is_some_and(fallback_disposition);
    let fallback_rows = if fallback {
        u32::try_from(candidate_row_count).unwrap_or(u32::MAX)
    } else {
        0
    };
    let expanded_nodes_slot = chunks::Resource::ALL
        .iter()
        .position(|resource| *resource == chunks::Resource::ExpandedNodes)
        .ok_or_else(|| String::from("resource roster omitted expanded-node accounting"))?;
    let maximum_expanded_program_nodes = attempts
        .iter()
        .map(|outcome| outcome.cost.resource_deltas[expanded_nodes_slot])
        .max();
    let total_charged_work = u64::from(report.checking_work);
    let work = json!({
        "total_charged_work": total_charged_work,
        "total_shared_work_definition": "Report.checking_work; distinct budget dimensions remain separately reported",
        "feature_extraction": score_calls.saturating_mul(chunks::FEATURE_EXTRACTION_WORK),
        "score_evaluation": score_evaluated_nodes,
        "queue_management": ranked_candidates.len(),
        "candidate_proposals": report.candidates,
        "candidate_checks": report.checked_candidates,
        "expanded_nodes": report.expanded_nodes,
        "maximum_expanded_program_nodes": maximum_expanded_program_nodes,
        "dependency_work": report.dependency_steps,
        "instantiation_work": report.instantiation_bindings,
        "source_preparation_work": null,
        "kernel_check_work": report.checking_work,
        "fixture_steps": report.evaluation_steps,
        "unmeasured": ["host source-preparation internals are not separately exposed by Session::prepare"]
    });
    let report_json = json!({
        "candidates": report.candidates,
        "checked_candidates": report.checked_candidates,
        "accepted": report.accepted,
        "rejected": report.rejected,
        "expanded_nodes": report.expanded_nodes,
        "maximum_expanded_program_nodes": maximum_expanded_program_nodes,
        "maximum_expanded_program_nodes_definition": "maximum per-admission expanded-node resource delta; null when no checker attempt occurred",
        "dependency_steps": report.dependency_steps,
        "instantiation_bindings": report.instantiation_bindings,
        "checking_work": report.checking_work,
        "evaluation_steps": report.evaluation_steps,
        "composed_chunks": report.composed_chunks,
        "source_bytes": report.source_bytes,
        "stack_height": report.stack_height,
        "quotation_depth": report.quotation_depth,
        "retained_bytes": report.retained_bytes,
        "core_latency_ns": report.total_latency_ns
    });
    let memory = json!({
        "score_graph_peak_scratch_bytes": score_scratch_peak_bytes,
        "actual_peak_retained_bytes": null,
        "retained_bytes_proxy": u64::from(library.retained()).saturating_add(u64::from(report.retained_bytes)).saturating_add(model_payload_bytes),
        "retained_bytes_proxy_definition": "library/report budget-retained bytes plus serialized model payload; allocator overhead is not measured",
        "library_retained_bytes": library.retained(),
        "request_retained_bytes": report.retained_bytes,
        "model_payload_bytes": model_payload_bytes
    });
    let ranked_candidates_json = ranked_candidates
        .iter()
        .map(|candidate| json!({ "ordinal": candidate.ordinal, "digest": hex(&candidate.digest) }))
        .collect::<Vec<_>>();
    let mut row = json!({
        "task_id": spec.id(),
        "family": FAMILY_NAMES[spec.family],
        "split": spec.split(),
        "repetition": repetition,
        "seed": format!("0x{seed:016x}"),
        "mode": mode.name(),
        "response_outcome": response_outcome(&response),
        "structural_acceptance": structural_acceptance,
        "finite_fixture_success": finite_fixture_success,
        "fixture_agreement": fixture_agreement,
        "independent_replay": independent_replay,
        "independent_replay_ns": independent_replay_ns,
        "ranking_disposition": disposition.map(ranking_disposition_name),
        "ranking_identity": ranking_identity.as_ref().map(ranking_identity_json),
        "invalid_score_rows": invalid_scores,
        "fallback_candidate_rows": fallback_rows,
        "candidate_rows": candidate_row_count,
        "score_calls": score_calls,
        "scorer_elapsed_ns": scorer_elapsed_ns,
        "score_evaluated_nodes": score_evaluated_nodes,
        "score_scratch_peak_bytes": score_scratch_peak_bytes,
        "model_load_ns": model_load_ns,
        "model_payload_bytes": model_payload_bytes,
        "latency_ns": 0,
        "work": work,
        "report": report_json,
        "memory": memory,
        "ranked_candidates": ranked_candidates_json,
        "scored_candidates": scored.iter().map(scored_candidate_json).collect::<Vec<_>>(),
        "attempts": attempts.iter().map(candidate_outcome_json).collect::<Vec<_>>()
    });
    row["latency_ns"] = json!(elapsed_ns(started));
    Ok(row)
}
