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

fn fixture_record(cases: &[Fixture]) -> Value {
    json!(cases
        .iter()
        .map(|case| json!({
            "inputs": case.inputs.iter().copied().map(scalar_json).collect::<Vec<_>>(),
            "outputs": case.outputs.iter().copied().map(scalar_json).collect::<Vec<_>>()
        }))
        .collect::<Vec<_>>())
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
    let encoded = serde_json::to_vec(&fixture_record(&cases))
        .expect("fixture records contain only JSON values");
    let digest = blake3::hash(&encoded);
    let mut id = [0u8; chunks::IDENTITY_BYTES];
    id.copy_from_slice(&digest.as_bytes()[..chunks::IDENTITY_BYTES]);
    FixtureSet { id, cases }
}

fn single_candidate_budget() -> Budget {
    let mut budget = Budget::PROFILE;
    budget.candidates = 4;
    budget.composed_chunks = 1;
    budget
}

pub(crate) fn request(spec: &TaskSpec, library: &Library) -> Request {
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
        budget: single_candidate_budget(),
    }
}

pub(crate) fn monotonic_now_ns() -> u64 {
    static START: std::sync::LazyLock<std::time::Instant> =
        std::sync::LazyLock::new(std::time::Instant::now);
    let Ok(nanos) = u64::try_from(START.elapsed().as_nanos()) else {
        return u64::MAX;
    };
    nanos
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
        let is_matched = identity.model_identity == Some(self.model_identity)
            && identity.library_revision == self.library_revision
            && identity.feature_revision == chunks::FEATURE_REVISION
            && identity.semantic_revision == 0;
        if is_matched {
            *self.identity.borrow_mut() = Some(identity.clone());
        }
        is_matched
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

pub(crate) struct Feature {
    pub(crate) identity: chunks::RankingIdentity,
    pub(crate) features: Vec<CapturedFeature>,
    pub(crate) outcomes: Vec<chunks::CandidateOutcome>,
    pub(crate) response: chunks::Response,
}

struct ObservedOutcomes(Vec<chunks::CandidateOutcome>);

impl ObservedOutcomes {
    fn push(&mut self, outcome: &chunks::CandidateOutcome) {
        self.0.push(outcome.clone());
    }
}

fn capturing_ranker(library: &Library) -> CapturingRanker {
    CapturingRanker {
        model_identity: [0x74; chunks::REVISION_BYTES],
        library_revision: *library.revision(),
        identity: std::cell::RefCell::new(None),
        features: Vec::new(),
    }
}

fn observed_ranked_response(
    session: &noble_contracts::source::Session,
    library: &Library,
    spec: &TaskSpec,
    ranker: &mut CapturingRanker,
    observed: &mut ObservedOutcomes,
) -> chunks::RankedResponse {
    let mut observer = |outcome: &chunks::CandidateOutcome| observed.push(outcome);
    let model_identity = ranker.model_identity;
    let mut ranker_session = chunks::RankerSession::new(ranker, model_identity);
    chunks::complete_ranked_observed(
        session,
        library,
        &request(spec, library),
        &mut ranker_session,
        &mut observer,
        monotonic_now_ns,
    )
}

fn checked_identity(
    ranker: &CapturingRanker,
    spec: &TaskSpec,
) -> Result<chunks::RankingIdentity, String> {
    ranker
        .identity
        .borrow()
        .clone()
        .ok_or_else(|| format!("ranking identity missing for {}", spec.id()))
}

fn checked_feature_queue(ranker: &mut CapturingRanker, spec: &TaskSpec) -> Result<(), String> {
    ranker.features.sort_by_key(|row| row.candidate.ordinal);
    if ranker.features.len() != 4 {
        return Err(format!(
            "feature queue for {} had {} candidates, expected four",
            spec.id(),
            ranker.features.len()
        ));
    }
    Ok(())
}

fn checked_ranked(
    ranker: &mut CapturingRanker,
    disposition: chunks::RankingDisposition,
    spec: &TaskSpec,
) -> Result<chunks::RankingIdentity, String> {
    let identity = checked_identity(ranker, spec)?;
    if disposition != chunks::RankingDisposition::Applied {
        return Err(format!(
            "feature scorer refused {}: {disposition:?}",
            spec.id()
        ));
    }
    checked_feature_queue(ranker, spec)?;
    Ok(identity)
}

pub(crate) fn features(
    session: &noble_contracts::source::Session,
    library: &Library,
    spec: &TaskSpec,
) -> Result<Feature, String> {
    let mut ranker = capturing_ranker(library);
    let mut observed = ObservedOutcomes(Vec::new());
    let ranked = observed_ranked_response(session, library, spec, &mut ranker, &mut observed);
    let identity = checked_ranked(&mut ranker, ranked.disposition, spec)?;
    Ok(Feature {
        identity,
        features: ranker.features,
        outcomes: observed.0,
        response: ranked.response,
    })
}
