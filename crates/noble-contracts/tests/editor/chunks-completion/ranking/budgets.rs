use super::*;

#[test]
fn ranked_search_fails_closed_at_shared_work_and_retention_limits() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let library = literal_library(&session)?;
    let checking_work = ranked_exhaustion_observation(
        &session,
        &library,
        fixture_request(),
        Resource::CheckingWork,
    )?;
    assert_eq!(checking_work, (Some(Resource::CheckingWork), 0));
    let expanded_nodes = ranked_exhaustion_observation(
        &session,
        &library,
        fixture_request(),
        Resource::ExpandedNodes,
    )?;
    assert_eq!(expanded_nodes, (Some(Resource::ExpandedNodes), 31));
    let evaluation_steps = ranked_exhaustion_observation(
        &session,
        &library,
        fixture_request(),
        Resource::EvaluationSteps,
    )?;
    assert_eq!(evaluation_steps, (Some(Resource::EvaluationSteps), 31));
    let retained_bytes = ranked_exhaustion_observation(
        &session,
        &library,
        fixture_request(),
        Resource::RetainedBytes,
    )?;
    assert_eq!(retained_bytes, (Some(Resource::RetainedBytes), 0));
    Ok(())
}
