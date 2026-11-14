use super::super::monotonic_now_ns;
use super::support::*;
use noble_contracts::source::chunks::{
    self, Budget, CandidateFeatureVector, CandidateId, CandidateOutcome, CandidateRanker,
    CandidateScoreRequest, CandidateScoreResponse, Library, RankerSession, RankingDisposition,
    RankingIdentity, Request, Resource, Response, Scalar, ScorerFailure,
};
use noble_kernel::types::Ty;

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
        let features = request.features;
        let metadata_matches = features[55..62].iter().all(|feature| *feature == 1.0);
        if !features.iter().all(|feature| feature.is_finite())
            || features.len() != chunks::FEATURE_LANES
            || features[10] != 0.0
            || features[11] != 1.0
            || !metadata_matches
            || features[63] != 0.0
        {
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
        let score = self
            .scores
            .get(usize::try_from(ordinal).unwrap_or(usize::MAX))
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
    chunks::candidate_queue(library, Budget::PROFILE.candidates)
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

fn assert_same_completion(left: &Response, right: &Response) {
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

fn assert_ranked_exhaustion(
    session: &noble_contracts::source::Session,
    library: &Library,
    mut request: Request,
    resource: Resource,
    expected_score_calls: u32,
) -> Result<(), String> {
    match resource {
        Resource::CheckingWork => request.budget.checking_work = 0,
        Resource::ExpandedNodes => request.budget.expanded_nodes = 0,
        Resource::EvaluationSteps => request.budget.evaluation_steps = 0,
        Resource::RetainedBytes => request.budget.retained_bytes = 0,
        _ => {
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
    assert_eq!(ranker.calls, expected_score_calls);
    Ok(())
}

#[test]
fn public_candidate_queue_is_the_bounded_shortlex_prefix() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let canonical = chunks::canonical_entry_order(&library);
    let prefix = chunks::candidate_queue(&library, 4);
    assert_eq!(canonical.len(), 2);
    assert_eq!(prefix.candidates().len(), 4);
    assert!(prefix.has_more());
    assert_eq!(prefix.candidates()[0].ordinal, 0);
    assert!(prefix.candidates()[0].sequence.is_empty());
    assert_eq!(prefix.candidates()[1].ordinal, 1);
    assert_eq!(prefix.candidates()[1].sequence.as_slice(), &[canonical[0]]);
    assert_eq!(prefix.candidates()[2].ordinal, 2);
    assert_eq!(prefix.candidates()[2].sequence.as_slice(), &[canonical[1]]);
    assert_eq!(
        prefix.candidates()[3].sequence.as_slice(),
        &[canonical[0], canonical[0]]
    );
    let complete = chunks::candidate_queue(&library, 128);
    assert_eq!(complete.candidates().len(), 31);
    assert!(!complete.has_more());
    Ok(())
}

#[test]
fn learned_order_only_reorders_the_queue_and_streams_consumer_outcomes() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let request = fixture_request();
    let target_index = library
        .entries()
        .iter()
        .position(|entry| entry.source() == b"1 ")
        .ok_or_else(|| String::from("independent literal entry missing"))?;
    let target_index =
        u16::try_from(target_index).map_err(|_| String::from("entry index overflow"))?;
    let target = chunks::candidate_queue(&library, 128)
        .candidates()
        .iter()
        .find(|candidate| candidate.sequence.as_slice() == [target_index])
        .map(|candidate| candidate.sequence.clone())
        .ok_or_else(|| String::from("target candidate missing"))?;
    let mut ranker = test_ranker(&library, score_for_sequence(&library, &target, 1.0));
    let mut outcomes: Vec<CandidateOutcome> = Vec::new();
    let mut observe = |outcome: &CandidateOutcome| outcomes.push(outcome.clone());
    let mut scorer = ranker_session(&mut ranker);
    let ranked = chunks::complete_ranked_observed(
        &session,
        &library,
        &request,
        &mut scorer,
        &mut observe,
        monotonic_now_ns,
    );
    assert_eq!(ranked.disposition, RankingDisposition::Applied);
    assert_eq!(ranker.calls, 31);
    let Response::Accepted { accepted, report } = ranked.response else {
        return Err(String::from("ranked completion was not accepted"));
    };
    assert_eq!(accepted.source, b"1 ");
    assert_eq!(
        report.fixtures.as_ref().map(|fixtures| fixtures.agreement),
        Some(true)
    );
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].sequence, target);
    assert!(outcomes[0].accepted);
    assert_eq!(outcomes[0].fixture_agreement, Some(true));
    assert_eq!(
        outcomes[0].request_identity,
        ranked.identity.as_ref().map_or([0; 32], |id| id.request)
    );
    assert_eq!(outcomes[0].candidate.ordinal, outcomes[0].ordinal);
    Ok(())
}

