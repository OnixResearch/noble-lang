use super::super::monotonic_now_ns;
use super::support::*;
use noble_contracts::source::chunks::{
    self, Budget, CandidateFeatureVector, CandidateId, CandidateLimit, CandidateOutcome,
    CandidateRanker, CandidateScoreRequest, CandidateScoreResponse, Library, RankerSession,
    RankingDisposition, RankingIdentity, Request, Resource, Response, Scalar, ScorerFailure,
};
use noble_kernel::types::Ty;

#[path = "ranking/budgets.rs"]
mod budgets;
#[path = "ranking/durability.rs"]
mod durability;
#[path = "ranking/fallbacks.rs"]
mod fallbacks;
#[path = "ranking/goals.rs"]
mod goals;
#[path = "ranking/lanes.rs"]
mod lanes;
#[path = "ranking/observations.rs"]
mod observations;
#[path = "ranking/ordering.rs"]
mod ordering;

struct TestRanker {
    current: bool,
    model_identity: [u8; 32],
    library_revision: [u8; 32],
    scores: Vec<f32>,
    failure: Option<ScorerFailure>,
    invalid_response: bool,
    unknown_candidate_id: bool,
    stale_model_response: bool,
    slow_first: bool,
    features_seen: Vec<CandidateFeatureVector>,
    calls: u32,
}

impl CandidateRanker for TestRanker {
    fn model_identity(&self) -> [u8; 32] {
        self.model_identity
    }

    fn provenance_matches(&self, identity: &RankingIdentity) -> bool {
        self.current
            && identity.library_revision == self.library_revision
            && identity.model_identity == Some(self.model_identity)
            && identity.feature_revision == chunks::FEATURE_REVISION
    }

    fn score(
        &mut self,
        request: &CandidateScoreRequest<'_>,
    ) -> Result<CandidateScoreResponse, ScorerFailure> {
        debug_assert!(self.calls <= chunks::MAX_CANDIDATES);
        debug_assert_eq!(
            u32::try_from(self.features_seen.len()).ok(),
            Some(self.calls)
        );
        let features = request.features;
        let has_only_finite_features = features.iter().all(|feature| feature.is_finite());
        let has_expected_feature_count = features.len() == chunks::FEATURE_LANES;
        let is_effect_lanes_excluded = features[10] == 0.0 && features[11] == 1.0;
        let is_metadata_lanes_matching = features[55..62].iter().all(|feature| *feature == 1.0);
        let is_reserved_lane_excluded = features[63] == 0.0;
        let is_valid_feature_request = has_only_finite_features
            && has_expected_feature_count
            && is_effect_lanes_excluded
            && is_metadata_lanes_matching
            && is_reserved_lane_excluded;
        if !is_valid_feature_request {
            return Err(ScorerFailure::InvalidFeature);
        }
        self.features_seen.push(*features);
        let ordinal = self.calls;
        if self.slow_first && ordinal == 0 {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        self.calls = self.calls.saturating_add(1);
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        let score_index = usize::try_from(ordinal).map_err(|_| ScorerFailure::Evaluation)?;
        let score = self
            .scores
            .get(score_index)
            .copied()
            .ok_or(ScorerFailure::Evaluation)?;
        Ok(CandidateScoreResponse {
            request_identity: if self.invalid_response {
                [0; 32]
            } else {
                request.request_identity
            },
            model_identity: if self.stale_model_response {
                [0x73; 32]
            } else {
                request.model_identity
            },
            candidate: if self.unknown_candidate_id {
                CandidateId {
                    ordinal: request.candidate.ordinal.saturating_add(1),
                    digest: request.candidate.digest,
                }
            } else {
                request.candidate
            },
            score,
        })
    }
}

fn model_identity() -> [u8; 32] {
    [0x74; 32]
}

fn test_ranker(library: &Library, scores: Vec<f32>) -> TestRanker {
    TestRanker {
        current: true,
        model_identity: model_identity(),
        library_revision: *library.revision(),
        scores,
        failure: None,
        invalid_response: false,
        unknown_candidate_id: false,
        stale_model_response: false,
        slow_first: false,
        features_seen: Vec::new(),
        calls: 0,
    }
}

fn ranker_session(ranker: &mut TestRanker) -> RankerSession<'_> {
    RankerSession::new(ranker, model_identity())
}

fn fixture_request() -> Request {
    let mut request = request(hole(), vec![0], vec![], vec![Ty::I64]);
    request.fixtures = Some(fixtures(41, vec![(vec![], vec![Scalar::I64(1)])]));
    request
}

