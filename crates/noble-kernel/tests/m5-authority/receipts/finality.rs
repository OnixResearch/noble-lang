#[test]
fn octet04_cancelled_invocation_allows_late_operation_success_only() -> Result<(), String> {
    let mut fixture = crate::support::fixture()?;
    let started = crate::support::started(&mut fixture)?;
    assert_eq!(
        fixture.table.cancel_invocation(),
        Ok(noble_kernel::authority::InvocationOutcome::Cancelled)
    );
    let unknown = crate::support::observe(
        &mut fixture,
        started.attempt(),
        noble_kernel::authority::ObservedOutcome::Unknown,
    )?;
    assert!(matches!(
        fixture.table.receipt(
            started.attempt(),
            noble_kernel::authority::ReceiptClaim::Unknown,
            Some(&unknown)
        ),
        Ok(receipt)
            if receipt.description().invocation == noble_kernel::authority::InvocationOutcome::Cancelled
    ));
    let success = crate::support::observe(
        &mut fixture,
        started.attempt(),
        noble_kernel::authority::ObservedOutcome::OperationSuccess,
    )?;
    check_late_success(&mut fixture, started.attempt(), &success)?;
    assert_eq!(
        fixture.table.cancel_invocation(),
        Ok(noble_kernel::authority::InvocationOutcome::Cancelled)
    );
    assert_eq!(fixture.table.counters().witness_consumptions, 1);
    assert_eq!(fixture.table.counters().protected_operations, 1);
    assert_eq!(fixture.table.counters().successful_deliveries, 0);
    Ok(())
}

fn check_late_success(
    fixture: &mut crate::support::Fixture,
    attempt: noble_kernel::authority::AttemptKey,
    success: &noble_kernel::authority::Observation,
) -> Result<(), String> {
    let receipt = match fixture.table.receipt(
        attempt,
        noble_kernel::authority::ReceiptClaim::OperationSuccess,
        Some(success),
    ) {
        Ok(value) => value,
        Err(error) => return Err(format!("receipt: {error:?}")),
    };
    assert_eq!(
        receipt.description().claim,
        noble_kernel::authority::ReceiptClaim::OperationSuccess
    );
    assert_eq!(
        receipt.description().scope,
        noble_kernel::authority::ReceiptScope::OperationBoundary
    );
    assert_eq!(
        receipt.description().invocation,
        noble_kernel::authority::InvocationOutcome::Cancelled
    );
    assert!(matches!(
        fixture.table.deliver_success(attempt, success),
        Err(noble_kernel::authority::BoundaryError::InvocationFinished)
    ));
    assert!(matches!(
        fixture.table.receipt(
            attempt,
            noble_kernel::authority::ReceiptClaim::InvocationSuccess,
            Some(success)
        ),
        Err(noble_kernel::authority::BoundaryError::InapplicableReceipt)
    ));
    Ok(())
}

#[test]
fn cancel_before_start_revokes_execution_without_restoring_witness() -> Result<(), String> {
    let mut fixture = crate::support::fixture()?;
    let witness = crate::support::authorized(&mut fixture)?;
    let claim = witness.claim();
    let execution = crate::support::admitted(&mut fixture, witness)?;
    assert_eq!(
        fixture.table.cancel_invocation(),
        Ok(noble_kernel::authority::InvocationOutcome::Cancelled)
    );
    assert!(matches!(
        fixture.table.start_execution(execution),
        Err(noble_kernel::authority::BoundaryError::InvocationFinished)
    ));
    assert_eq!(
        fixture.table.witness_snapshot(claim),
        Ok(noble_kernel::authority::WitnessSnapshot {
            claim,
            state: noble_kernel::authority::WitnessState::Consumed
        })
    );
    assert_eq!(fixture.table.counters().attempts_admitted, 1);
    assert_eq!(fixture.table.counters().protected_operations, 0);
    assert_eq!(fixture.table.counters().successful_deliveries, 0);
    Ok(())
}
