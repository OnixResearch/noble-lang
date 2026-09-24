#[test]
fn octet01_commit_precedes_execution_and_success_needs_observation() -> Result<(), String> {
    let mut fixture = crate::support::fixture()?;
    assert_eq!(
        fixture.table.decide(&fixture.plan, &fixture.facts),
        noble_kernel::authority::Decision::Allow
    );
    assert_eq!(fixture.table.counters().witnesses_created, 0);
    let witness = crate::support::authorized(&mut fixture)?;
    let claim = witness.claim();
    let execution = crate::support::admitted(&mut fixture, witness)?;
    let attempt = execution.attempt();
    assert_eq!(
        fixture.table.witness_snapshot(claim),
        Ok(noble_kernel::authority::WitnessSnapshot {
            claim,
            state: noble_kernel::authority::WitnessState::Consumed
        })
    );
    assert_eq!(fixture.table.counters().attempts_admitted, 1);
    assert_eq!(fixture.table.counters().protected_operations, 0);
    assert!(matches!(
        fixture.table.receipt(
            attempt,
            noble_kernel::authority::ReceiptClaim::OperationSuccess,
            None
        ),
        Err(noble_kernel::authority::BoundaryError::MissingObservation)
    ));
    let started = match fixture.table.start_execution(execution) {
        Ok(value) => value,
        Err(error) => return Err(format!("start: {error:?}")),
    };
    assert_eq!(started.plan(), &fixture.plan);
    record_success(&mut fixture, attempt)
}

fn record_success(
    fixture: &mut crate::support::Fixture,
    attempt: noble_kernel::authority::AttemptKey,
) -> Result<(), String> {
    let observation = crate::support::observe(
        fixture,
        attempt,
        noble_kernel::authority::ObservedOutcome::OperationSuccess,
    )?;
    let receipt = match fixture.table.receipt(
        attempt,
        noble_kernel::authority::ReceiptClaim::OperationSuccess,
        Some(&observation),
    ) {
        Ok(value) => value,
        Err(error) => return Err(format!("receipt: {error:?}")),
    };
    assert_eq!(
        receipt.description().claim,
        noble_kernel::authority::ReceiptClaim::OperationSuccess
    );
    assert_eq!(
        receipt.description().invocation,
        noble_kernel::authority::InvocationOutcome::Pending
    );
    assert_eq!(
        fixture.table.counters(),
        noble_kernel::authority::Counters {
            authorization_requests: 1,
            admission_requests: 1,
            witnesses_created: 1,
            witness_consumptions: 1,
            attempts_admitted: 1,
            protected_operations: 1,
            approved_observations: 1,
            successful_deliveries: 0,
        }
    );
    assert_eq!(
        fixture.table.requested_effects(),
        &[noble_kernel::types::EffId(91)]
    );
    Ok(())
}

#[test]
fn octet01_unknown_does_not_restore_or_retry_authority() -> Result<(), String> {
    let mut fixture = crate::support::fixture()?;
    let witness = crate::support::authorized(&mut fixture)?;
    let claim = witness.claim();
    let execution = crate::support::admitted(&mut fixture, witness)?;
    let started = match fixture.table.start_execution(execution) {
        Ok(value) => value,
        Err(error) => return Err(format!("start: {error:?}")),
    };
    let observation = crate::support::observe(
        &mut fixture,
        started.attempt(),
        noble_kernel::authority::ObservedOutcome::Unknown,
    )?;
    let receipt = match fixture.table.receipt(
        started.attempt(),
        noble_kernel::authority::ReceiptClaim::Unknown,
        Some(&observation),
    ) {
        Ok(value) => value,
        Err(error) => return Err(format!("receipt: {error:?}")),
    };
    assert_eq!(
        receipt.description().claim,
        noble_kernel::authority::ReceiptClaim::Unknown
    );
    assert!(matches!(
        fixture.table.receipt(
            started.attempt(),
            noble_kernel::authority::ReceiptClaim::OperationSuccess,
            Some(&observation)
        ),
        Err(noble_kernel::authority::BoundaryError::InapplicableObservation)
    ));
    let request = noble_kernel::authority::AdmissionRequest {
        claim,
        plan: fixture.plan.clone(),
    };
    assert!(matches!(
        fixture.table.admit(None, &request, &fixture.facts),
        noble_kernel::authority::Admission::Denied { witness: None, .. }
    ));
    assert_eq!(
        fixture.table.witness_snapshot(claim),
        Ok(noble_kernel::authority::WitnessSnapshot {
            claim,
            state: noble_kernel::authority::WitnessState::Consumed
        })
    );
    assert_eq!(fixture.table.counters().attempts_admitted, 1);
    assert_eq!(fixture.table.counters().protected_operations, 1);
    assert_eq!(fixture.table.counters().witness_consumptions, 1);
    assert_eq!(fixture.table.counters().admission_requests, 2);
    Ok(())
}
