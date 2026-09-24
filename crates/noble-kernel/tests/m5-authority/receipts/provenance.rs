#[test]
fn observation_boundary_checks_source_context_generation_plan_and_checkpoint() -> Result<(), String>
{
    let mut fixture = crate::support::fixture()?;
    let started = crate::support::started(&mut fixture)?;
    let original = crate::support::description(
        &fixture,
        started.attempt(),
        noble_kernel::authority::ObservedOutcome::OperationSuccess,
    );
    let mut wrong_source = original.clone();
    wrong_source.source = noble_kernel::authority::SourceId(99);
    assert!(matches!(
        fixture.table.observe_from_trusted_host(wrong_source),
        Err(noble_kernel::authority::BoundaryError::WrongSource)
    ));
    let mut wrong_context = original.clone();
    wrong_context.attempt.owner.invocation += 1;
    assert!(matches!(
        fixture.table.observe_from_trusted_host(wrong_context),
        Err(noble_kernel::authority::BoundaryError::WrongContext)
    ));
    let mut wrong_generation = original.clone();
    wrong_generation.attempt.generation += 1;
    assert!(matches!(
        fixture.table.observe_from_trusted_host(wrong_generation),
        Err(noble_kernel::authority::BoundaryError::StaleGeneration)
    ));
    let mut wrong_checkpoint = original.clone();
    wrong_checkpoint.checkpoint.sequence += 1;
    assert!(matches!(
        fixture.table.observe_from_trusted_host(wrong_checkpoint),
        Err(noble_kernel::authority::BoundaryError::StaleObservation)
    ));
    let mut changed = fixture.plan.description().clone();
    changed.arguments = b"target=account-b;amount=7".to_vec();
    let mut wrong_plan = original;
    wrong_plan.plan = match noble_kernel::authority::Plan::new(changed) {
        Ok(value) => value,
        Err(error) => return Err(format!("plan: {error:?}")),
    };
    assert!(matches!(
        fixture.table.observe_from_trusted_host(wrong_plan),
        Err(noble_kernel::authority::BoundaryError::ChangedPlan)
    ));
    assert_eq!(fixture.table.counters().approved_observations, 0);
    assert_eq!(fixture.table.counters().successful_deliveries, 0);
    Ok(())
}

#[test]
fn duplicate_and_conflicting_observations_cannot_overwrite_terminal_evidence() -> Result<(), String>
{
    let mut fixture = crate::support::fixture()?;
    let started = crate::support::started(&mut fixture)?;
    let success = crate::support::observe(
        &mut fixture,
        started.attempt(),
        noble_kernel::authority::ObservedOutcome::OperationSuccess,
    )?;
    let duplicate = crate::support::observe(
        &mut fixture,
        started.attempt(),
        noble_kernel::authority::ObservedOutcome::OperationSuccess,
    )?;
    assert_eq!(fixture.table.counters().approved_observations, 1);
    let failure = crate::support::description(
        &fixture,
        started.attempt(),
        noble_kernel::authority::ObservedOutcome::OperationFailure,
    );
    assert!(matches!(
        fixture.table.observe_from_trusted_host(failure),
        Err(noble_kernel::authority::BoundaryError::ConflictingObservation)
    ));
    let unknown = crate::support::description(
        &fixture,
        started.attempt(),
        noble_kernel::authority::ObservedOutcome::Unknown,
    );
    assert!(matches!(
        fixture.table.observe_from_trusted_host(unknown),
        Err(noble_kernel::authority::BoundaryError::ConflictingObservation)
    ));
    assert_eq!(
        fixture.table.receipt(
            started.attempt(),
            noble_kernel::authority::ReceiptClaim::OperationSuccess,
            Some(&success)
        ),
        fixture.table.receipt(
            started.attempt(),
            noble_kernel::authority::ReceiptClaim::OperationSuccess,
            Some(&duplicate)
        )
    );
    assert_eq!(fixture.table.counters().approved_observations, 1);
    Ok(())
}
