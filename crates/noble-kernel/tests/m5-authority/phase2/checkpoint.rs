//! Checkpoint transitions preserve consumed authority and carried quota reservations.

#[test]
fn advance_invalidates_stale_facts_without_restoring_authority() -> Result<(), String> {
    // A stale checkpoint is refused as stale facts rather than as capacity, and the refusal neither
    // resurrects the consumed witness nor adds a protected execution. The fresh snapshot then has to
    // carry the prior reservation forward: the same quota that admitted once admits nothing more.
    let mut fixture = super::super::support::fixture()?;
    super::quote(&mut fixture.facts, super::UNITS);
    let (first, second) = super::two_witnesses(&mut fixture)?;
    let consumed = first.claim();
    assert!(matches!(
        super::admit_witness(&mut fixture, first),
        noble_kernel::authority::Admission::Committed(_)
    ));
    assert_eq!(
        fixture
            .table
            .advance_from_trusted_host(noble_kernel::authority::Checkpoint {
                sequence: 4,
                now: 51,
            }),
        Ok(()),
        "a monotone checkpoint is accepted"
    );
    // Reusing the pre-advance checkpoint is stale facts.
    let mut backwards = super::super::support::fixture()?;
    assert_eq!(
        backwards
            .table
            .advance_from_trusted_host(noble_kernel::authority::Checkpoint {
                sequence: 3,
                now: 50,
            }),
        Err(noble_kernel::authority::SetupError::StaleCheckpoint),
        "a repeated sequence is refused by the table"
    );
    let denied = match super::admit_witness(&mut fixture, second) {
        noble_kernel::authority::Admission::Denied { witness, rejection } => {
            assert_eq!(
                rejection.receipt().description().denial,
                Some(noble_kernel::authority::Denial::StaleFacts),
                "a pre-advance snapshot is stale facts, not a quota denial"
            );
            witness.ok_or("a denial returns the original obligation")?
        }
        other => return Err(std::format!("expected a stale-facts denial: {other:?}")),
    };
    assert_eq!(
        fixture.table.witness_snapshot(consumed),
        Ok(noble_kernel::authority::WitnessSnapshot {
            claim: consumed,
            state: noble_kernel::authority::WitnessState::Consumed
        }),
        "the stale denial does not resurrect the consumed witness"
    );
    assert_eq!(
        fixture.table.counters().protected_operations,
        0,
        "a stale denial adds no protected execution: only an approved observation counts one"
    );
    assert_eq!(
        fixture.table.counters().attempts_admitted,
        1,
        "no attempt is admitted by a stale denial"
    );

    refuse_carried_reservation(fixture, denied)
}

fn refuse_carried_reservation(
    mut fixture: super::super::support::Fixture,
    denied: noble_kernel::authority::Witness,
) -> Result<(), String> {
    // The fresh facts carry the reservation forward, so the same quota admits nothing more.
    fixture.facts.checkpoint = noble_kernel::authority::Checkpoint {
        sequence: 4,
        now: 51,
    };
    super::quote(&mut fixture.facts, 0);
    let still_denied = match super::admit_witness(&mut fixture, denied) {
        noble_kernel::authority::Admission::Denied { witness, rejection } => {
            let denial = rejection.receipt().description().denial;
            assert!(
                matches!(
                    denial,
                    Some(noble_kernel::authority::Denial::QuotaExhausted)
                        | Some(noble_kernel::authority::Denial::InconsistentQuota)
                ),
                "a carried-forward snapshot has no capacity left: {denial:?}"
            );
            witness.ok_or("a denial returns the original obligation")?
        }
        other => return Err(std::format!("expected a quota denial: {other:?}")),
    };
    assert_eq!(
        fixture.table.witness_snapshot(still_denied.claim()),
        Ok(noble_kernel::authority::WitnessSnapshot {
            claim: still_denied.claim(),
            state: noble_kernel::authority::WitnessState::Live
        }),
        "the quota denial spends nothing either"
    );
    Ok(())
}
