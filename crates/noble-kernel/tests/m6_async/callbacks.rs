#[test]
fn slot_reuse_requires_finalization_of_the_previous_task() -> Result<(), String> {
    // Two slots: a one-slot table refuses a second admission through the
    // reservation check, so the finalized-slot rule can only be observed here.
    let mut table = crate::support::table()?;
    let first = crate::support::accepted(table.admit(crate::support::request(), ()))?;
    let second = crate::support::accepted(table.admit(crate::support::request(), ()))?;
    let first_slot = first.task.handle().slot;
    let second_handle = second.task.handle();
    let second_slot = second_handle.slot;
    assert_eq!(
        table.snapshot(first_slot),
        Some(first.decision.record),
        "the second admission does not overwrite the first live record"
    );
    assert_ne!(first_slot, second_slot, "each admission takes its own slot");

    // Both records are live and unfinalized: the table must refuse rather
    // than overwrite either one's pins and reservation.
    let full = table
        .admit(crate::support::request(), ())
        .err()
        .ok_or("a full table of unfinalized tasks admitted a third task")?;
    assert_eq!(full.error, noble_kernel::async_tasks::Error::TaskCapacity);

    // Finalize the second task only. Its slot becomes reusable; the first
    // live record must be left exactly as admitted.
    let second_generation = second_handle.generation;
    crate::support::accepted(table.cancel(second.task, crate::support::request().context))?;
    crate::support::accepted(table.native_stopped(second.callback))?;
    crate::support::accepted(table.complete(
        second.callback,
        noble_kernel::async_tasks::Outcome::Success,
        crate::support::completion(),
    ))?;
    crate::support::accepted(table.settle_pins(second.callback, 3))?;
    crate::support::accepted(table.cleanup(
        second_handle,
        noble_kernel::async_tasks::Obligations {
            inputs: 7,
            results: 0,
            buffers: 5,
        },
    ))?;
    crate::support::accepted(table.finish(second_handle))?;

    let reused = crate::support::accepted(table.admit(crate::support::request(), ()))?;
    assert_eq!(
        reused.task.handle().slot,
        second_slot,
        "the finalized slot is the one reused"
    );
    assert_ne!(
        reused.task.handle().generation,
        second_generation,
        "reuse starts a new generation"
    );
    assert_eq!(
        table.snapshot(first_slot),
        Some(first.decision.record),
        "the live task's record is not overwritten"
    );
    crate::support::accepted(table.complete(
        first.callback,
        noble_kernel::async_tasks::Outcome::Success,
        crate::support::completion(),
    ))?;
    Ok(())
}

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
