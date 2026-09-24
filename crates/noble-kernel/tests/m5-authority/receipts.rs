#[path = "receipts/completion.rs"]
mod completion;
#[path = "receipts/finality.rs"]
mod finality;
#[path = "receipts/provenance.rs"]
mod provenance;

#[test]
fn octet04_failure_observation_supports_failure_not_success() -> Result<(), String> {
    let mut fixture = crate::support::fixture()?;
    let started = crate::support::started(&mut fixture)?;
    let observation = crate::support::observe(
        &mut fixture,
        started.attempt(),
        noble_kernel::authority::ObservedOutcome::OperationFailure,
    )?;
    let receipt = match fixture.table.receipt(
        started.attempt(),
        noble_kernel::authority::ReceiptClaim::OperationFailure,
        Some(&observation),
    ) {
        Ok(value) => value,
        Err(error) => return Err(format!("receipt: {error:?}")),
    };
    assert_eq!(
        receipt.description().claim,
        noble_kernel::authority::ReceiptClaim::OperationFailure
    );
    assert_eq!(
        receipt.description().invocation,
        noble_kernel::authority::InvocationOutcome::Pending
    );
    assert!(matches!(
        fixture.table.receipt(
            started.attempt(),
            noble_kernel::authority::ReceiptClaim::OperationSuccess,
            Some(&observation)
        ),
        Err(noble_kernel::authority::BoundaryError::InapplicableObservation)
    ));
    assert_eq!(
        fixture.table.fail_invocation(),
        Ok(noble_kernel::authority::InvocationOutcome::Failed)
    );
    assert_eq!(fixture.table.counters().witness_consumptions, 1);
    assert_eq!(fixture.table.counters().protected_operations, 1);
    assert_eq!(fixture.table.counters().successful_deliveries, 0);
    Ok(())
}

#[test]
fn octet04_plan_witness_attempt_and_missing_observation_cannot_claim_success() -> Result<(), String>
{
    let mut fixture = crate::support::fixture()?;
    let guessed = noble_kernel::authority::AttemptKey {
        owner: fixture.plan.description().owner,
        slot: 0,
        generation: 1,
    };
    assert!(matches!(
        fixture.table.receipt(
            guessed,
            noble_kernel::authority::ReceiptClaim::OperationSuccess,
            None
        ),
        Err(noble_kernel::authority::BoundaryError::MissingObservation)
    ));
    let witness = crate::support::authorized(&mut fixture)?;
    assert!(matches!(
        fixture.table.receipt(
            guessed,
            noble_kernel::authority::ReceiptClaim::OperationSuccess,
            None
        ),
        Err(noble_kernel::authority::BoundaryError::MissingObservation)
    ));
    let execution = crate::support::admitted(&mut fixture, witness)?;
    let attempt = execution.attempt();
    assert!(matches!(
        fixture.table.receipt(
            attempt,
            noble_kernel::authority::ReceiptClaim::OperationSuccess,
            None
        ),
        Err(noble_kernel::authority::BoundaryError::MissingObservation)
    ));
    let premature = crate::support::description(
        &fixture,
        attempt,
        noble_kernel::authority::ObservedOutcome::OperationSuccess,
    );
    assert!(matches!(
        fixture.table.observe_from_trusted_host(premature),
        Err(noble_kernel::authority::BoundaryError::NotStarted)
    ));
    assert_eq!(fixture.table.counters().approved_observations, 0);
    assert_eq!(fixture.table.counters().protected_operations, 0);
    Ok(())
}

#[test]
fn octet04_another_attempts_observation_is_inapplicable() -> Result<(), String> {
    let mut fixture = crate::support::fixture()?;
    let first = crate::support::started(&mut fixture)?;
    let second = crate::support::started(&mut fixture)?;
    let observation = crate::support::observe(
        &mut fixture,
        first.attempt(),
        noble_kernel::authority::ObservedOutcome::OperationSuccess,
    )?;
    assert!(matches!(
        fixture.table.receipt(
            second.attempt(),
            noble_kernel::authority::ReceiptClaim::OperationSuccess,
            Some(&observation)
        ),
        Err(noble_kernel::authority::BoundaryError::InapplicableObservation)
    ));
    assert_eq!(fixture.table.counters().approved_observations, 1);
    assert_eq!(fixture.table.counters().successful_deliveries, 0);
    Ok(())
}

#[test]
fn octet04_imported_bytes_need_provenance_and_cannot_import_trust_flags() -> Result<(), String> {
    let mut fixture = crate::support::fixture()?;
    let started = crate::support::started(&mut fixture)?;
    let description = noble_kernel::authority::ReceiptDescription {
        plan: fixture.plan.clone(),
        boundary_owner: fixture.plan.description().owner,
        attempt: Some(started.attempt()),
        source: noble_kernel::authority::SourceId(37),
        checkpoint: fixture.facts.checkpoint,
        claim: noble_kernel::authority::ReceiptClaim::OperationSuccess,
        scope: noble_kernel::authority::ReceiptScope::OperationBoundary,
        invocation: noble_kernel::authority::InvocationOutcome::Pending,
        denial: None,
        preflight: None,
    };
    assert!(matches!(
        noble_kernel::authority::UntrustedReceipt::import(description.clone(), Some(true)),
        Err(noble_kernel::authority::BoundaryError::ImportedTrustFlag)
    ));
    assert!(matches!(
        noble_kernel::authority::UntrustedReceipt::import(description.clone(), Some(false)),
        Err(noble_kernel::authority::BoundaryError::ImportedTrustFlag)
    ));
    let imported = match noble_kernel::authority::UntrustedReceipt::import(description, None) {
        Ok(value) => value,
        Err(error) => return Err(format!("import: {error:?}")),
    };
    assert_eq!(fixture.table.counters().approved_observations, 0);
    assert!(matches!(
        fixture.table.receipt(
            started.attempt(),
            noble_kernel::authority::ReceiptClaim::OperationSuccess,
            None
        ),
        Err(noble_kernel::authority::BoundaryError::MissingObservation)
    ));
    let observation = crate::support::observe(
        &mut fixture,
        started.attempt(),
        noble_kernel::authority::ObservedOutcome::OperationSuccess,
    )?;
    let receipt = match fixture.table.admit_receipt(&imported, &observation) {
        Ok(value) => value,
        Err(error) => return Err(format!("admit: {error:?}")),
    };
    assert_eq!(receipt.description(), imported.description());
    let mut wrong = imported.description().clone();
    wrong.source = noble_kernel::authority::SourceId(99);
    let wrong = match noble_kernel::authority::UntrustedReceipt::import(wrong, None) {
        Ok(value) => value,
        Err(error) => return Err(format!("import: {error:?}")),
    };
    assert!(matches!(
        fixture.table.admit_receipt(&wrong, &observation),
        Err(noble_kernel::authority::BoundaryError::InapplicableReceipt)
    ));
    assert_eq!(fixture.table.counters().protected_operations, 1);
    Ok(())
}
