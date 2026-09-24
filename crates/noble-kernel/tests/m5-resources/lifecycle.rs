#[test]
fn ra_case_01_both_normal_results_return_the_same_owner_once() -> Result<(), String> {
    for outcome in [
        noble_kernel::resources::Completion::Success,
        noble_kernel::resources::Completion::DomainError,
    ] {
        let mut table = crate::support::table()?;
        let initial = crate::support::accepted(
            table.register(crate::support::required(crate::support::SENDER)),
        )?;
        let handle = initial.handle();
        let admitted = crate::support::accepted(
            table.begin(initial, crate::support::required(crate::support::SENDER)),
        )?;
        assert_eq!(admitted.decision.accounting().pins_acquired, 1);
        assert_eq!(table.observation().native_pins, 1);
        let scope = admitted.borrow.scope();
        let completed = crate::support::accepted(table.complete(scope, outcome))?;
        assert_eq!(completed.decision.accounting().owners_returned, 1);
        assert_eq!(completed.decision.accounting().pins_released, 1);
        assert_eq!(completed.decision.accounting().local_releases, 0);
        let returned = crate::support::owner(completed)?;
        assert_eq!(returned.handle(), handle);
        assert_eq!(table.observation().native_pins, 0);
        assert_eq!(
            crate::support::accepted(
                table.validate(handle, crate::support::required(crate::support::SENDER))
            )?
            .state,
            noble_kernel::resources::State::Live
        );
        let duplicate = crate::support::accepted(table.complete(scope, outcome))?;
        assert!(duplicate.owner.is_none());
        crate::support::no_accounting(duplicate.decision);
        assert_eq!(
            crate::support::accepted(
                table.release(returned, crate::support::required(crate::support::SENDER))
            )?
            .accounting()
            .local_releases,
            1
        );
    }
    Ok(())
}

#[test]
fn ra_case_03_busy_reentry_rejects_read_release_transfer_and_borrow() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let handle = initial.handle();
    let admitted = crate::support::accepted(
        table.begin(initial, crate::support::required(crate::support::SENDER)),
    )?;
    let scope = admitted.borrow.scope();
    let before = crate::support::accepted(table.native_access(&admitted.borrow))?;
    let events = [
        noble_kernel::resources::Event::Inspect(crate::support::required(crate::support::SENDER)),
        noble_kernel::resources::Event::Release(crate::support::required(crate::support::SENDER)),
        noble_kernel::resources::Event::Transfer {
            required: crate::support::required(crate::support::SENDER),
            receiver: crate::support::RECEIVER,
            generation: 2,
        },
        noble_kernel::resources::Event::Begin {
            required: crate::support::required(crate::support::SENDER),
            serial: 2,
        },
    ];
    for event in events {
        assert_eq!(
            noble_kernel::resources::transition(before, handle, event),
            Err(noble_kernel::resources::Error::Busy)
        );
    }
    assert_eq!(
        table.validate(handle, crate::support::required(crate::support::SENDER)),
        Err(noble_kernel::resources::Error::Busy)
    );
    assert_eq!(table.snapshot(handle.slot), Some(before));
    assert_eq!(table.observation().native_pins, 1);
    let returned = crate::support::owner(crate::support::accepted(
        table.complete(scope, noble_kernel::resources::Completion::Success),
    )?)?;
    assert_eq!(
        crate::support::accepted(
            table.release(returned, crate::support::required(crate::support::SENDER))
        )?
        .accounting()
        .local_releases,
        1
    );
    Ok(())
}
