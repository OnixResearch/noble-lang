#![feature(register_tool)]
#![register_tool(tigerstyle)]
//! M5 protected authority, replay and claim-applicability controls.

#[path = "m5-authority/bindings.rs"]
mod bindings;
#[path = "m5-authority/receipts.rs"]
mod receipts;
#[path = "m5-authority/sequencing.rs"]
mod sequencing;
#[path = "m5-authority/support.rs"]
mod support;

#[test]
fn octet01_denial_creates_neither_witness_nor_attempt() -> Result<(), String> {
    let mut fixture = crate::support::fixture()?;
    fixture.facts.credentials_valid = Some(false);
    assert_eq!(
        fixture.table.decide(&fixture.plan, &fixture.facts),
        noble_kernel::authority::Decision::Deny(
            noble_kernel::authority::Denial::InvalidCredentials
        )
    );
    let rejection = match fixture
        .table
        .authorize_from_trusted_host(fixture.plan, &fixture.facts)
    {
        noble_kernel::authority::Authorization::Denied(rejection) => rejection,
        other @ (noble_kernel::authority::Authorization::Authorized(_)
        | noble_kernel::authority::Authorization::Preflight(_)) => {
            return Err(format!("expected denial: {other:?}"));
        }
    };
    assert_eq!(
        rejection.receipt().description().claim,
        noble_kernel::authority::ReceiptClaim::Denial
    );
    assert_eq!(
        rejection.receipt().description().denial,
        Some(noble_kernel::authority::Denial::InvalidCredentials)
    );
    assert_eq!(fixture.table.counters().witnesses_created, 0);
    assert_eq!(fixture.table.counters().witness_consumptions, 0);
    assert_eq!(fixture.table.counters().attempts_admitted, 0);
    assert_eq!(fixture.table.counters().protected_operations, 0);
    Ok(())
}

#[test]
fn octet01_preflight_preserves_the_valid_caller_obligation() -> Result<(), String> {
    let limits = noble_kernel::authority::Limits {
        witnesses: 1,
        attempts: 0,
        requests: 4,
    };
    let mut fixture = crate::support::fixture_with_limits(limits)?;
    let witness = crate::support::authorized(&mut fixture)?;
    let request = noble_kernel::authority::AdmissionRequest {
        claim: witness.claim(),
        plan: fixture.plan.clone(),
    };
    let retained = match fixture.table.admit(Some(witness), &request, &fixture.facts) {
        noble_kernel::authority::Admission::Preflight {
            witness: Some(retained),
            rejection,
        } => {
            assert_eq!(
                rejection.receipt().description().claim,
                noble_kernel::authority::ReceiptClaim::PreflightFailure
            );
            assert_eq!(
                rejection.receipt().description().preflight,
                Some(noble_kernel::authority::PreflightFailure::AttemptCapacity)
            );
            retained
        }
        other @ (noble_kernel::authority::Admission::Committed(_)
        | noble_kernel::authority::Admission::Denied { .. }
        | noble_kernel::authority::Admission::Preflight { witness: None, .. }) => {
            return Err(format!("expected preserving preflight: {other:?}"));
        }
    };
    assert_eq!(
        fixture.table.witness_snapshot(request.claim),
        Ok(noble_kernel::authority::WitnessSnapshot {
            claim: request.claim,
            state: noble_kernel::authority::WitnessState::Live
        })
    );
    assert_eq!(fixture.table.counters().witness_consumptions, 0);
    assert_eq!(fixture.table.counters().attempts_admitted, 0);
    assert_eq!(fixture.table.counters().protected_operations, 0);
    assert_eq!(fixture.table.retire_witness(retained), Ok(()));
    assert_eq!(
        fixture.table.requested_effects(),
        &[noble_kernel::types::EffId(91)]
    );
    Ok(())
}

#[test]
fn octet03_witness_resource_is_neither_data_nor_capturable_payload() {
    let witness = noble_kernel::types::Ty::Resource(noble_kernel::authority::WITNESS_KIND);
    assert!(!witness.is_data());
    assert!(!noble_kernel::types::stack_is_data(std::slice::from_ref(
        &witness
    )));
    let aggregate =
        noble_kernel::types::Ty::Pair(Box::new(noble_kernel::types::Ty::I64), Box::new(witness));
    assert!(!aggregate.is_data());
}

#[test]
fn bounded_request_preflight_never_spends_the_last_witness() -> Result<(), String> {
    let limits = noble_kernel::authority::Limits {
        witnesses: 1,
        attempts: 1,
        requests: 1,
    };
    let mut fixture = crate::support::fixture_with_limits(limits)?;
    let witness = crate::support::authorized(&mut fixture)?;
    let request = noble_kernel::authority::AdmissionRequest {
        claim: witness.claim(),
        plan: fixture.plan.clone(),
    };
    let retained = match fixture.table.admit(Some(witness), &request, &fixture.facts) {
        noble_kernel::authority::Admission::Preflight {
            witness: Some(retained),
            rejection,
        } => {
            assert_eq!(
                rejection.receipt().description().preflight,
                Some(noble_kernel::authority::PreflightFailure::RequestCapacity)
            );
            retained
        }
        other @ (noble_kernel::authority::Admission::Committed(_)
        | noble_kernel::authority::Admission::Denied { .. }
        | noble_kernel::authority::Admission::Preflight { witness: None, .. }) => {
            return Err(format!("expected capacity rejection: {other:?}"));
        }
    };
    assert_eq!(fixture.table.counters().attempts_admitted, 0);
    assert_eq!(fixture.table.retire_witness(retained), Ok(()));
    Ok(())
}
