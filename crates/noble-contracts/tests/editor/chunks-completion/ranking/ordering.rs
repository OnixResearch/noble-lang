use super::*;

#[test]
fn public_shortlex_prefix_is_bounded_and_canonical() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let canonical = chunks::canonical_entry_order(&library);
    let prefix = chunks::candidate_queue(&library, CandidateLimit::from(4));
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
    let complete = chunks::candidate_queue(&library, CandidateLimit::from(128));
    assert_eq!(complete.candidates().len(), 31);
    assert!(!complete.has_more());
    Ok(())
}

#[test]
fn declared_candidate_budget_above_profile_is_clamped_without_a_debug_panic() -> Result<(), String>
{
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let mut request = fixture_request();
    request.budget.candidates = u32::MAX;
    let direct = chunks::complete(&session, &library, &request, monotonic_now_ns);
    let mut ranker = test_ranker(&library, vec![0.0; 31]);
    let mut scorer = ranker_session(&mut ranker);
    let ranked =
        chunks::complete_ranked(&session, &library, &request, &mut scorer, monotonic_now_ns);
    assert_eq!(ranked.disposition, RankingDisposition::Applied);
    assert_eq!(ranker.calls, 31);
    assert_same_result(&direct, &ranked.response);
    Ok(())
}

#[test]
fn overheight_candidate_features_remain_bounded_before_readmission() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let (library, _, duplicate_id) = stack_tail_library(&session)?;
    let request = request(hole(), vec![0], vec![Ty::I64; 30], vec![Ty::I64; 30]);
    let duplicate_index = library
        .entries()
        .iter()
        .position(|entry| entry.envelope().chunk_id == duplicate_id)
        .ok_or_else(|| String::from("duplicating chunk missing"))?;
    let duplicate_index =
        u16::try_from(duplicate_index).map_err(|_| String::from("entry index overflow"))?;
    let queue = chunks::candidate_queue(&library, CandidateLimit::from(Budget::PROFILE.candidates));
    let ordinal = queue
        .candidates()
        .iter()
        .find(|candidate| candidate.sequence == [duplicate_index; 3])
        .map(|candidate| candidate.ordinal)
        .ok_or_else(|| String::from("overheight candidate missing"))?;
    let ordinal = usize::try_from(ordinal).map_err(|_| String::from("ordinal overflow"))?;
    let mut ranker = test_ranker(&library, vec![0.0; 31]);
    let mut scorer = ranker_session(&mut ranker);
    let ranked =
        chunks::complete_ranked(&session, &library, &request, &mut scorer, monotonic_now_ns);
    assert_eq!(ranked.disposition, RankingDisposition::Applied);
    assert_eq!(ranker.calls, 31);
    let features = ranker
        .features_seen
        .get(ordinal)
        .ok_or_else(|| String::from("overheight features missing"))?;
    assert_eq!(features[14], 1.0);
    assert_eq!(features[30], 0.0);
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
    let target = chunks::candidate_queue(&library, CandidateLimit::from(128))
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
