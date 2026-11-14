use super::*;

pub(crate) fn source_limits(budget: Budget) -> noble_contracts::Limits {
    noble_contracts::Limits {
        bytes: budget.source_bytes.min(Budget::PROFILE.source_bytes),
        nodes: budget.source_bytes.min(Budget::PROFILE.source_bytes),
        depth: budget.quotation_depth.min(Budget::PROFILE.quotation_depth),
        work: budget.checking_work.min(Budget::PROFILE.checking_work),
    }
}

#[test]
pub(crate) fn primitive_baseline_uses_the_callers_source_budget_caps() {
    let budget = Budget {
        source_bytes: 1_024,
        quotation_depth: 4,
        checking_work: 1_000,
        ..Budget::PROFILE
    };
    let limits = source_limits(budget);
    assert_eq!(limits.bytes, 1_024);
    assert_eq!(limits.nodes, 1_024);
    assert_eq!(limits.depth, 4);
    assert_eq!(limits.work, 1_000);
}

pub(crate) fn fixture_set(spec: &TaskSpec) -> FixtureSet {
    let cases: Vec<Fixture> = spec
        .cases
        .iter()
        .map(|case| Fixture {
            inputs: case.inputs.clone(),
            outputs: case.outputs.clone(),
        })
        .collect();
    let record = json!(cases
        .iter()
        .map(|case| json!({
            "inputs": case.inputs.iter().copied().map(scalar_json).collect::<Vec<_>>(),
            "outputs": case.outputs.iter().copied().map(scalar_json).collect::<Vec<_>>()
        }))
        .collect::<Vec<_>>());
    let encoded = serde_json::to_vec(&record).unwrap_or_default();
    let digest = blake3::hash(&encoded);
    let mut id = [0u8; chunks::IDENTITY_BYTES];
    id.copy_from_slice(&digest.as_bytes()[..chunks::IDENTITY_BYTES]);
    FixtureSet { id, cases }
}

pub(crate) fn request(spec: &TaskSpec, library: &Library) -> Request {
    let mut budget = Budget::PROFILE;
    budget.candidates = 4;
    budget.composed_chunks = 1;
    Request {
        format: 1,
        semantic_revision: 0,
        profile: String::from("pure-i64-bool/1"),
        task: Task {
            stack_in: spec.stack_in.clone(),
            stack_out: spec.stack_out.clone(),
            allowed_effects: EffSet::empty(),
        },
        editor: editor::Candidate {
            format: 1,
            nodes: vec![editor::Node::Hole],
        },
        hole_path: vec![0],
        library_revision: *library.revision(),
        fixtures: Some(fixture_set(spec)),
        budget,
    }
}

