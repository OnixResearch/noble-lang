#[test]
fn ra_case_07_invalid_second_resource_argument_preserves_all_sender_owners() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let first =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let second =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let handles = [first.handle(), second.handle()];
    let mut wrong_kind = crate::support::required(crate::support::SENDER);
    wrong_kind.kind = noble_kernel::types::ResourceKind(8);
    let rejection = crate::support::rejected(table.transfer(
        vec![first, second],
        &[crate::support::required(crate::support::SENDER), wrong_kind],
        crate::support::RECEIVER,
    ))?;
    assert_eq!(rejection.error, noble_kernel::resources::Error::WrongKind);
    assert_eq!(table.observation().live, 2);
    assert_eq!(table.observation().native_pins, 0);
    for handle in handles {
        assert_eq!(
            crate::support::accepted(
                table.validate(handle, crate::support::required(crate::support::SENDER))
            )?
            .state,
            noble_kernel::resources::State::Live
        );
    }
    for owner in rejection.input {
        let borrowed = crate::support::accepted(
            table.begin(owner, crate::support::required(crate::support::SENDER)),
        )?;
        let returned = crate::support::owner_from_scope(&mut table, borrowed.borrow.scope())?;
        crate::support::accepted(
            table.release(returned, crate::support::required(crate::support::SENDER)),
        )?;
    }
    assert_eq!(table.observation().retired, 2);
    Ok(())
}

#[test]
fn ra_case_08_domain_error_keeps_transfer_cleanup_with_receiver() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let sender_handle = initial.handle();
    let transferred = crate::support::accepted(table.transfer(
        vec![initial],
        &[crate::support::required(crate::support::SENDER)],
        crate::support::RECEIVER,
    ))?;
    assert_eq!(transferred.decisions[0].accounting().owners_transferred, 1);
    assert_eq!(
        table.validate(
            sender_handle,
            crate::support::required(crate::support::SENDER)
        ),
        Err(noble_kernel::resources::Error::WrongContext)
    );
    let receiver_owner = crate::support::accepted(
        transferred
            .owners
            .into_iter()
            .next()
            .ok_or("missing receiver owner"),
    )?;
    let receiver_handle = receiver_owner.handle();
    let admitted = crate::support::accepted(table.begin(
        receiver_owner,
        crate::support::required(crate::support::RECEIVER),
    ))?;
    let returned = crate::support::owner(crate::support::accepted(table.complete(
        admitted.borrow.scope(),
        noble_kernel::resources::Completion::DomainError,
    ))?)?;
    assert_eq!(returned.handle(), receiver_handle);
    assert_eq!(
        table.validate(
            sender_handle,
            crate::support::required(crate::support::SENDER)
        ),
        Err(noble_kernel::resources::Error::WrongContext)
    );
    {
        let _discarded = returned;
    }
    let sender_cleanup = crate::support::accepted(table.retire_context(
        crate::support::SENDER,
        noble_kernel::resources::Retirement::Trap,
    ))?;
    assert!(sender_cleanup.is_empty());
    let receiver_cleanup = crate::support::accepted(table.retire_context(
        crate::support::RECEIVER,
        noble_kernel::resources::Retirement::Trap,
    ))?;
    assert_eq!(receiver_cleanup[0].accounting().local_releases, 1);
    assert_eq!(table.observation().retired, 1);
    Ok(())
}

#[test]
fn wi_06_every_handle_binding_is_checked_before_guest_work() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let handle = initial.handle();
    let mutations = crate::support::invalid_claims(handle);
    for (claim, error) in mutations {
        assert_eq!(
            table.validate(claim, crate::support::required(crate::support::SENDER)),
            Err(error)
        );
    }
    assert_eq!(table.observation().live, 1);
    assert_eq!(table.observation().native_pins, 0);
    let admitted = crate::support::accepted(
        table.begin(initial, crate::support::required(crate::support::SENDER)),
    )?;
    assert_eq!(table.observation().native_pins, 1);
    let returned = crate::support::owner_from_scope(&mut table, admitted.borrow.scope())?;
    crate::support::accepted(
        table.release(returned, crate::support::required(crate::support::SENDER)),
    )?;
    Ok(())
}
