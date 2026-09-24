#[test]
fn delivery_and_cancellation_have_distinct_irreversible_ordering() -> Result<(), String> {
    let mut fixture = crate::support::fixture()?;
    let started = crate::support::started(&mut fixture)?;
    let success = crate::support::observe(
        &mut fixture,
        started.attempt(),
        noble_kernel::authority::ObservedOutcome::OperationSuccess,
    )?;
    assert!(matches!(
        fixture.table.receipt(
            started.attempt(),
            noble_kernel::authority::ReceiptClaim::InvocationSuccess,
            Some(&success)
        ),
        Err(noble_kernel::authority::BoundaryError::InapplicableReceipt)
    ));
    let delivered = match fixture.table.deliver_success(started.attempt(), &success) {
        Ok(value) => value,
        Err(error) => return Err(format!("deliver: {error:?}")),
    };
    assert_eq!(
        delivered.description().claim,
        noble_kernel::authority::ReceiptClaim::InvocationSuccess
    );
    assert_eq!(
        delivered.description().scope,
        noble_kernel::authority::ReceiptScope::InvocationDelivery
    );
    assert_eq!(
        delivered.description().source,
        noble_kernel::authority::SourceId(41)
    );
    assert_eq!(
        delivered.description().invocation,
        noble_kernel::authority::InvocationOutcome::Succeeded
    );
    assert_eq!(
        fixture.table.cancel_invocation(),
        Err(noble_kernel::authority::BoundaryError::InvocationFinished)
    );
    assert!(matches!(
        fixture.table.deliver_success(started.attempt(), &success),
        Err(noble_kernel::authority::BoundaryError::InvocationFinished)
    ));
    assert_eq!(fixture.table.counters().successful_deliveries, 1);
    assert_eq!(fixture.table.counters().witness_consumptions, 1);
    Ok(())
}

#[test]
fn successful_invocation_delivery_does_not_promote_operation_failure() -> Result<(), String> {
    let mut fixture = crate::support::fixture()?;
    let started = crate::support::started(&mut fixture)?;
    let failure = crate::support::observe(
        &mut fixture,
        started.attempt(),
        noble_kernel::authority::ObservedOutcome::OperationFailure,
    )?;
    let delivery_checkpoint = noble_kernel::authority::Checkpoint {
        sequence: 4,
        now: 60,
    };
    assert_eq!(
        fixture.table.advance_from_trusted_host(delivery_checkpoint),
        Ok(())
    );
    let delivery = match fixture.table.deliver_success(started.attempt(), &failure) {
        Ok(value) => value,
        Err(error) => return Err(format!("deliver: {error:?}")),
    };
    assert_eq!(
        delivery.description().claim,
        noble_kernel::authority::ReceiptClaim::InvocationSuccess
    );
    assert_eq!(
        delivery.description().source,
        noble_kernel::authority::SourceId(41)
    );
    assert_eq!(delivery.description().checkpoint, delivery_checkpoint);
    let operation = match fixture.table.receipt(
        started.attempt(),
        noble_kernel::authority::ReceiptClaim::OperationFailure,
        Some(&failure),
    ) {
        Ok(value) => value,
        Err(error) => return Err(format!("receipt: {error:?}")),
    };
    assert_eq!(
        operation.description().source,
        noble_kernel::authority::SourceId(37)
    );
    assert_eq!(operation.description().checkpoint, fixture.facts.checkpoint);
    assert_eq!(
        operation.description().invocation,
        noble_kernel::authority::InvocationOutcome::Succeeded
    );
    assert!(matches!(
        fixture.table.receipt(
            started.attempt(),
            noble_kernel::authority::ReceiptClaim::OperationSuccess,
            Some(&failure)
        ),
        Err(noble_kernel::authority::BoundaryError::InapplicableObservation)
    ));
    assert_eq!(fixture.table.counters().successful_deliveries, 1);
    Ok(())
}
