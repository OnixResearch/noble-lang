#[path = "controls/ownership.rs"]
mod ownership;

#[path = "controls/foreign.rs"]
mod foreign;

fn settle_retired_member<T>(
    scope: &mut super::manager::Scope<T>,
    table: &mut noble_kernel::async_tasks::Table,
    member: super::manager::MemberRef,
) -> Result<(), String> {
    scope
        .native_stopped(table, member)
        .map_err(|error| format!("{error:?}"))?;
    let pin_mask = 3_u64;
    let settled_pin_count =
        usize::try_from(pin_mask.count_ones()).map_err(|_| "pin count exceeds usize")?;
    let outstanding_before_settlement = table.observation().outstanding_pins;
    assert!(outstanding_before_settlement >= settled_pin_count);
    scope
        .settle_pins(table, member, pin_mask)
        .map_err(|error| format!("{error:?}"))?;
    let expected_remaining = outstanding_before_settlement
        .checked_sub(settled_pin_count)
        .ok_or("settlement exceeded outstanding pins")?;
    assert_eq!(table.observation().outstanding_pins, expected_remaining);
    scope
        .cleanup(
            table,
            member,
            noble_kernel::async_tasks::Obligations {
                inputs: 5,
                results: 3,
                buffers: 7,
            },
        )
        .map_err(|error| format!("{error:?}"))?;
    scope
        .finish(table, member)
        .map_err(|error| format!("{error:?}"))?;
    scope
        .release(member)
        .map_err(|error| format!("{error:?}"))?;
    Ok(())
}

#[test]
fn stale_member_identity_cannot_mutate_a_reused_table_slot() -> Result<(), String> {
    let mut table = super::support::table(1)?;
    let registry = super::registry::ScopeRegistry::shared();
    let mut scope = super::manager::Scope::new(
        super::manager::ScopeId(115),
        noble_kernel::async_tasks::TableId(3),
        registry,
    );
    let old = super::scope_support::admit(&mut scope, &mut table, 1_u8)?;
    scope
        .complete(
            &mut table,
            old,
            noble_kernel::async_tasks::Outcome::Success,
            super::support::completion(),
            1,
        )
        .map_err(|error| format!("{error:?}"))?;
    scope
        .deliver(&mut table, old, super::support::request().context)
        .map_err(|error| format!("{error:?}"))?;
    super::scope_support::settle_delivered(&mut table, &mut scope, old)?;
    let new = super::scope_support::admit(&mut scope, &mut table, 2_u8)?;
    assert_eq!(old.handle.slot, new.handle.slot);
    assert_ne!(old.handle.generation, new.handle.generation);
    let retained = table.snapshot(new.handle.slot);
    assert_eq!(
        scope.complete(
            &mut table,
            old,
            noble_kernel::async_tasks::Outcome::Success,
            super::support::completion(),
            3
        ),
        Err(super::manager::OperationError::Table(
            noble_kernel::async_tasks::Error::WrongGeneration
        ))
    );
    assert_eq!(table.snapshot(new.handle.slot), retained);
    Ok(())
}

#[test]
fn the_first_host_retirement_failure_survives_later_failure_and_completion() -> Result<(), String> {
    let mut table = super::support::table(2)?;
    let registry = super::registry::ScopeRegistry::shared();
    let mut scope = super::manager::Scope::new(
        super::manager::ScopeId(115),
        noble_kernel::async_tasks::TableId(3),
        registry,
    );
    let first = super::scope_support::admit(&mut scope, &mut table, 1_u8)?;
    let second = super::scope_support::admit(&mut scope, &mut table, 2_u8)?;
    scope
        .retire(
            &mut table,
            first,
            noble_kernel::async_tasks::Failure::Deadline,
        )
        .map_err(|error| format!("{error:?}"))?;
    scope
        .retire(&mut table, second, noble_kernel::async_tasks::Failure::Trap)
        .map_err(|error| format!("{error:?}"))?;
    let late_first = scope
        .complete(
            &mut table,
            first,
            noble_kernel::async_tasks::Outcome::Success,
            super::support::completion(),
            1,
        )
        .map_err(|error| format!("{error:?}"))?;
    let late_second = scope
        .complete(
            &mut table,
            second,
            noble_kernel::async_tasks::Outcome::DomainError,
            super::support::completion(),
            2,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        late_first.record.failure,
        Some(noble_kernel::async_tasks::Failure::Deadline)
    );
    assert_eq!(
        late_second.record.failure,
        Some(noble_kernel::async_tasks::Failure::Trap)
    );
    assert_eq!(
        scope.primary_failure(),
        Some(noble_kernel::async_tasks::Failure::Deadline)
    );
    assert!(scope.results().is_empty());

    for member in [first, second] {
        settle_retired_member(&mut scope, &mut table, member)?;
    }
    scope.close().map_err(|error| format!("{error:?}"))?;
    assert_eq!(table.observation().retired, 2);
    assert_eq!(table.observation().outstanding_pins, 0);
    Ok(())
}

#[test]
fn the_two_child_bound_and_post_close_refusal_preserve_inputs() -> Result<(), String> {
    let mut table = super::support::table(3)?;
    let registry = super::registry::ScopeRegistry::shared();
    let mut scope = super::manager::Scope::new(
        super::manager::ScopeId(115),
        noble_kernel::async_tasks::TableId(3),
        registry,
    );
    let first = super::scope_support::admit(&mut scope, &mut table, 1_u8)?;
    let second = super::scope_support::admit(&mut scope, &mut table, 2_u8)?;
    let before_full = table.observation();
    match scope.admit(&mut table, super::support::request(), 3_u8) {
        Err(super::manager::AdmissionError::Scope { error, input }) => {
            assert_eq!(error, super::manager::ScopeError::Full);
            assert_eq!(input, 3);
        }
        Err(super::manager::AdmissionError::Table(rejected)) => {
            return Err(format!("unexpected table refusal: {:?}", rejected.error));
        }
        Ok(_) => return Err("scope admitted a third child".to_owned()),
    }
    assert_eq!(scope.members(), vec![first, second]);
    assert_eq!(table.observation(), before_full);

    let mut closed = super::manager::Scope::new(
        super::manager::ScopeId(116),
        noble_kernel::async_tasks::TableId(3),
        super::registry::ScopeRegistry::shared(),
    );
    closed.close().map_err(|error| format!("{error:?}"))?;
    let before_closed = table.observation();
    match closed.admit(&mut table, super::support::request(), 4_u8) {
        Err(super::manager::AdmissionError::Scope { error, input }) => {
            assert_eq!(error, super::manager::ScopeError::Closed);
            assert_eq!(input, 4);
        }
        Err(super::manager::AdmissionError::Table(rejected)) => {
            return Err(format!("unexpected table refusal: {:?}", rejected.error));
        }
        Ok(_) => return Err("closed scope admitted a child".to_owned()),
    }
    assert_eq!(table.observation(), before_closed);
    Ok(())
}
