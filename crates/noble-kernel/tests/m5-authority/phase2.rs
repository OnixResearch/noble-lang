//! Issue #66 Phase 2: finite Rust controls over the M5 authority ledger.
//!
//! These are finite comparisons against the real `noble_kernel::authority` table, not proofs. Two
//! witnesses sharing one checkpoint's quota is already measured by
//! `bindings::same_checkpoint_quota_is_reserved_across_distinct_witnesses`; what is measured here is
//! what that scenario does not: admission order independence, a fresh snapshot that does not carry
//! prior reservations being refused rather than read as capacity, and finishing an invocation between
//! admission and start stopping the start without restoring any authority.

#[path = "phase2/checkpoint.rs"]
mod checkpoint;

const UNITS: u64 = 7;

fn quote(facts: &mut noble_kernel::authority::CurrentFacts, available: u64) {
    facts.quota_remaining = Some(available);
}

/// Authorize two independent witnesses over the same plan and quota.
fn two_witnesses(
    fixture: &mut super::support::Fixture,
) -> Result<
    (
        noble_kernel::authority::Witness,
        noble_kernel::authority::Witness,
    ),
    String,
> {
    let first = super::support::authorized(fixture)?;
    let second = super::support::authorized(fixture)?;
    assert_ne!(first.claim(), second.claim(), "two distinct witnesses");
    Ok((first, second))
}

fn admit_witness(
    fixture: &mut super::support::Fixture,
    witness: noble_kernel::authority::Witness,
) -> noble_kernel::authority::Admission {
    let request = noble_kernel::authority::AdmissionRequest {
        claim: witness.claim(),
        plan: fixture.plan.clone(),
    };
    fixture.table.admit(Some(witness), &request, &fixture.facts)
}

#[test]
fn both_admission_orders_charge_the_same_quota() -> Result<(), String> {
    // Two witnesses, quota enough for exactly one admission, admitted in both orders. The observable
    // outcome must be the same: one commit and one quota denial either way.
    let mut forward = super::support::fixture()?;
    quote(&mut forward.facts, UNITS);
    let (first, second) = two_witnesses(&mut forward)?;
    let forward_outcomes = [
        matches!(
            admit_witness(&mut forward, first),
            noble_kernel::authority::Admission::Committed(_)
        ),
        matches!(
            admit_witness(&mut forward, second),
            noble_kernel::authority::Admission::Denied { .. }
        ),
    ];

    let mut reverse = super::support::fixture()?;
    quote(&mut reverse.facts, UNITS);
    let (first, second) = two_witnesses(&mut reverse)?;
    let reverse_outcomes = [
        matches!(
            admit_witness(&mut reverse, second),
            noble_kernel::authority::Admission::Committed(_)
        ),
        matches!(
            admit_witness(&mut reverse, first),
            noble_kernel::authority::Admission::Denied { .. }
        ),
    ];
    assert_eq!(forward_outcomes, [true, true], "forward order");
    assert_eq!(reverse_outcomes, [true, true], "reverse order");
    assert_eq!(forward.table.counters(), reverse.table.counters());
    Ok(())
}

#[test]
fn quota_exhaustion_stops_before_consumption() -> Result<(), String> {
    // Authorization sees enough quota; the checkpoint's available quota is then below one plan's
    // units by the time admission charges it. No quota record exists yet, so this is a plain
    // exhaustion rather than a snapshot mismatch.
    let mut fixture = super::support::fixture()?;
    let witness = super::support::authorized(&mut fixture)?;
    quote(&mut fixture.facts, UNITS.saturating_sub(1));
    let retained = match admit_witness(&mut fixture, witness) {
        noble_kernel::authority::Admission::Denied { witness, rejection } => {
            assert_eq!(
                rejection.receipt().description().denial,
                Some(noble_kernel::authority::Denial::QuotaExhausted)
            );
            witness.ok_or("a denial returns the original obligation")?
        }
        other => return Err(std::format!("expected a quota denial: {other:?}")),
    };
    assert_eq!(fixture.table.counters().attempts_admitted, 0);
    assert_eq!(fixture.table.counters().witness_consumptions, 0);
    assert_eq!(fixture.table.counters().protected_operations, 0);
    assert_eq!(
        fixture.table.witness_snapshot(retained.claim()),
        Ok(noble_kernel::authority::WitnessSnapshot {
            claim: retained.claim(),
            state: noble_kernel::authority::WitnessState::Live
        })
    );
    Ok(())
}

