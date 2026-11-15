use super::*;

#[test]
fn equal_signature_wrong_behavior_remains_independently_observed() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let request = fixture_request();
    let wrong_index = library
        .entries()
        .iter()
        .position(|entry| entry.source() == b"2 ")
        .ok_or_else(|| String::from("wrong-behavior literal entry missing"))?;
    let wrong_index =
        u16::try_from(wrong_index).map_err(|_| String::from("entry index overflow"))?;
    let wrong = vec![wrong_index];
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
    let observed = outcomes
        .first()
        .ok_or_else(|| String::from("wrong-behavior proposal was not observed"))?;
    assert_eq!(observed.sequence.as_slice(), wrong.as_slice());
    assert!(observed.accepted);
    assert_eq!(observed.fixture_agreement, Some(false));
    assert_eq!(observed.request_identity, identity.request);
    assert_eq!(observed.candidate.ordinal, observed.ordinal);
    let Response::Accepted { accepted, report } = ranked.response else {
        return Err(String::from(
            "structurally accepted proposal was not returned",
        ));
    };
    assert_eq!(accepted.source, b"2 ");
    assert_eq!(
        report.fixtures.as_ref().map(|fixtures| fixtures.agreement),
        Some(false)
    );
    Ok(())
}

#[test]
fn equal_signature_features_exclude_expected_result_values() -> Result<(), String> {
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
    let queue = chunks::candidate_queue(&library, CandidateLimit::from(Budget::PROFILE.candidates));
    let candidate_ordinal = |sequence: &[u16]| {
        queue
            .candidates()
            .iter()
            .find(|candidate| candidate.sequence.as_slice() == sequence)
            .map(|candidate| candidate.ordinal)
    };
    let wrong_ordinal = candidate_ordinal(&[wrong_index])
        .ok_or_else(|| String::from("wrong-behavior queue entry missing"))?;
    let right_ordinal = candidate_ordinal(&[right_index])
        .ok_or_else(|| String::from("correct-behavior queue entry missing"))?;
    let mut ranker = test_ranker(&library, vec![0.0; 31]);
    let mut scorer = ranker_session(&mut ranker);
    let _ = chunks::complete_ranked(&session, &library, &request, &mut scorer, monotonic_now_ns);
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
    Ok(())
}

#[test]
fn independent_goal_changes_ranked_request_binding() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let request = fixture_request();
    let mut first_ranker = test_ranker(&library, vec![0.0; 31]);
    let first = {
        let mut scorer = ranker_session(&mut first_ranker);
        chunks::complete_ranked(&session, &library, &request, &mut scorer, monotonic_now_ns)
    };
    let first_identity = first
        .identity
        .as_ref()
        .ok_or_else(|| String::from("ranked request identity missing"))?;

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
    assert_ne!(
        first_identity.independent_goal,
        second_identity.independent_goal
    );
    assert_ne!(first_identity.request, second_identity.request);
    Ok(())
}