#[test]
fn consumer_owned_fixture_is_linked_to_the_exact_ranked_candidate() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let request = fixture_request();
    let target_index = library
        .entries()
        .iter()
        .position(|entry| entry.source() == b"1 ")
        .ok_or_else(|| String::from("consumer's literal candidate is missing"))?;
    let target_index =
        u16::try_from(target_index).map_err(|_| String::from("candidate index overflow"))?;
    let target_sequence = [target_index];
    let scores = score_for_sequence(&library, &target_sequence, 1.0);
    let mut ranker = test_ranker(&library, scores);
    let consumer_expected = vec![Scalar::I64(1)];
    let mut observations = Vec::new();
    let mut observe = |outcome: &CandidateOutcome| {
        let consumer_actual = if outcome.accepted {
            consumer_literal_replay(&library, &outcome.sequence)
        } else {
            None
        };
        let fixture_success = consumer_actual.as_ref() == Some(&consumer_expected);
        observations.push((outcome.clone(), consumer_actual, fixture_success));
    };
    let mut scorer = ranker_session(&mut ranker);
    let ranked = chunks::complete_ranked_observed(
        &session,
        &library,
        &request,
        &mut scorer,
        &mut observe,
        monotonic_now_ns,
    );
    let identity = ranked
        .identity
        .as_ref()
        .ok_or_else(|| String::from("consumer observation has no request identity"))?;
    assert_eq!(ranked.disposition, RankingDisposition::Applied);
    assert_eq!(identity.library_revision, *library.revision());
    assert_eq!(identity.feature_revision, chunks::FEATURE_REVISION);
    assert_eq!(identity.semantic_revision, request.semantic_revision);
    assert_eq!(identity.budget, request.budget);
    assert_eq!(identity.model_identity, Some(model_identity()));
    assert_ne!(identity.task_interface, [0; 32]);
    assert_ne!(identity.independent_goal, [0; 32]);
    assert_ne!(identity.candidate_set, [0; 32]);
    assert_eq!(observations.len(), 1);
    let (outcome, actual, fixture_success) = observations
        .first()
        .ok_or_else(|| String::from("consumer fixture was not called"))?;
    assert!(outcome.accepted);
    assert_eq!(outcome.sequence.as_slice(), target_sequence);
    assert_eq!(outcome.request_identity, identity.request);
    assert!(ranked.ranked_candidates.contains(&outcome.candidate));
    assert_eq!(actual.as_ref(), Some(&consumer_expected));
    assert!(fixture_success);
    Ok(())
}