pub(crate) fn monotonic_now_ns() -> u64 {
    static START: std::sync::LazyLock<std::time::Instant> =
        std::sync::LazyLock::new(std::time::Instant::now);
    u64::try_from(START.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

pub(crate) struct CapturedFeature {
    pub(crate) candidate: chunks::CandidateId,
    pub(crate) features: chunks::CandidateFeatureVector,
}

pub(crate) struct CapturingRanker {
    pub(crate) model_identity: [u8; chunks::REVISION_BYTES],
    pub(crate) library_revision: [u8; chunks::REVISION_BYTES],
    pub(crate) identity: std::cell::RefCell<Option<chunks::RankingIdentity>>,
    pub(crate) features: Vec<CapturedFeature>,
}

impl chunks::CandidateRanker for CapturingRanker {
    fn model_identity(&self) -> [u8; chunks::REVISION_BYTES] {
        self.model_identity
    }

    fn provenance_matches(&self, identity: &chunks::RankingIdentity) -> bool {
        let matched = identity.model_identity == Some(self.model_identity)
            && identity.library_revision == self.library_revision
            && identity.feature_revision == chunks::FEATURE_REVISION
            && identity.semantic_revision == 0;
        if matched {
            *self.identity.borrow_mut() = Some(identity.clone());
        }
        matched
    }

    fn score(
        &mut self,
        request: &chunks::CandidateScoreRequest<'_>,
    ) -> Result<chunks::CandidateScoreResponse, chunks::ScorerFailure> {
        if !request.features.iter().all(|feature| feature.is_finite()) {
            return Err(chunks::ScorerFailure::InvalidFeature);
        }
        self.features.push(CapturedFeature {
            candidate: request.candidate,
            features: *request.features,
        });
        Ok(chunks::CandidateScoreResponse {
            request_identity: request.request_identity,
            model_identity: request.model_identity,
            candidate: request.candidate,
            score: 0.0,
        })
    }
}

pub(crate) struct FeatureCapture {
    pub(crate) identity: chunks::RankingIdentity,
    pub(crate) features: Vec<CapturedFeature>,
    pub(crate) outcomes: Vec<chunks::CandidateOutcome>,
    pub(crate) response: chunks::Response,
}

pub(crate) fn collect_features(
    session: &noble_contracts::source::Session,
    library: &Library,
    spec: &TaskSpec,
) -> Result<FeatureCapture, String> {
    let model_identity = [0x74; chunks::REVISION_BYTES];
    let mut ranker = CapturingRanker {
        model_identity,
        library_revision: *library.revision(),
        identity: std::cell::RefCell::new(None),
        features: Vec::new(),
    };
    let mut observed = Vec::new();
    let mut observer = |outcome: &chunks::CandidateOutcome| observed.push(outcome.clone());
    let mut ranker_session = chunks::RankerSession::new(&mut ranker, model_identity);
    let ranked = chunks::complete_ranked_observed(
        session,
        library,
        &request(spec, library),
        &mut ranker_session,
        &mut observer,
        monotonic_now_ns,
    );
    let identity = ranker
        .identity
        .borrow()
        .clone()
        .ok_or_else(|| format!("ranking identity missing for {}", spec.id()))?;
    if ranked.disposition != chunks::RankingDisposition::Applied {
        return Err(format!(
            "feature scorer refused {}: {:?}",
            spec.id(),
            ranked.disposition
        ));
    }
    ranker.features.sort_by_key(|row| row.candidate.ordinal);
    if ranker.features.len() != 4 {
        return Err(format!(
            "feature queue for {} had {} candidates, expected four",
            spec.id(),
            ranker.features.len()
        ));
    }
    Ok(FeatureCapture {
        identity,
        features: ranker.features,
        outcomes: observed,
        response: ranked.response,
    })
}

pub(crate) fn report_of(response: &chunks::Response) -> &chunks::Report {
    match response {
        chunks::Response::Accepted { report, .. }
        | chunks::Response::Invalid { report, .. }
        | chunks::Response::Unsupported { report, .. }
        | chunks::Response::Exhausted { report, .. }
        | chunks::Response::NoResult { report }
        | chunks::Response::InternalFailure { report, .. } => report,
    }
}

pub(crate) fn candidate_attempt_for<'a>(
    outcomes: &'a [chunks::CandidateOutcome],
    sequence: &[u16],
) -> Option<&'a chunks::CandidateOutcome> {
    outcomes.iter().find(|outcome| outcome.sequence == sequence)
}

pub(crate) fn candidate_label(
    session: &noble_contracts::source::Session,
    library: &Library,
    spec: &TaskSpec,
    envelope: &Envelope,
) -> Result<(f32, chunks::CandidateOutcome, chunks::Response), String> {
    let mut isolated = Library::new(*library.revision());
    isolated
        .admit(session, envelope.clone())
        .map_err(|refusal| format!("isolated target envelope refused: {refusal:?}"))?;
    let request = request(spec, &isolated);
    let mut outcomes = Vec::new();
    let mut observer = |outcome: &chunks::CandidateOutcome| outcomes.push(outcome.clone());
    let response = chunks::complete_observed(
        session,
        &isolated,
        &request,
        &mut observer,
        monotonic_now_ns,
    );
    let candidate_outcome = candidate_attempt_for(&outcomes, &[0])
        .cloned()
        .ok_or_else(|| format!("isolated candidate was not attempted for {}", spec.id()))?;
    if candidate_outcome.candidate.ordinal != 1 {
        return Err(format!(
            "isolated candidate ordinal changed for {}",
            spec.id()
        ));
    }
    if candidate_outcome.accepted
        && !matches!(&response, chunks::Response::Accepted { accepted, .. }
            if accepted.selected.as_slice() == [envelope.chunk_id])
    {
        return Err(format!(
            "isolated admission selected a different body for {}",
            spec.id()
        ));
    }
    let target =
        if candidate_outcome.accepted && candidate_outcome.fixture_agreement.unwrap_or(false) {
            1.0
        } else {
            0.0
        };
    Ok((target, candidate_outcome, response))
}
