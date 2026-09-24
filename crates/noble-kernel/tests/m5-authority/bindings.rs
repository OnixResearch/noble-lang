#[path = "bindings/cases.rs"]
mod cases;

#[test]
fn octet02_every_binding_and_required_current_fact_is_checked() -> Result<(), String> {
    for case in cases::ALL {
        match rejects_mutation(case) {
            Ok(()) => (),
            Err(error) => return Err(format!("{case:?}: {error}")),
        }
    }
    Ok(())
}

fn rejects_mutation(case: cases::Mutation) -> Result<(), String> {
    let mut fixture = crate::support::fixture()?;
    let witness = crate::support::authorized(&mut fixture)?;
    let original = witness.claim();
    let mut request = noble_kernel::authority::AdmissionRequest {
        claim: original,
        plan: fixture.plan.clone(),
    };
    let expected = cases::apply(case, &mut fixture, &mut request)?;
    let retained = match fixture.table.admit(Some(witness), &request, &fixture.facts) {
        noble_kernel::authority::Admission::Denied {
            witness: Some(retained),
            rejection,
        } => {
            assert_eq!(rejection.receipt().description().denial, Some(expected));
            assert_eq!(
                rejection.receipt().description().claim,
                noble_kernel::authority::ReceiptClaim::Denial
            );
            retained
        }
        other @ (noble_kernel::authority::Admission::Committed(_)
        | noble_kernel::authority::Admission::Preflight { .. }
        | noble_kernel::authority::Admission::Denied { witness: None, .. }) => {
            return Err(format!("expected denial: {other:?}"));
        }
    };
    assert_eq!(fixture.table.counters().admission_requests, 1);
    assert_eq!(fixture.table.counters().attempts_admitted, 0);
    assert_eq!(fixture.table.counters().protected_operations, 0);
    assert_eq!(fixture.table.counters().witness_consumptions, 0);
    assert_eq!(
        fixture.table.requested_effects(),
        &[request.plan.description().operation.effect]
    );
    assert_eq!(
        fixture.table.witness_snapshot(original),
        Ok(noble_kernel::authority::WitnessSnapshot {
            claim: original,
            state: noble_kernel::authority::WitnessState::Live
        })
    );
    assert_eq!(fixture.table.retire_witness(retained), Ok(()));
    Ok(())
}

#[test]
fn octet02_serialized_tag_cannot_mint_live_authority() -> Result<(), String> {
    let mut fixture = crate::support::fixture()?;
    let witness = crate::support::authorized(&mut fixture)?;
    let claim = witness.claim();
    let request = noble_kernel::authority::AdmissionRequest {
        claim,
        plan: fixture.plan.clone(),
    };
    match fixture.table.admit(None, &request, &fixture.facts) {
        noble_kernel::authority::Admission::Denied {
            witness: None,
            rejection,
        } => assert_eq!(
            rejection.receipt().description().denial,
            Some(noble_kernel::authority::Denial::UntrustedWitnessClaim)
        ),
        other @ (noble_kernel::authority::Admission::Committed(_)
        | noble_kernel::authority::Admission::Preflight { .. }
        | noble_kernel::authority::Admission::Denied {
            witness: Some(_), ..
        }) => {
            return Err(format!("raw tag was not denied: {other:?}"));
        }
    }
    assert_eq!(fixture.table.counters().attempts_admitted, 0);
    assert_eq!(fixture.table.counters().protected_operations, 0);
    assert_eq!(fixture.table.counters().witness_consumptions, 0);
    assert_eq!(fixture.table.counters().admission_requests, 1);
    // The rejected copied description neither consumes nor duplicates the owner.
    let execution = crate::support::admitted(&mut fixture, witness)?;
    assert_eq!(fixture.table.counters().witness_consumptions, 1);
    assert_eq!(
        fixture
            .table
            .attempt_snapshot(execution.attempt())
            .map(|snapshot| snapshot.started),
        Ok(false)
    );
    assert!(matches!(
        fixture.table.admit(None, &request, &fixture.facts),
        noble_kernel::authority::Admission::Denied { witness: None, .. }
    ));
    assert_eq!(fixture.table.counters().attempts_admitted, 1);
    Ok(())
}

#[test]
fn same_checkpoint_quota_is_reserved_across_distinct_witnesses() -> Result<(), String> {
    let mut fixture = crate::support::fixture()?;
    fixture.facts.quota_remaining = Some(7);
    let first = crate::support::authorized(&mut fixture)?;
    let second = crate::support::authorized(&mut fixture)?;
    let execution = crate::support::admitted(&mut fixture, first)?;
    let request = noble_kernel::authority::AdmissionRequest {
        claim: second.claim(),
        plan: fixture.plan.clone(),
    };
    match fixture.table.admit(Some(second), &request, &fixture.facts) {
        noble_kernel::authority::Admission::Denied {
            witness: Some(retained),
            rejection,
        } => {
            assert_eq!(
                rejection.receipt().description().denial,
                Some(noble_kernel::authority::Denial::QuotaExhausted)
            );
            assert_eq!(fixture.table.retire_witness(retained), Ok(()));
        }
        other @ (noble_kernel::authority::Admission::Committed(_)
        | noble_kernel::authority::Admission::Preflight { .. }
        | noble_kernel::authority::Admission::Denied { witness: None, .. }) => {
            return Err(format!("quota overspend accepted: {other:?}"));
        }
    }
    assert_eq!(fixture.table.counters().witness_consumptions, 1);
    assert_eq!(fixture.table.counters().attempts_admitted, 1);
    assert_eq!(fixture.table.counters().protected_operations, 0);
    assert_eq!(
        fixture
            .table
            .attempt_snapshot(execution.attempt())
            .map(|snapshot| snapshot.started),
        Ok(false)
    );
    Ok(())
}

#[test]
fn changed_grant_cannot_rebind_an_existing_witness() -> Result<(), String> {
    let mut fixture = crate::support::fixture()?;
    let witness = crate::support::authorized(&mut fixture)?;
    let mut changed = fixture.plan.description().clone();
    changed.arguments = b"target=account-b;amount=7".to_vec();
    let changed = match noble_kernel::authority::Plan::new(changed) {
        Ok(value) => value,
        Err(error) => return Err(format!("plan: {error:?}")),
    };
    let facts = crate::support::facts_for(&changed, fixture.facts.checkpoint);
    let request = noble_kernel::authority::AdmissionRequest {
        claim: witness.claim(),
        plan: changed,
    };
    match fixture.table.admit(Some(witness), &request, &facts) {
        noble_kernel::authority::Admission::Denied {
            witness: Some(retained),
            rejection,
        } => {
            assert_eq!(
                rejection.receipt().description().denial,
                Some(noble_kernel::authority::Denial::ChangedPlan)
            );
            assert_eq!(fixture.table.retire_witness(retained), Ok(()));
        }
        other @ (noble_kernel::authority::Admission::Committed(_)
        | noble_kernel::authority::Admission::Preflight { .. }
        | noble_kernel::authority::Admission::Denied { witness: None, .. }) => {
            return Err(format!("witness rebound: {other:?}"));
        }
    }
    assert_eq!(fixture.table.counters().attempts_admitted, 0);
    Ok(())
}