#[test]
fn stale_and_failed_scorers_keep_fixed_results_and_budget_counts() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let request = fixture_request();
    let fixed = chunks::complete(&session, &library, &request, monotonic_now_ns);
    let mut stale = test_ranker(&library, Vec::new());
    stale.model_identity = [0x73; 32];
    let stale_result = {
        let mut scorer = ranker_session(&mut stale);
        chunks::complete_ranked(&session, &library, &request, &mut scorer, monotonic_now_ns)
    };
    assert_eq!(
        stale_result.disposition,
        RankingDisposition::FallbackStaleProvenance
    );
    assert_eq!(stale.calls, 0);
    assert_same_completion(&fixed, &stale_result.response);
    assert_eq!(
        checking_work(&stale_result.response),
        checking_work(&fixed)
            .saturating_add(chunks::RANKING_CONTEXT_WORK)
            .saturating_add(31u32.saturating_mul(chunks::RANKING_QUEUE_WORK)),
    );

    let mut failed = test_ranker(&library, Vec::new());
    failed.failure = Some(ScorerFailure::Evaluation);
    let failed_result = {
        let mut scorer = ranker_session(&mut failed);
        chunks::complete_ranked(&session, &library, &request, &mut scorer, monotonic_now_ns)
    };
    assert_eq!(
        failed_result.disposition,
        RankingDisposition::FallbackScorerFailure
    );
    assert_eq!(failed.calls, 1);
    assert_same_completion(&fixed, &failed_result.response);
    assert_eq!(
        checking_work(&failed_result.response),
        checking_work(&fixed)
            .saturating_add(chunks::RANKING_CONTEXT_WORK)
            .saturating_add(31u32.saturating_mul(chunks::RANKING_QUEUE_WORK))
            .saturating_add(chunks::RANKING_CANDIDATE_ID_WORK)
            .saturating_add(chunks::FEATURE_EXTRACTION_WORK)
            .saturating_add(chunks::RANKING_SCORE_WORK),
    );

    let mut invalid_feature = test_ranker(&library, Vec::new());
    invalid_feature.failure = Some(ScorerFailure::InvalidFeature);
    let invalid_feature_result = {
        let mut scorer = ranker_session(&mut invalid_feature);
        chunks::complete_ranked(&session, &library, &request, &mut scorer, monotonic_now_ns)
    };
    assert_eq!(
        invalid_feature_result.disposition,
        RankingDisposition::FallbackScorerFailure
    );
    assert_eq!(invalid_feature.calls, 1);
    assert_same_completion(&fixed, &invalid_feature_result.response);
    Ok(())
}

#[test]
fn invalid_scores_fall_back_and_mixed_scores_keep_invalids_after_finite() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let request = fixture_request();
    let fixed = chunks::complete(&session, &library, &request, monotonic_now_ns);
    let all_invalid = (0..31)
        .map(|ordinal| match ordinal % 3 {
            0 => f32::NAN,
            1 => f32::INFINITY,
            _ => f32::NEG_INFINITY,
        })
        .collect();
    let mut invalid_ranker = test_ranker(&library, all_invalid);
    let invalid_result = {
        let mut scorer = ranker_session(&mut invalid_ranker);
        chunks::complete_ranked(&session, &library, &request, &mut scorer, monotonic_now_ns)
    };
    assert_eq!(
        invalid_result.disposition,
        RankingDisposition::FallbackAllInvalidScores
    );
    assert_eq!(invalid_result.invalid_scores, 31);
    assert_same_completion(&fixed, &invalid_result.response);
    assert_eq!(
        checking_work(&invalid_result.response),
        checking_work(&fixed)
            .saturating_add(chunks::RANKING_CONTEXT_WORK)
            .saturating_add(31u32.saturating_mul(chunks::RANKING_QUEUE_WORK))
            .saturating_add(31u32.saturating_mul(chunks::RANKING_CANDIDATE_ID_WORK))
            .saturating_add(31u32.saturating_mul(chunks::FEATURE_EXTRACTION_WORK))
            .saturating_add(31u32.saturating_mul(chunks::RANKING_SCORE_WORK)),
    );

    let target_index = library
        .entries()
        .iter()
        .position(|entry| entry.source() == b"1 ")
        .ok_or_else(|| String::from("independent literal entry missing"))?;
    let target_index =
        u16::try_from(target_index).map_err(|_| String::from("entry index overflow"))?;
    let target = vec![target_index];
    let mut scores = score_for_sequence(&library, &target, 5.0);
    for score in &mut scores {
        if *score == 0.0 {
            *score = f32::INFINITY;
        }
    }
    let mut mixed_ranker = test_ranker(&library, scores);
    let mut outcomes: Vec<CandidateOutcome> = Vec::new();
    let mut observe = |outcome: &CandidateOutcome| outcomes.push(outcome.clone());
    let mut scorer = ranker_session(&mut mixed_ranker);
    let mixed = chunks::complete_ranked_observed(
        &session,
        &library,
        &request,
        &mut scorer,
        &mut observe,
        monotonic_now_ns,
    );
    assert_eq!(
        mixed.disposition,
        RankingDisposition::AppliedWithInvalidScores
    );
    assert_eq!(mixed.invalid_scores, 30);
    assert_eq!(
        outcomes.first().map(|outcome| outcome.sequence.as_slice()),
        Some(target.as_slice())
    );
    assert!(outcomes.first().is_some_and(|outcome| outcome.accepted));
    Ok(())
}

