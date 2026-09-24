#[test]
fn incorrect_callback_bindings_do_not_release_a_pin_or_return_ownership() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let admitted = crate::support::accepted(
        table.begin(initial, crate::support::required(crate::support::SENDER)),
    )?;
    let scope = admitted.borrow.scope();
    let callbacks = crate::support::invalid_callbacks(scope);
    for callback in callbacks {
        assert!(table
            .complete(callback, noble_kernel::resources::Completion::Success)
            .is_err());
        assert!(table
            .revoke(callback, noble_kernel::resources::Retirement::Trap)
            .is_err());
        assert_eq!(table.observation().native_pins, 1);
        assert_eq!(table.observation().busy, 1);
    }
    let returned = crate::support::owner_from_scope(&mut table, scope)?;
    crate::support::accepted(
        table.release(returned, crate::support::required(crate::support::SENDER)),
    )?;
    Ok(())
}

#[test]
fn an_old_callback_cannot_complete_or_cancel_a_newer_scope() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let first = crate::support::accepted(
        table.begin(initial, crate::support::required(crate::support::SENDER)),
    )?;
    let old_scope = first.borrow.scope();
    let returned = crate::support::owner_from_scope(&mut table, old_scope)?;
    let second = crate::support::accepted(
        table.begin(returned, crate::support::required(crate::support::SENDER)),
    )?;
    let current_scope = second.borrow.scope();
    assert_ne!(old_scope.serial, current_scope.serial);
    assert_eq!(
        crate::support::rejected(
            table.complete(old_scope, noble_kernel::resources::Completion::Success)
        )?,
        noble_kernel::resources::Error::WrongScope
    );
    assert_eq!(
        table.revoke(old_scope, noble_kernel::resources::Retirement::Cancelled),
        Err(noble_kernel::resources::Error::WrongScope)
    );
    assert_eq!(
        table.native_access(&first.borrow),
        Err(noble_kernel::resources::Error::WrongScope)
    );
    assert_eq!(table.observation().busy, 1);
    assert_eq!(table.observation().native_pins, 1);
    let returned = crate::support::owner_from_scope(&mut table, current_scope)?;
    crate::support::accepted(
        table.release(returned, crate::support::required(crate::support::SENDER)),
    )?;
    Ok(())
}

#[test]
fn completion_winning_before_cancellation_keeps_the_returned_owner_live() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let handle = initial.handle();
    let admitted = crate::support::accepted(
        table.begin(initial, crate::support::required(crate::support::SENDER)),
    )?;
    let scope = admitted.borrow.scope();
    let returned = crate::support::owner_from_scope(&mut table, scope)?;
    crate::support::no_accounting(crate::support::accepted(
        table.revoke(scope, noble_kernel::resources::Retirement::Cancelled),
    )?);
    assert_eq!(
        crate::support::accepted(
            table.validate(handle, crate::support::required(crate::support::SENDER))
        )?
        .state,
        noble_kernel::resources::State::Live
    );
    assert_eq!(table.observation().native_pins, 0);
    crate::support::accepted(
        table.release(returned, crate::support::required(crate::support::SENDER)),
    )?;
    Ok(())
}

#[test]
fn slot_reuse_rejects_old_generations_without_releasing_the_replacement() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let old_handle = initial.handle();
    let admitted = crate::support::accepted(
        table.begin(initial, crate::support::required(crate::support::SENDER)),
    )?;
    let old_scope = admitted.borrow.scope();
    crate::support::accepted(
        table.revoke(old_scope, noble_kernel::resources::Retirement::Cancelled),
    )?;
    crate::support::accepted(
        table.complete(old_scope, noble_kernel::resources::Completion::Success),
    )?;
    let replacement =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let handle = replacement.handle();
    assert_eq!(handle.slot, old_handle.slot);
    assert_ne!(handle.generation, old_handle.generation);
    assert_eq!(
        table.validate(old_handle, crate::support::required(crate::support::SENDER)),
        Err(noble_kernel::resources::Error::WrongGeneration)
    );
    assert_eq!(
        crate::support::rejected(
            table.complete(old_scope, noble_kernel::resources::Completion::Success)
        )?,
        noble_kernel::resources::Error::WrongGeneration
    );
    assert_eq!(
        table.retire(old_handle, noble_kernel::resources::Retirement::Trap),
        Err(noble_kernel::resources::Error::WrongGeneration)
    );
    assert_eq!(
        crate::support::accepted(
            table.validate(handle, crate::support::required(crate::support::SENDER))
        )?
        .state,
        noble_kernel::resources::State::Live
    );
    assert_eq!(
        crate::support::accepted(table.release(
            replacement,
            crate::support::required(crate::support::SENDER)
        ))?
        .accounting()
        .local_releases,
        1
    );
    Ok(())
}

