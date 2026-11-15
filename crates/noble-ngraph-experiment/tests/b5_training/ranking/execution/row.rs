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

fn expanded_node_max(attempts: &[chunks::CandidateOutcome]) -> Result<Option<u32>, String> {
    let slot = chunks::Resource::ALL
        .iter()
        .position(|resource| *resource == chunks::Resource::ExpandedNodes)
        .ok_or_else(|| String::from("resource roster omitted expanded-node accounting"))?;
    Ok(attempts
        .iter()
        .map(|attempt| attempt.cost.resource_deltas[slot])
        .max())
}

fn work_json(
    report: &chunks::Report,
    score_calls: u32,
    evaluated_nodes: u64,
    ranked_count: usize,
    maximum_nodes: Option<u32>,
) -> Value {
    json!({
        "total_charged_work": u64::from(report.checking_work),
        "total_shared_work_definition": "Report.checking_work; distinct budget dimensions remain separately reported",
        "feature_extraction": score_calls.saturating_mul(chunks::FEATURE_EXTRACTION_WORK),
        "score_evaluation": evaluated_nodes, "queue_management": ranked_count,
        "candidate_proposals": report.candidates, "candidate_checks": report.checked_candidates,
        "expanded_nodes": report.expanded_nodes, "maximum_expanded_program_nodes": maximum_nodes,
        "dependency_work": report.dependency_steps, "instantiation_work": report.instantiation_bindings,
        "source_preparation_work": null, "kernel_check_work": report.checking_work,
        "fixture_steps": report.evaluation_steps,
        "unmeasured": ["host source-preparation internals are not separately exposed by Session::prepare"]
    })
}

fn report_json(report: &chunks::Report, maximum_nodes: Option<u32>) -> Value {
    json!({
        "candidates": report.candidates, "checked_candidates": report.checked_candidates,
        "accepted": report.accepted, "rejected": report.rejected,
        "expanded_nodes": report.expanded_nodes,
        "maximum_expanded_program_nodes": maximum_nodes,
        "maximum_expanded_program_nodes_definition": "maximum per-admission expanded-node resource delta; null when no checker attempt occurred",
        "dependency_steps": report.dependency_steps,
        "instantiation_bindings": report.instantiation_bindings,
        "checking_work": report.checking_work, "evaluation_steps": report.evaluation_steps,
        "composed_chunks": report.composed_chunks, "source_bytes": report.source_bytes,
        "stack_height": report.stack_height, "quotation_depth": report.quotation_depth,
        "retained_bytes": report.retained_bytes, "core_latency_ns": report.total_latency_ns
    })
}

fn memory_json(record: &EvaluationRecord<'_>, report: &chunks::Report) -> Value {
    json!({
        "score_graph_peak_scratch_bytes": record.score_scratch_peak_bytes,
        "actual_peak_retained_bytes": null,
        "retained_bytes_proxy": u64::from(record.metadata.library.retained()).saturating_add(u64::from(report.retained_bytes)).saturating_add(record.model_payload_bytes),
        "retained_bytes_proxy_definition": "library/report budget-retained bytes plus serialized model payload; allocator overhead is not measured",
        "library_retained_bytes": record.metadata.library.retained(), "request_retained_bytes": report.retained_bytes,
        "model_payload_bytes": record.model_payload_bytes
    })
}

pub(crate) fn serialize_evaluation(record: EvaluationRecord<'_>) -> Result<Value, String> {
    let replay_started = std::time::Instant::now();
    let is_independent_replay = independent_replay_agrees(
        &record.response,
        record.metadata.library,
        record.metadata.spec,
    )?;
    let independent_replay_ns = elapsed_ns(replay_started);
    let report = report_of(&record.response);
    let fixture_agreement = report.fixtures.as_ref().map(|fixture| fixture.agreement);
    let maximum_nodes = expanded_node_max(&record.attempts)?;
    let is_fallback = record.disposition.is_some_and(fallback_disposition);
    let fallback_rows = if is_fallback {
        u32::try_from(record.candidate_row_count).expect("bounded candidate queue")
    } else {
        0
    };
    debug_assert!(report.checked_candidates <= report.candidates);
    debug_assert!(report.candidates <= Budget::PROFILE.candidates);
    let work = work_json(
        report,
        record.score_calls,
        record.score_evaluated_nodes,
        record.ranked_candidates.len(),
        maximum_nodes,
    );
    let memory = memory_json(&record, report);
    let ranked_candidates = record
        .ranked_candidates
        .iter()
        .map(|candidate| {
            json!({
                "ordinal": candidate.ordinal, "digest": hex(&candidate.digest)
            })
        })
        .collect::<Vec<_>>();
    let mut row = json!({
        "task_id": record.metadata.spec.id(), "family": FAMILY_NAMES[record.metadata.spec.family],
        "split": record.metadata.spec.split(), "repetition": record.metadata.repetition,
        "seed": format!("0x{:016x}", record.metadata.seed), "mode": record.metadata.mode.name(),
        "response_outcome": response_outcome(&record.response),
        "structural_acceptance": matches!(&record.response, chunks::Response::Accepted { .. }),
        "finite_fixture_success": fixture_agreement.unwrap_or(false),
        "fixture_agreement": fixture_agreement, "independent_replay": is_independent_replay,
        "independent_replay_ns": independent_replay_ns,
        "ranking_disposition": record.disposition.map(disposition_name),
        "ranking_identity": record.ranking_identity.as_ref().map(identity_record),
        "invalid_score_rows": record.invalid_scores, "fallback_candidate_rows": fallback_rows,
        "candidate_rows": record.candidate_row_count, "score_calls": record.score_calls,
        "scorer_elapsed_ns": record.scorer_elapsed_ns,
        "score_evaluated_nodes": record.score_evaluated_nodes,
        "score_scratch_peak_bytes": record.score_scratch_peak_bytes,
        "model_load_ns": record.model_load_ns,
        "model_payload_bytes": record.model_payload_bytes,
        "latency_ns": 0, "work": work,
        "report": report_json(report, maximum_nodes), "memory": memory,
        "ranked_candidates": ranked_candidates,
        "scored_candidates": record.scored.iter().map(score_record).collect::<Vec<_>>(),
        "attempts": record.attempts.iter().map(candidate_record).collect::<Vec<_>>()
    });
    row["latency_ns"] = json!(elapsed_ns(record.started));
    Ok(row)
}