fn literal_library(session: &noble_contracts::source::Session) -> Result<Library, String> {
    admitted(
        session,
        &[literal(1, &[Ty::I64], b"1 "), literal(2, &[Ty::I64], b"2 ")],
    )
}

fn score_for_sequence(library: &Library, sequence: &[u16], score: f32) -> Vec<f32> {
    chunks::candidate_queue(library, CandidateLimit::from(Budget::PROFILE.candidates))
        .candidates()
        .iter()
        .map(|candidate| {
            if candidate.sequence.as_slice() == sequence {
                score
            } else {
                0.0
            }
        })
        .collect()
}

fn consumer_literal_replay(library: &Library, sequence: &[u16]) -> Option<Vec<Scalar>> {
    if sequence.len() != 1 {
        return None;
    }
    let chunk_index = usize::from(*sequence.first()?);
    let source = std::str::from_utf8(library.entries().get(chunk_index)?.source()).ok()?;
    let value = source.trim().parse::<i64>().ok()?;
    Some(vec![Scalar::I64(value)])
}

fn assert_same_result(left: &Response, right: &Response) {
    assert_eq!(left.outcome(), right.outcome());
    let left_report = match left {
        Response::Accepted { report, .. }
        | Response::Invalid { report, .. }
        | Response::Unsupported { report, .. }
        | Response::Exhausted { report, .. }
        | Response::NoResult { report }
        | Response::InternalFailure { report, .. } => report,
    };
    let right_report = match right {
        Response::Accepted { report, .. }
        | Response::Invalid { report, .. }
        | Response::Unsupported { report, .. }
        | Response::Exhausted { report, .. }
        | Response::NoResult { report }
        | Response::InternalFailure { report, .. } => report,
    };
    assert_eq!(left_report.candidates, right_report.candidates);
    assert_eq!(
        left_report.checked_candidates,
        right_report.checked_candidates
    );
    assert_eq!(left_report.rejected, right_report.rejected);
    assert_eq!(left_report.expanded_nodes, right_report.expanded_nodes);
    assert_eq!(left_report.dependency_steps, right_report.dependency_steps);
    assert_eq!(
        left_report.instantiation_bindings,
        right_report.instantiation_bindings
    );
    assert_eq!(left_report.evaluation_steps, right_report.evaluation_steps);
    assert_eq!(left_report.retained_bytes, right_report.retained_bytes);
    assert_eq!(left_report.fixtures, right_report.fixtures);
    if let (
        Response::Accepted {
            accepted: left_accepted,
            ..
        },
        Response::Accepted {
            accepted: right_accepted,
            ..
        },
    ) = (left, right)
    {
        assert_eq!(left_accepted.source, right_accepted.source);
        assert_eq!(left_accepted.selected, right_accepted.selected);
    }
}

fn checking_work(response: &Response) -> u32 {
    match response {
        Response::Accepted { report, .. }
        | Response::Invalid { report, .. }
        | Response::Unsupported { report, .. }
        | Response::Exhausted { report, .. }
        | Response::NoResult { report }
        | Response::InternalFailure { report, .. } => report.checking_work,
    }
}

fn ranked_exhaustion_observation(
    session: &noble_contracts::source::Session,
    library: &Library,
    mut request: Request,
    resource: Resource,
) -> Result<(Option<Resource>, u32), String> {
    match resource {
        Resource::CheckingWork => request.budget.checking_work = 0,
        Resource::ExpandedNodes => request.budget.expanded_nodes = 0,
        Resource::EvaluationSteps => request.budget.evaluation_steps = 0,
        Resource::RetainedBytes => request.budget.retained_bytes = 0,
        Resource::LibraryBytes
        | Resource::Entries
        | Resource::RecipeNodes
        | Resource::StackHeight
        | Resource::DependencySteps
        | Resource::InstantiationBindings
        | Resource::QuotationDepth
        | Resource::ComposedChunks
        | Resource::SourceBytes
        | Resource::Candidates
        | Resource::Diagnostics => {
            return Err(String::from(
                "unsupported resource in ranking exhaustion test",
            ))
        }
    }
    let mut ranker = test_ranker(library, vec![0.0; 31]);
    let mut scorer = ranker_session(&mut ranker);
    let result = chunks::complete_ranked(session, library, &request, &mut scorer, monotonic_now_ns);
    let Response::Exhausted { refusal, .. } = result.response else {
        return Err(format!("{resource:?} exhaustion did not fail closed"));
    };
    assert_eq!(refusal.resource, Some(resource));
    assert!(ranker.calls <= chunks::MAX_CANDIDATES);
    Ok((refusal.resource, ranker.calls))
}