#[test]
fn required_kind_context_and_rights_are_independent_of_claimed_handle_fields() -> Result<(), String>
{
    let mut table = crate::support::table()?;
    let mut live =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let handle = live.handle();
    let invalid = crate::support::invalid_requirements();
    for (requirement, error) in invalid {
        let rejection = crate::support::rejected(table.begin(live, requirement))?;
        assert_eq!(rejection.error, error);
        live = rejection.input;
        assert_eq!(
            crate::support::accepted(
                table.validate(handle, crate::support::required(crate::support::SENDER))
            )?
            .state,
            noble_kernel::resources::State::Live
        );
        assert_eq!(table.observation().native_pins, 0);
    }
    crate::support::accepted(
        table.release(live, crate::support::required(crate::support::SENDER)),
    )?;
    Ok(())
}

#[test]
fn an_opaque_owner_from_a_different_table_cannot_admit_work() -> Result<(), String> {
    let mut first = crate::support::table()?;
    let mut second = crate::support::accepted(noble_kernel::resources::Table::new(
        noble_kernel::resources::TableId(2),
        crate::support::limits(),
    ))?;
    let first_owner =
        crate::support::accepted(first.register(crate::support::required(crate::support::SENDER)))?;
    let second_owner = crate::support::accepted(
        second.register(crate::support::required(crate::support::SENDER)),
    )?;
    let rejection = crate::support::rejected(second.begin(
        first_owner,
        crate::support::required(crate::support::SENDER),
    ))?;
    assert_eq!(
        rejection.error,
        noble_kernel::resources::Error::InvalidHandle
    );
    assert_eq!(second.observation().native_pins, 0);
    assert_eq!(first.observation().live, 1);
    let admitted = crate::support::accepted(first.begin(
        rejection.input,
        crate::support::required(crate::support::SENDER),
    ))?;
    let returned = crate::support::owner_from_scope(&mut first, admitted.borrow.scope())?;
    crate::support::accepted(
        first.release(returned, crate::support::required(crate::support::SENDER)),
    )?;
    crate::support::accepted(second.release(
        second_owner,
        crate::support::required(crate::support::SENDER),
    ))?;
    Ok(())
}

#[test]
fn semantic_generation_and_scope_edges_never_wrap_to_zero() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let observed = crate::support::accepted(table.validate(
        initial.handle(),
        crate::support::required(crate::support::SENDER),
    ))?;
    let at_end = noble_kernel::resources::Snapshot {
        handle: noble_kernel::resources::Handle {
            generation: u64::MAX,
            ..observed.handle
        },
        last_scope: Some(u64::MAX),
        ..observed
    };
    assert_eq!(
        noble_kernel::resources::transition(
            at_end,
            at_end.handle,
            noble_kernel::resources::Event::Transfer {
                required: crate::support::required(crate::support::SENDER),
                receiver: crate::support::RECEIVER,
                generation: 0,
            }
        ),
        Err(noble_kernel::resources::Error::WrongGeneration)
    );
    assert_eq!(
        noble_kernel::resources::transition(
            at_end,
            at_end.handle,
            noble_kernel::resources::Event::Begin {
                required: crate::support::required(crate::support::SENDER),
                serial: 0,
            }
        ),
        Err(noble_kernel::resources::Error::WrongScope)
    );
    assert_eq!(
        noble_kernel::resources::transition(
            at_end,
            at_end.handle,
            noble_kernel::resources::Event::Begin {
                required: crate::support::required(crate::support::SENDER),
                serial: u64::MAX,
            }
        ),
        Err(noble_kernel::resources::Error::WrongScope)
    );
    assert_eq!(
        crate::support::accepted(
            table.release(initial, crate::support::required(crate::support::SENDER))
        )?
        .accounting()
        .local_releases,
        1
    );
    Ok(())
}
