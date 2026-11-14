use super::*;

#[derive(Clone, Copy)]
pub(crate) enum EvaluationMode {
    PrimitiveOnly,
    FixedChunkShortlex,
    LearnedRanking,
    ShuffledControl,
}

impl EvaluationMode {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::PrimitiveOnly => "primitive-only",
            Self::FixedChunkShortlex => "fixed-chunk-shortlex",
            Self::LearnedRanking => "learned-ranking",
            Self::ShuffledControl => "splitmix64-shuffled-control",
        }
    }
}

pub(crate) fn ranking_disposition_name(disposition: chunks::RankingDisposition) -> &'static str {
    match disposition {
        chunks::RankingDisposition::NotEvaluated => "not-evaluated",
        chunks::RankingDisposition::Applied => "applied",
        chunks::RankingDisposition::AppliedWithInvalidScores => "applied-with-invalid-scores",
        chunks::RankingDisposition::FallbackStaleProvenance => "fallback-stale-provenance",
        chunks::RankingDisposition::FallbackScorerFailure => "fallback-scorer-failure",
        chunks::RankingDisposition::FallbackInvalidResponse => "fallback-invalid-response",
        chunks::RankingDisposition::FallbackAllInvalidScores => "fallback-all-invalid-scores",
    }
}

pub(crate) fn response_outcome(response: &chunks::Response) -> &'static str {
    match response {
        chunks::Response::Accepted { .. } => "accepted",
        chunks::Response::Invalid { .. } => "invalid",
        chunks::Response::Unsupported { .. } => "unsupported",
        chunks::Response::Exhausted { .. } => "exhausted",
        chunks::Response::NoResult { .. } => "no-result",
        chunks::Response::InternalFailure { .. } => "internal-failure",
    }
}

pub(crate) fn fallback_disposition(disposition: chunks::RankingDisposition) -> bool {
    matches!(
        disposition,
        chunks::RankingDisposition::FallbackStaleProvenance
            | chunks::RankingDisposition::FallbackScorerFailure
            | chunks::RankingDisposition::FallbackInvalidResponse
            | chunks::RankingDisposition::FallbackAllInvalidScores
    )
}

pub(crate) fn candidate_outcome_json(outcome: &chunks::CandidateOutcome) -> Value {
    json!({
        "candidate_id": {"ordinal": outcome.candidate.ordinal, "digest": hex(&outcome.candidate.digest)},
        "ordinal": outcome.ordinal,
        "sequence": outcome.sequence,
        "accepted": outcome.accepted,
        "refusal_class": outcome.refusal_class.map(chunks::Class::name),
        "fixture_agreement": outcome.fixture_agreement,
        "request_identity": hex(&outcome.request_identity),
        "cost": {
            "feature_extraction_ns": outcome.cost.feature_extraction_ns,
            "scorer_elapsed_ns": outcome.cost.scorer_elapsed_ns,
            "re_admission_ns": outcome.cost.re_admission_ns,
            "resource_deltas": outcome.cost.resource_deltas
        }
    })
}

pub(crate) fn scored_candidate_json(candidate: &ScoredCandidate) -> Value {
    json!({
        "candidate_id": {"ordinal": candidate.candidate.ordinal, "digest": hex(&candidate.candidate.digest)},
        "features_f32_bits": candidate.features.iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
        "score_f32_bits": candidate.score.to_bits()
    })
}

pub(crate) fn ranking_identity_json(identity: &chunks::RankingIdentity) -> Value {
    let budgets: Vec<Value> = chunks::Resource::ALL
        .iter()
        .map(|resource| {
            json!({
                "resource": format!("{resource:?}"),
                "limit": identity.budget.limit(*resource)
            })
        })
        .collect();
    json!({
        "request": hex(&identity.request),
        "task_interface": hex(&identity.task_interface),
        "independent_goal": hex(&identity.independent_goal),
        "candidate_set": hex(&identity.candidate_set),
        "library_revision": hex(&identity.library_revision),
        "feature_revision": identity.feature_revision,
        "semantic_revision": identity.semantic_revision,
        "policy": hex(&identity.policy),
        "editor_request": hex(&identity.editor_request),
        "model_identity": identity.model_identity.map(|model| hex(&model)),
        "budget": budgets
    })
}

pub(crate) fn independent_replay_agrees(
    response: &chunks::Response,
    library: &Library,
    spec: &TaskSpec,
) -> Result<bool, String> {
    let chunks::Response::Accepted { accepted, .. } = response else {
        return Ok(false);
    };
    if accepted.selected.len() != 1 {
        return Ok(false);
    }
    let selected = accepted.selected[0];
    let envelope = library
        .entries()
        .iter()
        .map(|entry| entry.envelope())
        .find(|envelope| envelope.chunk_id == selected)
        .ok_or_else(|| {
            format!(
                "accepted chunk identity is absent from library for {}",
                spec.id()
            )
        })?;
    for case in &spec.cases {
        let actual = independent_recipe_output(envelope, &case.inputs)?;
        if actual != case.outputs {
            return Ok(false);
        }
    }
    Ok(true)
}