#[test]
fn equal_signature_behavior_stays_independent_and_goal_identity_is_bound() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let request = fixture_request();
    let wrong_index = library
        .entries()
        .iter()
        .position(|entry| entry.source() == b"2 ")
        .ok_or_else(|| String::from("wrong-behavior literal entry missing"))?;
    let right_index = library
        .entries()
        .iter()
        .position(|entry| entry.source() == b"1 ")
        .ok_or_else(|| String::from("expected literal entry missing"))?;
    let wrong_index =
        u16::try_from(wrong_index).map_err(|_| String::from("entry index overflow"))?;
    let right_index =
        u16::try_from(right_index).map_err(|_| String::from("entry index overflow"))?;
    let queue = chunks::candidate_queue(&library, Budget::PROFILE.candidates);
    let candidate_ordinal = |sequence: &[u16]| {
        queue
            .candidates()
            .iter()
            .find(|candidate| candidate.sequence.as_slice() == sequence)
            .map(|candidate| candidate.ordinal)
    };
    let wrong = vec![wrong_index];
    let right = vec![right_index];
    let wrong_ordinal = candidate_ordinal(&wrong)
        .ok_or_else(|| String::from("wrong-behavior queue entry missing"))?;
    let right_ordinal = candidate_ordinal(&right)
        .ok_or_else(|| String::from("correct-behavior queue entry missing"))?;
    let mut ranker = test_ranker(&library, score_for_sequence(&library, &wrong, 5.0));
    let mut outcomes = Vec::new();
    let mut observe = |outcome: &CandidateOutcome| outcomes.push(outcome.clone());
    let mut scorer = ranker_session(&mut ranker);
    let ranked = chunks::complete_ranked_observed(
        &session,
        &library,
        &request,
        &mut scorer,
        &mut observe,
        monotonic_now_ns,
    );
    let identity = ranked
        .identity
        .as_ref()
        .ok_or_else(|| String::from("ranked request identity missing"))?;
    assert_eq!(identity.model_identity, Some(model_identity()));
    assert_eq!(identity.library_revision, *library.revision());
    assert_eq!(identity.feature_revision, chunks::FEATURE_REVISION);
    assert_eq!(identity.semantic_revision, request.semantic_revision);
    assert_eq!(identity.budget, request.budget);
    assert_eq!(
        outcomes.first().map(|outcome| outcome.sequence.as_slice()),
        Some(wrong.as_slice())
    );
    assert!(outcomes.first().is_some_and(|outcome| outcome.accepted));
    assert_eq!(
        outcomes
            .first()
            .and_then(|outcome| outcome.fixture_agreement),
        Some(false)
    );
    let accepted = outcomes
        .iter()
        .find(|outcome| outcome.accepted)
        .ok_or_else(|| String::from("structurally accepted proposal was not observed"))?;
    assert_eq!(accepted.sequence, wrong);
    assert_eq!(accepted.request_identity, identity.request);
    assert_eq!(accepted.candidate.ordinal, accepted.ordinal);
    let Response::Accepted { accepted, report } = ranked.response else {
        return Err(String::from(
            "completion did not recover from the wrong behavior",
        ));
    };
    assert_eq!(accepted.source, b"2 ");
    assert_eq!(
        report.fixtures.as_ref().map(|fixtures| fixtures.agreement),
        Some(false)
    );

    let wrong_features = ranker
        .features_seen
        .get(usize::try_from(wrong_ordinal).map_err(|_| String::from("ordinal overflow"))?)
        .ok_or_else(|| String::from("wrong-behavior features missing"))?;
    let right_features = ranker
        .features_seen
        .get(usize::try_from(right_ordinal).map_err(|_| String::from("ordinal overflow"))?)
        .ok_or_else(|| String::from("correct-behavior features missing"))?;
    assert_ne!(wrong_features[13], right_features[13]);
    assert_ne!(wrong_features[48], right_features[48]);
    assert_ne!(wrong_features[50], right_features[50]);
    for lane in 0..chunks::FEATURE_LANES {
        if lane != 13 && lane != 48 && lane != 50 {
            assert_eq!(
                wrong_features[lane], right_features[lane],
                "feature lane {lane}"
            );
        }
    }

    let mut changed_goal = request.clone();
    if let Some(fixtures) = changed_goal.fixtures.as_mut() {
        let Some(case) = fixtures.cases.first_mut() else {
            return Err(String::from("independent fixture case missing"));
        };
        case.outputs = vec![Scalar::I64(2)];
    }
    let mut second_ranker = test_ranker(&library, vec![0.0; 31]);
    let second = {
        let mut scorer = ranker_session(&mut second_ranker);
        chunks::complete_ranked(
            &session,
            &library,
            &changed_goal,
            &mut scorer,
            monotonic_now_ns,
        )
    };
    let second_identity = second
        .identity
        .as_ref()
        .ok_or_else(|| String::from("changed goal identity missing"))?;
    assert_ne!(identity.independent_goal, second_identity.independent_goal);
    assert_ne!(identity.request, second_identity.request);
    Ok(())
}

