#[test]
fn terminal_slot_reuse_rejects_stale_native_events() -> Result<(), String> {
    let mut bound = crate::support::limits();
    bound.tasks = 1;
    let mut table = crate::support::accepted(noble_kernel::async_tasks::Table::new(
        noble_kernel::async_tasks::TableId(3),
        bound,
    ))?;
    let admitted = crate::support::accepted(table.admit(crate::support::request(), ()))?;
    let old = admitted.callback;
    crate::support::accepted(table.cancel(admitted.task, crate::support::request().context))?;
    assert_eq!(
        table
            .admit(crate::support::request(), 19)
            .err()
            .ok_or("reused pinned task")?
            .input,
        19
    );
    crate::support::accepted(table.native_stopped(old))?;
    let late = crate::support::accepted(table.complete(
        old,
        noble_kernel::async_tasks::Outcome::Success,
        crate::support::completion(),
    ))?;
    assert_eq!(
        late.accounting,
        noble_kernel::async_tasks::Accounting::empty()
    );
    crate::support::accepted(table.settle_pins(old, 3))?;
    crate::support::accepted(table.cleanup(
        old.task,
        noble_kernel::async_tasks::Obligations {
            inputs: 7,
            results: 0,
            buffers: 5,
        },
    ))?;
    assert!(table.admit(crate::support::request(), ()).is_err());
    crate::support::accepted(table.finish(old.task))?;
    let current = crate::support::accepted(table.admit(crate::support::request(), ()))?;
    assert_ne!(current.callback.task.generation, old.task.generation);
    assert_eq!(
        table.complete(
            old,
            noble_kernel::async_tasks::Outcome::Success,
            crate::support::completion()
        ),
        Err(noble_kernel::async_tasks::Error::WrongGeneration)
    );
    assert_eq!(
        table.settle_pins(old, 3),
        Err(noble_kernel::async_tasks::Error::WrongGeneration)
    );
    assert_eq!(
        table.wake(old),
        Err(noble_kernel::async_tasks::Error::WrongGeneration)
    );
    assert_eq!(table.snapshot(0), Some(current.decision.record));
    Ok(())
}

#[test]
fn callback_identity_and_receiver_context_are_independently_checked() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let admitted = crate::support::accepted(table.admit(crate::support::request(), ()))?;
    let callback = admitted.callback;
    let mut wrong = callback;
    wrong.native = noble_kernel::async_tasks::NativeId(12);
    assert_eq!(
        table.complete(
            wrong,
            noble_kernel::async_tasks::Outcome::Success,
            crate::support::completion()
        ),
        Err(noble_kernel::async_tasks::Error::WrongNative)
    );
    wrong = callback;
    wrong.task.context = noble_kernel::async_tasks::Context(8);
    assert_eq!(
        table.native_stopped(wrong),
        Err(noble_kernel::async_tasks::Error::WrongContext)
    );
    wrong = callback;
    wrong.task.table = noble_kernel::async_tasks::TableId(4);
    assert_eq!(
        table.wake(wrong),
        Err(noble_kernel::async_tasks::Error::InvalidHandle)
    );
    assert_eq!(
        table.snapshot(callback.task.slot),
        Some(admitted.decision.record)
    );
    let rejected = table
        .deliver(admitted.task, noble_kernel::async_tasks::Context(8))
        .err()
        .ok_or("wrong receiver")?;
    assert_eq!(
        rejected.error,
        noble_kernel::async_tasks::Error::WrongContext
    );
    crate::support::accepted(table.complete(
        callback,
        noble_kernel::async_tasks::Outcome::DomainError,
        crate::support::completion(),
    ))?;
    let delivered =
        crate::support::accepted(table.deliver(rejected.input, crate::support::request().context))?;
    assert_eq!(delivered.accounting.delivered.inputs, 1);
    assert_eq!(delivered.accounting.delivered.results, 3);
    assert_eq!(
        delivered.record.outcome,
        Some(noble_kernel::async_tasks::Outcome::DomainError)
    );
    Ok(())
}

#[test]
fn wake_notifications_coalesce_and_exhaustion_retires_without_new_work() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let mut bounded = crate::support::request();
    bounded.wakeups = 1;
    let admitted = crate::support::accepted(table.admit(bounded, ()))?;
    assert!(
        crate::support::accepted(table.wake(admitted.callback))?
            .accounting
            .wake_queued
    );
    assert_eq!(
        crate::support::accepted(table.wake(admitted.callback))?.action,
        noble_kernel::async_tasks::Action::WakeCoalesced
    );
    assert_eq!(table.observation().queued_wakeups, 1);
    crate::support::accepted(table.take_wake(admitted.callback.task))?;
    assert_eq!(
        table.take_wake(admitted.callback.task),
        Err(noble_kernel::async_tasks::Error::NoWakeup)
    );
    let exhausted = crate::support::accepted(table.wake(admitted.callback))?;
    assert_eq!(
        exhausted.record.failure,
        Some(noble_kernel::async_tasks::Failure::Budget)
    );
    assert_eq!(
        exhausted.record.state,
        noble_kernel::async_tasks::State::Retiring
    );
    assert_eq!(
        table.wake(admitted.callback),
        Err(noble_kernel::async_tasks::Error::Retiring)
    );
    assert_eq!(table.observation().outstanding_pins, 2);
    Ok(())
}
