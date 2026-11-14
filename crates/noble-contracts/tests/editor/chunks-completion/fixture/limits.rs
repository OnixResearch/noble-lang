use super::super::monotonic_now_ns;
use super::support::*;

#[test]
fn oversized_input_is_refused_before_candidate_enumeration() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = library();
    let mut candidate = request(hole(), vec![0], vec![], vec![]);
    candidate.fixtures = Some(fixtures(
        19,
        (0..33_000).map(|_| (Vec::new(), Vec::new())).collect(),
    ));
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Exhausted { refusal, report } = response else {
        return Err(format!("retained-byte bound: {}", response.outcome()));
    };
    assert_eq!(refusal.resource, Some(Resource::RetainedBytes));
    assert_eq!(refusal.location.field, "request.retained");
    assert_eq!(refusal.limit, Budget::PROFILE.retained_bytes);
    assert!(refusal.observed > refusal.limit, "{refusal:?}");
    assert_eq!(report.candidates, 0);
    Ok(())
}

#[test]
fn reported_output_cannot_bypass_the_shared_byte_budget() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = library();
    let stack = vec![Ty::I64; 32];
    let values: Vec<Scalar> = (0..32).map(|value| Scalar::I64(i64::from(value))).collect();
    let mut candidate = request(hole(), vec![0], stack.clone(), stack);
    candidate.fixtures = Some(fixtures(
        91,
        (0..100).map(|_| (values.clone(), values.clone())).collect(),
    ));
    candidate.budget.retained_bytes = 100_000;

    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Exhausted { refusal, report } = response else {
        return Err(format!(
            "retained evidence should exhaust, got {}",
            response.outcome()
        ));
    };
    assert_eq!(refusal.resource, Some(Resource::RetainedBytes));
    assert_eq!(refusal.limit, 100_000);
    assert!(refusal.observed > refusal.limit);
    assert_eq!(report.candidates, 1);
    assert_eq!(report.evaluation_steps, 0);
    assert!(report.fixtures.is_none());
    Ok(())
}

#[test]
fn raised_request_budgets_are_clamped_to_the_profile() {
    let mut raised = Budget::PROFILE;
    raised.candidates = u32::MAX;
    raised.evaluation_steps = u32::MAX;
    let clamped = raised.lowered(&Budget::PROFILE);
    assert_eq!(clamped.candidates, Budget::PROFILE.candidates);
    assert_eq!(clamped.evaluation_steps, Budget::PROFILE.evaluation_steps);
}

#[test]
fn the_257th_step_exhausts_at_the_256_step_limit() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let mut candidate = request(
        editor(vec![EditorNode::Integer(1), EditorNode::Hole]),
        vec![1],
        vec![],
        vec![Ty::I64],
    );
    candidate.fixtures = Some(fixtures(
        18,
        (0..129)
            .map(|_| (Vec::new(), vec![Scalar::I64(1)]))
            .collect(),
    ));
    let response = complete(&session, &library(), &candidate, monotonic_now_ns);
    let Response::Exhausted { refusal, report } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(refusal.resource, Some(Resource::EvaluationSteps));
    assert_eq!(refusal.observed, 257);
    assert_eq!(refusal.limit, 256);
    assert_eq!(report.evaluation_steps, 256);
    Ok(())
}