#[test]
fn stale_library_and_unknown_adapter_ids_cannot_change_the_queue() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let request = fixture_request();
    let fixed = chunks::complete(&session, &library, &request, monotonic_now_ns);

    let mut stale_library = test_ranker(&library, vec![0.0; 31]);
    stale_library.library_revision = [0x73; 32];
    let stale = {
        let mut scorer = ranker_session(&mut stale_library);
        chunks::complete_ranked(&session, &library, &request, &mut scorer, monotonic_now_ns)
    };
    assert_eq!(
        stale.disposition,
        RankingDisposition::FallbackStaleProvenance
    );
    assert_eq!(stale_library.calls, 0);
    assert_same_completion(&fixed, &stale.response);

    let mut unknown = test_ranker(&library, vec![0.0; 31]);
    unknown.unknown_candidate_id = true;
    let invalid = {
        let mut scorer = ranker_session(&mut unknown);
        chunks::complete_ranked(&session, &library, &request, &mut scorer, monotonic_now_ns)
    };
    assert_eq!(
        invalid.disposition,
        RankingDisposition::FallbackInvalidResponse
    );
    assert_eq!(unknown.calls, 1);
    assert_same_completion(&fixed, &invalid.response);

    let mut wrong_request = test_ranker(&library, vec![0.0; 31]);
    wrong_request.invalid_response = true;
    let invalid = {
        let mut scorer = ranker_session(&mut wrong_request);
        chunks::complete_ranked(&session, &library, &request, &mut scorer, monotonic_now_ns)
    };
    assert_eq!(
        invalid.disposition,
        RankingDisposition::FallbackInvalidResponse
    );
    assert_eq!(wrong_request.calls, 1);
    assert_same_completion(&fixed, &invalid.response);

    let mut stale_response = test_ranker(&library, vec![0.0; 31]);
    stale_response.stale_model_response = true;
    let stale = {
        let mut scorer = ranker_session(&mut stale_response);
        chunks::complete_ranked(&session, &library, &request, &mut scorer, monotonic_now_ns)
    };
    assert_eq!(
        stale.disposition,
        RankingDisposition::FallbackStaleProvenance
    );
    assert_eq!(stale_response.calls, 1);
    assert_same_completion(&fixed, &stale.response);
    Ok(())
}

