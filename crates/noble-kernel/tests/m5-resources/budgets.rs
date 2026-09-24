#[test]
fn receiver_capacity_preflight_rejects_the_whole_transfer() -> Result<(), String> {
    let mut policy = crate::support::limits();
    policy.owners_per_context = 2;
    let mut table = crate::support::accepted(noble_kernel::resources::Table::new(
        noble_kernel::resources::TableId(1),
        policy,
    ))?;
    let first =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let second =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let resident = crate::support::accepted(
        table.register(crate::support::required(crate::support::RECEIVER)),
    )?;
    let handles = [first.handle(), second.handle()];
    let rejection = crate::support::rejected(table.transfer(
        vec![first, second],
        &[crate::support::required(crate::support::SENDER); 2],
        crate::support::RECEIVER,
    ))?;
    assert_eq!(
        rejection.error,
        noble_kernel::resources::Error::ContextCapacity
    );
    for handle in handles {
        assert_eq!(
            crate::support::accepted(
                table.validate(handle, crate::support::required(crate::support::SENDER))
            )?
            .state,
            noble_kernel::resources::State::Live
        );
    }
    crate::support::accepted(
        table.release(resident, crate::support::required(crate::support::RECEIVER)),
    )?;
    let transferred = crate::support::accepted(table.transfer(
        rejection.input,
        &[crate::support::required(crate::support::SENDER); 2],
        crate::support::RECEIVER,
    ))?;
    assert_eq!(transferred.owners[0].handle().slot, handles[0].slot);
    assert_eq!(transferred.owners[1].handle().slot, handles[1].slot);
    for owner in transferred.owners {
        crate::support::accepted(
            table.release(owner, crate::support::required(crate::support::RECEIVER)),
        )?;
    }
    Ok(())
}

#[test]
fn batch_generation_exhaustion_does_not_partially_transfer_or_spend_a_generation(
) -> Result<(), String> {
    let mut policy = crate::support::limits();
    policy.generations = 3;
    let mut table = crate::support::accepted(noble_kernel::resources::Table::new(
        noble_kernel::resources::TableId(1),
        policy,
    ))?;
    let first =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let second =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let handles = [first.handle(), second.handle()];
    let rejection = crate::support::rejected(table.transfer(
        vec![first, second],
        &[crate::support::required(crate::support::SENDER); 2],
        crate::support::RECEIVER,
    ))?;
    assert_eq!(
        rejection.error,
        noble_kernel::resources::Error::GenerationExhausted
    );
    for handle in handles {
        assert_eq!(
            crate::support::accepted(
                table.validate(handle, crate::support::required(crate::support::SENDER))
            )?
            .state,
            noble_kernel::resources::State::Live
        );
    }
    let final_owner = crate::support::accepted(
        table.register(crate::support::required(crate::support::RECEIVER)),
    )?;
    crate::support::accepted(table.release(
        final_owner,
        crate::support::required(crate::support::RECEIVER),
    ))?;
    assert_eq!(
        crate::support::rejected(
            table.register(crate::support::required(crate::support::RECEIVER))
        )?,
        noble_kernel::resources::Error::GenerationExhausted
    );
    for owner in rejection.input {
        crate::support::accepted(
            table.release(owner, crate::support::required(crate::support::SENDER)),
        )?;
    }
    Ok(())
}

#[test]
fn scope_exhaustion_preserves_unadmitted_owners() -> Result<(), String> {
    let mut policy = crate::support::limits();
    policy.pins = 1;
    policy.scopes = 1;
    let mut table = crate::support::accepted(noble_kernel::resources::Table::new(
        noble_kernel::resources::TableId(1),
        policy,
    ))?;
    let first =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let second =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let first_call = crate::support::accepted(
        table.begin(first, crate::support::required(crate::support::SENDER)),
    )?;
    let rejected_scope = crate::support::rejected(
        table.begin(second, crate::support::required(crate::support::SENDER)),
    )?;
    assert_eq!(
        rejected_scope.error,
        noble_kernel::resources::Error::ScopeExhausted
    );
    let first = crate::support::owner_from_scope(&mut table, first_call.borrow.scope())?;
    assert_eq!(table.observation().live, 2);
    assert_eq!(table.observation().native_pins, 0);
    crate::support::accepted(
        table.release(first, crate::support::required(crate::support::SENDER)),
    )?;
    crate::support::accepted(table.release(
        rejected_scope.input,
        crate::support::required(crate::support::SENDER),
    ))?;
    Ok(())
}

#[test]
fn a_full_pin_budget_is_reusable_only_after_matching_completion() -> Result<(), String> {
    let mut policy = crate::support::limits();
    policy.pins = 1;
    policy.scopes = 2;
    let mut table = crate::support::accepted(noble_kernel::resources::Table::new(
        noble_kernel::resources::TableId(1),
        policy,
    ))?;
    let first =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let second =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let first_call = crate::support::accepted(
        table.begin(first, crate::support::required(crate::support::SENDER)),
    )?;
    crate::support::accepted(table.revoke(
        first_call.borrow.scope(),
        noble_kernel::resources::Retirement::Trap,
    ))?;
    let rejected_pin = crate::support::rejected(
        table.begin(second, crate::support::required(crate::support::SENDER)),
    )?;
    assert_eq!(
        rejected_pin.error,
        noble_kernel::resources::Error::PinCapacity
    );
    assert_eq!(table.observation().retiring, 1);
    crate::support::accepted(table.complete(
        first_call.borrow.scope(),
        noble_kernel::resources::Completion::Success,
    ))?;
    let second_call = crate::support::accepted(table.begin(
        rejected_pin.input,
        crate::support::required(crate::support::SENDER),
    ))?;
    let returned = crate::support::owner_from_scope(&mut table, second_call.borrow.scope())?;
    crate::support::accepted(
        table.release(returned, crate::support::required(crate::support::SENDER)),
    )?;
    assert_eq!(table.observation().native_pins, 0);
    Ok(())
}
