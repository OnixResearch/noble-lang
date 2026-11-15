use super::*;

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
        let is_fixture_success = consumer_actual.as_ref() == Some(&consumer_expected);
        observations.push((outcome.clone(), consumer_actual, is_fixture_success));
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
    assert_same_result(&direct, &observed);
    let direct_candidates = match &direct {
        Response::Accepted { report, .. }
        | Response::Invalid { report, .. }
        | Response::Unsupported { report, .. }
        | Response::Exhausted { report, .. }
        | Response::NoResult { report }
        | Response::InternalFailure { report, .. } => report.candidates,
    };
    let direct_candidate_count =
        usize::try_from(direct_candidates).map_err(|_| String::from("candidate count overflow"))?;
    assert_eq!(outcomes.len(), direct_candidate_count);
    let request_identity = outcomes
        .first()
        .ok_or_else(|| String::from("default observer recorded no candidate identity"))?
        .request_identity;
    assert_ne!(request_identity, [0; 32]);
    for outcome in &outcomes {
        assert_eq!(outcome.request_identity, request_identity);
    }
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