#[test]
fn stored_rankings_replay_without_a_model_and_refuse_stale_or_unknown_ids() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let request = fixture_request();
    let mut ranker = test_ranker(&library, score_for_sequence(&library, &[0], 1.0));
    let ranked = {
        let mut scorer = ranker_session(&mut ranker);
        chunks::complete_ranked(&session, &library, &request, &mut scorer, monotonic_now_ns)
    };
    let stored = ranked
        .stored_ranking()
        .ok_or_else(|| String::from("successful scorer order was not retainable"))?;
    let calls_before_replay = ranker.calls;
    let mut outcomes = Vec::new();
    let mut observe = |outcome: &CandidateOutcome| outcomes.push(outcome.clone());
    let replayed = chunks::complete_replayed(
        &session,
        &library,
        &request,
        &stored,
        model_identity(),
        Some(&mut observe),
        monotonic_now_ns,
    );
    assert_eq!(replayed.disposition, RankingDisposition::Applied);
    assert_eq!(ranker.calls, calls_before_replay);
    assert_eq!(replayed.ranked_candidates, stored.ordered_candidates);
    assert_same_completion(&ranked.response, &replayed.response);
    let identity = replayed
        .identity
        .as_ref()
        .ok_or_else(|| String::from("replay omitted request identity"))?;
    assert!(outcomes
        .iter()
        .all(|outcome| outcome.request_identity == identity.request));

    let fixed = chunks::complete(&session, &library, &request, monotonic_now_ns);
    let stale_model = chunks::complete_replayed(
        &session,
        &library,
        &request,
        &stored,
        [0x73; 32],
        None,
        monotonic_now_ns,
    );
    assert_eq!(
        stale_model.disposition,
        RankingDisposition::FallbackStaleProvenance
    );
    assert_same_completion(&fixed, &stale_model.response);

    let mut unknown = stored.clone();
    let Some(candidate) = unknown.ordered_candidates.first_mut() else {
        return Err(String::from("stored candidate ordering was empty"));
    };
    candidate.ordinal = u32::MAX;
    let invalid = chunks::complete_replayed(
        &session,
        &library,
        &request,
        &unknown,
        model_identity(),
        None,
        monotonic_now_ns,
    );
    assert_eq!(
        invalid.disposition,
        RankingDisposition::FallbackInvalidResponse
    );
    assert_same_completion(&fixed, &invalid.response);
    let changed_body_library = admitted(&session, &[literal(3, &[Ty::I64], b"3 ")])?;
    assert_eq!(changed_body_library.revision(), library.revision());
    let changed_body = chunks::complete_replayed(
        &session,
        &changed_body_library,
        &request,
        &stored,
        model_identity(),
        None,
        monotonic_now_ns,
    );
    assert_eq!(
        changed_body.disposition,
        RankingDisposition::FallbackStaleProvenance
    );
    let changed_fixed =
        chunks::complete(&session, &changed_body_library, &request, monotonic_now_ns);
    assert_same_completion(&changed_fixed, &changed_body.response);

    let mut stale_library = stored;
    stale_library.identity.library_revision = [0x73; 32];
    let stale = chunks::complete_replayed(
        &session,
        &library,
        &request,
        &stale_library,
        model_identity(),
        None,
        monotonic_now_ns,
    );
    assert_eq!(
        stale.disposition,
        RankingDisposition::FallbackStaleProvenance
    );
    assert_same_completion(&fixed, &stale.response);
    Ok(())
}

