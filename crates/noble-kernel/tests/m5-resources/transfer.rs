#[test]
fn trapped_context_cleanup_finds_dropped_live_owners_and_pending_native_work() -> Result<(), String>
{
    let mut table = crate::support::table()?;
    let dropped =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let pending =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let unaffected = crate::support::accepted(
        table.register(crate::support::required(crate::support::RECEIVER)),
    )?;
    {
        let _discarded = dropped;
    }
    let admitted = crate::support::accepted(
        table.begin(pending, crate::support::required(crate::support::SENDER)),
    )?;
    let decisions = crate::support::accepted(table.retire_context(
        crate::support::SENDER,
        noble_kernel::resources::Retirement::Trap,
    ))?;
    assert_eq!(decisions.len(), 2);
    assert_eq!(decisions[0].accounting().local_releases, 1);
    assert_eq!(
        decisions[1].record.state,
        noble_kernel::resources::State::Retiring(admitted.borrow.scope().serial)
    );
    assert_eq!(table.observation().native_pins, 1);
    assert_eq!(table.observation().live, 1);
    let repeated = crate::support::accepted(table.retire_context(
        crate::support::SENDER,
        noble_kernel::resources::Retirement::Cancelled,
    ))?;
    assert_eq!(repeated.len(), 1);
    crate::support::no_accounting(repeated[0]);
    assert_eq!(
        repeated[0].record.retirement,
        Some(noble_kernel::resources::Retirement::Trap)
    );
    let completion = crate::support::accepted(table.complete(
        admitted.borrow.scope(),
        noble_kernel::resources::Completion::Success,
    ))?;
    assert!(completion.owner.is_none());
    assert_eq!(completion.decision.accounting().local_releases, 1);
    crate::support::accepted(table.release(
        unaffected,
        crate::support::required(crate::support::RECEIVER),
    ))?;
    Ok(())
}

#[test]
fn a_retiring_slot_is_not_reusable_and_full_tables_reject_registration() -> Result<(), String> {
    let mut policy = crate::support::limits();
    policy.slots = 1;
    policy.pins = 1;
    policy.owners_per_context = 1;
    let mut table = crate::support::accepted(noble_kernel::resources::Table::new(
        noble_kernel::resources::TableId(1),
        policy,
    ))?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    assert_eq!(
        crate::support::rejected(
            table.register(crate::support::required(crate::support::RECEIVER))
        )?,
        noble_kernel::resources::Error::Capacity
    );
    let admitted = crate::support::accepted(
        table.begin(initial, crate::support::required(crate::support::SENDER)),
    )?;
    crate::support::accepted(table.revoke(
        admitted.borrow.scope(),
        noble_kernel::resources::Retirement::Cancelled,
    ))?;
    assert_eq!(
        crate::support::rejected(
            table.register(crate::support::required(crate::support::RECEIVER))
        )?,
        noble_kernel::resources::Error::Capacity
    );
    crate::support::accepted(table.complete(
        admitted.borrow.scope(),
        noble_kernel::resources::Completion::DomainError,
    ))?;
    let replacement = crate::support::accepted(
        table.register(crate::support::required(crate::support::RECEIVER)),
    )?;
    assert_eq!(
        crate::support::accepted(table.validate(
            replacement.handle(),
            crate::support::required(crate::support::RECEIVER)
        ))?
        .state,
        noble_kernel::resources::State::Live
    );
    crate::support::accepted(table.release(
        replacement,
        crate::support::required(crate::support::RECEIVER),
    ))?;
    Ok(())
}

#[test]
fn only_an_explicit_owned_result_can_return_to_the_sender() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let old_sender = initial.handle();
    let transferred = crate::support::accepted(table.transfer(
        vec![initial],
        &[crate::support::required(crate::support::SENDER)],
        crate::support::RECEIVER,
    ))?;
    let old_receiver = transferred.owners[0].handle();
    let returned = crate::support::accepted(table.transfer(
        transferred.owners,
        &[crate::support::required(crate::support::RECEIVER)],
        crate::support::SENDER,
    ))?;
    let new_sender = returned.owners[0].handle();
    assert_eq!(new_sender.context, crate::support::SENDER);
    assert_eq!(
        table.validate(old_sender, crate::support::required(crate::support::SENDER)),
        Err(noble_kernel::resources::Error::WrongGeneration)
    );
    assert_eq!(
        table.validate(
            old_receiver,
            crate::support::required(crate::support::RECEIVER)
        ),
        Err(noble_kernel::resources::Error::WrongContext)
    );
    for owner in returned.owners {
        crate::support::accepted(
            table.release(owner, crate::support::required(crate::support::SENDER)),
        )?;
    }
    Ok(())
}

#[test]
fn same_context_invalidates_sender_generation_without_double_charging_quota() -> Result<(), String>
{
    let mut policy = crate::support::limits();
    policy.owners_per_context = 1;
    let mut table = crate::support::accepted(noble_kernel::resources::Table::new(
        noble_kernel::resources::TableId(1),
        policy,
    ))?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let old = initial.handle();
    let transferred = crate::support::accepted(table.transfer(
        vec![initial],
        &[crate::support::required(crate::support::SENDER)],
        crate::support::SENDER,
    ))?;
    assert_eq!(table.observation().live, 1);
    assert_eq!(
        table.validate(old, crate::support::required(crate::support::SENDER)),
        Err(noble_kernel::resources::Error::WrongGeneration)
    );
    for owner in transferred.owners {
        crate::support::accepted(
            table.release(owner, crate::support::required(crate::support::SENDER)),
        )?;
    }
    Ok(())
}

#[test]
fn malformed_argument_count_preserves_the_owned_input() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let handle = initial.handle();
    let rejection =
        crate::support::rejected(table.transfer(vec![initial], &[], crate::support::RECEIVER))?;
    assert_eq!(
        rejection.error,
        noble_kernel::resources::Error::ArgumentCount
    );
    assert_eq!(
        crate::support::accepted(
            table.validate(handle, crate::support::required(crate::support::SENDER))
        )?
        .state,
        noble_kernel::resources::State::Live
    );
    let transferred = crate::support::accepted(table.transfer(
        rejection.input,
        &[crate::support::required(crate::support::SENDER)],
        crate::support::RECEIVER,
    ))?;
    for owner in transferred.owners {
        crate::support::accepted(
            table.release(owner, crate::support::required(crate::support::RECEIVER)),
        )?;
    }
    Ok(())
}
