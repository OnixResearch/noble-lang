#[test]
fn ra_case_05_cancelled_unstoppable_work_keeps_its_pin_after_tokens_are_dropped(
) -> Result<(), String> {
    let mut table = crate::support::table()?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let handle = initial.handle();
    let admitted = crate::support::accepted(
        table.begin(initial, crate::support::required(crate::support::SENDER)),
    )?;
    let scope = admitted.borrow.scope();
    let revoked = crate::support::accepted(
        table.revoke(scope, noble_kernel::resources::Retirement::Cancelled),
    )?;
    assert_eq!(
        revoked.record.state,
        noble_kernel::resources::State::Retiring(scope.serial)
    );
    assert_eq!(revoked.accounting().local_releases, 0);
    assert_eq!(revoked.accounting().pins_released, 0);
    assert_eq!(
        table.native_access(&admitted.borrow),
        Err(noble_kernel::resources::Error::Retiring)
    );
    {
        let _discarded = admitted.borrow;
    }
    assert_eq!(
        table.validate(handle, crate::support::required(crate::support::SENDER)),
        Err(noble_kernel::resources::Error::Retiring)
    );
    assert_eq!(table.observation().retiring, 1);
    assert_eq!(table.observation().native_pins, 1);
    let repeated =
        crate::support::accepted(table.retire(handle, noble_kernel::resources::Retirement::Trap))?;
    crate::support::no_accounting(repeated);
    assert_eq!(
        repeated.record.retirement,
        Some(noble_kernel::resources::Retirement::Cancelled)
    );
    Ok(())
}

#[test]
fn ra_case_06_late_completion_retires_once_without_returning_an_owner() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let handle = initial.handle();
    let admitted = crate::support::accepted(
        table.begin(initial, crate::support::required(crate::support::SENDER)),
    )?;
    let scope = admitted.borrow.scope();
    crate::support::accepted(table.revoke(scope, noble_kernel::resources::Retirement::Cancelled))?;
    let completed = crate::support::accepted(
        table.complete(scope, noble_kernel::resources::Completion::Success),
    )?;
    assert!(completed.owner.is_none());
    assert_eq!(
        completed.decision.record.state,
        noble_kernel::resources::State::Retired
    );
    assert_eq!(completed.decision.accounting().pins_released, 1);
    assert_eq!(completed.decision.accounting().local_releases, 1);
    let duplicate = crate::support::accepted(
        table.complete(scope, noble_kernel::resources::Completion::DomainError),
    )?;
    assert!(duplicate.owner.is_none());
    crate::support::no_accounting(duplicate.decision);
    crate::support::no_accounting(crate::support::accepted(
        table.retire(handle, noble_kernel::resources::Retirement::HostCleanup),
    )?);
    assert_eq!(table.observation().native_pins, 0);
    assert_eq!(
        table.validate(handle, crate::support::required(crate::support::SENDER)),
        Err(noble_kernel::resources::Error::Retired)
    );
    Ok(())
}

#[test]
fn ra_case_09_unexpected_sync_suspension_revokes_access_but_not_the_pin() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let initial =
        crate::support::accepted(table.register(crate::support::required(crate::support::SENDER)))?;
    let admitted = crate::support::accepted(
        table.begin(initial, crate::support::required(crate::support::SENDER)),
    )?;
    let scope = admitted.borrow.scope();
    let decision = crate::support::accepted(table.revoke(
        scope,
        noble_kernel::resources::Retirement::UnexpectedSuspension,
    ))?;
    assert_eq!(
        decision.record.state,
        noble_kernel::resources::State::Retiring(scope.serial)
    );
    assert_eq!(
        decision.record.retirement,
        Some(noble_kernel::resources::Retirement::UnexpectedSuspension)
    );
    assert_eq!(
        table.native_access(&admitted.borrow),
        Err(noble_kernel::resources::Error::Retiring)
    );
    assert_eq!(table.observation().native_pins, 1);
    let completed = crate::support::accepted(
        table.complete(scope, noble_kernel::resources::Completion::DomainError),
    )?;
    assert!(completed.owner.is_none());
    assert_eq!(completed.decision.accounting().local_releases, 1);
    Ok(())
}