#[test]
fn consumer_training_rows_retain_refusals_and_slow_proposer_costs() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let mut request = request(hole(), vec![0], Vec::new(), vec![Ty::Bool]);
    request.fixtures = Some(fixtures(81, vec![(Vec::new(), vec![Scalar::Bool(true)])]));
    let mut ranker = test_ranker(&library, vec![0.0; 31]);
    ranker.slow_first = true;
    let mut outcomes = Vec::new();
    let mut observe = |outcome: &CandidateOutcome| outcomes.push(outcome.clone());
    let mut scorer = ranker_session(&mut ranker);
    let ranked = chunks::complete_ranked_observed(
        &session,
        &library,
        &request,
        &mut scorer,
        &mut observe,
        monotonic_now_ns,
    );
    assert_eq!(ranked.disposition, RankingDisposition::Applied);
    assert!(matches!(
        ranked.response.outcome(),
        "no-result" | "exhausted"
    ));
    assert!(outcomes.len() > 1);
    assert!(outcomes.iter().all(|outcome| !outcome.accepted));
    assert!(outcomes
        .iter()
        .all(|outcome| outcome.refusal_class.is_some()));
    assert!(outcomes[0].cost.scorer_elapsed_ns >= 1_000_000);
    let checking_work_slot = Resource::ALL
        .iter()
        .position(|resource| *resource == Resource::CheckingWork)
        .ok_or_else(|| String::from("checking-work resource missing"))?;
    assert!(outcomes
        .iter()
        .any(|outcome| outcome.cost.resource_deltas[checking_work_slot] > 0));
    let identity = ranked
        .identity
        .as_ref()
        .ok_or_else(|| String::from("training attempt identity missing"))?;
    assert!(outcomes
        .iter()
        .all(|outcome| outcome.request_identity == identity.request));
    Ok(())
}

#[test]
fn absent_scorer_keeps_default_result_and_budgets() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let request = fixture_request();
    let direct = chunks::complete(&session, &library, &request, monotonic_now_ns);
    let mut outcomes = Vec::new();
    let mut observe = |outcome: &CandidateOutcome| outcomes.push(outcome.clone());
    let observed =
        chunks::complete_observed(&session, &library, &request, &mut observe, monotonic_now_ns);
    assert_same_completion(&direct, &observed);
    let direct_candidates = match &direct {
        Response::Accepted { report, .. }
        | Response::Invalid { report, .. }
        | Response::Unsupported { report, .. }
        | Response::Exhausted { report, .. }
        | Response::NoResult { report }
        | Response::InternalFailure { report, .. } => report.candidates,
    };
    assert_eq!(outcomes.len(), direct_candidates as usize);
    Ok(())
}

#[test]
fn ranked_candidate_budget_exhaustion_does_not_trigger_fallback_work() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let mut request = fixture_request();
    request.budget.candidates = 0;
    let mut ranker = test_ranker(&library, Vec::new());
    let mut outcomes: Vec<CandidateOutcome> = Vec::new();
    let mut observe = |outcome: &CandidateOutcome| outcomes.push(outcome.clone());
    let mut scorer = ranker_session(&mut ranker);
    let result = chunks::complete_ranked_observed(
        &session,
        &library,
        &request,
        &mut scorer,
        &mut observe,
        monotonic_now_ns,
    );
    let Response::Exhausted { refusal, report } = result.response else {
        return Err(String::from("zero candidate budget did not exhaust"));
    };
    assert_eq!(refusal.resource, Some(Resource::Candidates));
    assert_eq!(report.candidates, 0);
    assert_eq!(ranker.calls, 0);
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].ordinal, 0);
    assert_eq!(outcomes[0].refusal_class, Some(chunks::Class::Exhausted));
    Ok(())
}

