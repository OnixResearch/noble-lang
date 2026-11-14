use super::*;

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
    assert_same_result(&fixed, &stale_result.response);
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
    assert_same_result(&fixed, &failed_result.response);
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
    assert_same_result(&fixed, &invalid_feature_result.response);
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
    assert_same_result(&fixed, &invalid_result.response);
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
    assert_same_result(&fixed, &stale.response);

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
    assert_same_result(&fixed, &invalid.response);

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
    assert_same_result(&fixed, &invalid.response);

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
    assert_same_result(&fixed, &stale.response);
    Ok(())
}
