use super::*;

fn rank_for_replay(
    session: &noble_contracts::source::Session,
    library: &Library,
    request: &Request,
) -> Result<chunks::StoredRanking, String> {
    let mut ranker = test_ranker(library, vec![0.0; 31]);
    let ranked = {
        let mut scorer = ranker_session(&mut ranker);
        chunks::complete_ranked(session, library, request, &mut scorer, monotonic_now_ns)
    };
    ranked
        .stored_ranking()
        .ok_or_else(|| String::from("successful scorer order was not retainable"))
}

#[test]
fn stored_rankings_apply_without_a_model() -> Result<(), String> {
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
    assert_same_result(&ranked.response, &replayed.response);
    let identity = replayed
        .identity
        .as_ref()
        .ok_or_else(|| String::from("replay omitted request identity"))?;
    assert!(outcomes
        .iter()
        .all(|outcome| outcome.request_identity == identity.request));
    Ok(())
}

#[test]
fn stored_rankings_refuse_stale_model_and_unknown_candidates() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let request = fixture_request();
    let stored = rank_for_replay(&session, &library, &request)?;
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
    assert_same_result(&fixed, &stale_model.response);

    let mut unknown = stored;
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
    assert_same_result(&fixed, &invalid.response);
    Ok(())
}

#[test]
fn stored_rankings_refuse_changed_library_content_and_revision() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let request = fixture_request();
    let stored = rank_for_replay(&session, &library, &request)?;
    let fixed = chunks::complete(&session, &library, &request, monotonic_now_ns);

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
    assert_same_result(&changed_fixed, &changed_body.response);

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
    assert_same_result(&fixed, &stale.response);
    Ok(())
}