#[test]
fn features_bind_analyzed_hole_constraints_and_full_chunk_interfaces() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = admitted(&session, &[literal(7, &[Ty::I64], b"7 ")])?;
    let mut request = request(
        editor(vec![
            EditorNode::Hole,
            EditorNode::Word(String::from("drop")),
        ]),
        vec![0],
        Vec::new(),
        Vec::new(),
    );
    request.fixtures = Some(fixtures(75, vec![(Vec::new(), Vec::new())]));
    let analysis = session
        .analyze_editor(
            &request.editor,
            &request.task.stack_in,
            noble_contracts::Limits::default(),
        )
        .map_err(|error| format!("editor analysis: {error:?}"))?;
    let hole = analysis
        .holes
        .first()
        .ok_or_else(|| String::from("one-hole analysis omitted its constraint"))?;
    let queue = chunks::candidate_queue(&library, Budget::PROFILE.candidates);
    let candidate = queue
        .candidates()
        .iter()
        .find(|candidate| candidate.sequence.len() == 1)
        .ok_or_else(|| String::from("candidate queue omitted one-chunk proposals"))?;
    let ordinal = usize::try_from(candidate.ordinal)
        .map_err(|_| String::from("candidate ordinal overflow"))?;
    let index = *candidate
        .sequence
        .first()
        .ok_or_else(|| String::from("one-chunk candidate has no entry index"))?;
    let entry = library
        .entries()
        .get(usize::from(index))
        .ok_or_else(|| String::from("candidate entry is not retained"))?;
    let interface = entry.interface();
    let i64_count = interface
        .stack_in
        .iter()
        .chain(&interface.stack_out)
        .filter(|ty| **ty == Ty::I64)
        .count();
    let bool_count = interface
        .stack_in
        .iter()
        .chain(&interface.stack_out)
        .filter(|ty| **ty == Ty::Bool)
        .count();

    let mut ranker = test_ranker(&library, vec![0.0; 128]);
    let mut scorer = ranker_session(&mut ranker);
    let _ = chunks::complete_ranked(&session, &library, &request, &mut scorer, monotonic_now_ns);
    let features = ranker
        .features_seen
        .get(ordinal)
        .ok_or_else(|| String::from("one-chunk candidate features were not extracted"))?;
    assert_eq!(
        features[9],
        hole.required_output_suffix
            .len()
            .min(chunks::MAX_STACK_HEIGHT as usize) as f32
            / chunks::MAX_STACK_HEIGHT as f32
    );
    assert_eq!(
        features[10],
        hole.known_effects
            .len()
            .min(chunks::MAX_DIAGNOSTICS as usize) as f32
            / chunks::MAX_DIAGNOSTICS as f32
    );
    assert_eq!(features[11], if hole.unresolved_effect { 1.0 } else { 0.0 });
    assert_eq!(
        features[34],
        i64_count.min(chunks::MAX_STACK_HEIGHT as usize) as f32 / chunks::MAX_STACK_HEIGHT as f32
    );
    assert_eq!(
        features[35],
        bool_count.min(chunks::MAX_STACK_HEIGHT as usize) as f32 / chunks::MAX_STACK_HEIGHT as f32
    );
    Ok(())
}

#[test]
fn ranked_completion_fails_closed_at_shared_work_and_retention_limits() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    assert_ranked_exhaustion(
        &session,
        &library,
        fixture_request(),
        Resource::CheckingWork,
        0,
    )?;
    assert_ranked_exhaustion(
        &session,
        &library,
        fixture_request(),
        Resource::ExpandedNodes,
        31,
    )?;
    assert_ranked_exhaustion(
        &session,
        &library,
        fixture_request(),
        Resource::EvaluationSteps,
        31,
    )?;
    assert_ranked_exhaustion(
        &session,
        &library,
        fixture_request(),
        Resource::RetainedBytes,
        0,
    )?;
    Ok(())
}