#[test]
fn a_fresh_snapshot_without_carry_forward_is_refused() -> Result<(), String> {
    // The host premise: a fresh quota snapshot must account for prior reservations. A snapshot that
    // reports a different available quota at the same checkpoint is refused rather than read as
    // extra capacity, so the kernel never enforces cross-snapshot conservation.
    let mut fixture = super::support::fixture()?;
    quote(&mut fixture.facts, UNITS);
    let (first, second) = two_witnesses(&mut fixture)?;
    assert!(matches!(
        admit_witness(&mut fixture, first),
        noble_kernel::authority::Admission::Committed(_)
    ));
    quote(
        &mut fixture.facts,
        UNITS.saturating_mul(2).saturating_add(6),
    );
    let retained = match admit_witness(&mut fixture, second) {
        noble_kernel::authority::Admission::Denied { witness, rejection } => {
            assert_eq!(
                rejection.receipt().description().denial,
                Some(noble_kernel::authority::Denial::InconsistentQuota),
                "a replenished snapshot is refused, not accepted as capacity"
            );
            witness.ok_or("a denial returns the original obligation")?
        }
        other => {
            return Err(std::format!(
                "expected an inconsistent-quota denial: {other:?}"
            ))
        }
    };
    assert_eq!(fixture.table.counters().attempts_admitted, 1);
    assert_eq!(fixture.table.counters().witness_consumptions, 1);
    assert_eq!(
        fixture.table.witness_snapshot(retained.claim()),
        Ok(noble_kernel::authority::WitnessSnapshot {
            claim: retained.claim(),
            state: noble_kernel::authority::WitnessState::Live
        }),
        "the refused admission spends nothing"
    );
    Ok(())
}

#[test]
fn finishing_the_invocation_between_admission_and_start_stops_the_start() -> Result<(), String> {
    let mut fixture = super::support::fixture()?;
    let witness = super::support::authorized(&mut fixture)?;
    let claim = witness.claim();
    let execution = match admit_witness(&mut fixture, witness) {
        noble_kernel::authority::Admission::Committed(execution) => execution,
        other => return Err(std::format!("admission: {other:?}")),
    };
    assert_eq!(fixture.table.counters().protected_operations, 0);
    assert_eq!(
        fixture.table.cancel_invocation(),
        Ok(noble_kernel::authority::InvocationOutcome::Cancelled)
    );
    assert_eq!(
        fixture.table.start_execution(execution).map(|_| ()),
        Err(noble_kernel::authority::BoundaryError::InvocationFinished),
        "a finished invocation cannot start a new protected operation"
    );
    assert_eq!(
        fixture.table.counters().protected_operations,
        0,
        "no protected operation ran"
    );
    // Cancellation never restores the consumed witness, and nothing it charged is refunded.
    assert_eq!(
        fixture.table.witness_snapshot(claim),
        Ok(noble_kernel::authority::WitnessSnapshot {
            claim,
            state: noble_kernel::authority::WitnessState::Consumed,
        })
    );
    assert_eq!(fixture.table.counters().witness_consumptions, 1);
    assert_eq!(fixture.table.counters().attempts_admitted, 1);
    Ok(())
}

#[test]
fn revocation_between_authorization_and_admission_refuses_the_admission() -> Result<(), String> {
    // Authorization saw valid, unrevoked facts. The trusted host then reports revocation before the
    // serialized commit, and admission re-checks the facts rather than trusting the earlier decision.
    let mut fixture = super::support::fixture()?;
    let witness = super::support::authorized(&mut fixture)?;
    fixture.facts.revoked = Some(true);
    let retained = match admit_witness(&mut fixture, witness) {
        noble_kernel::authority::Admission::Denied { witness, rejection } => {
            assert_eq!(
                rejection.receipt().description().claim,
                noble_kernel::authority::ReceiptClaim::Denial
            );
            assert_eq!(
                rejection.receipt().description().denial,
                Some(noble_kernel::authority::Denial::Revoked)
            );
            witness.ok_or("a denial returns the original obligation")?
        }
        other => return Err(std::format!("expected a revocation denial: {other:?}")),
    };
    assert_eq!(fixture.table.counters().attempts_admitted, 0);
    assert_eq!(fixture.table.counters().witness_consumptions, 0);
    assert_eq!(fixture.table.counters().protected_operations, 0);
    assert_eq!(fixture.table.retire_witness(retained), Ok(()));
    Ok(())
}
