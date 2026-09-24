#[test]
fn delivery_transfers_ready_owners_once_without_settling_native_debt() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let admitted = crate::support::accepted(table.admit(crate::support::request(), ()))?;
    let callback = admitted.callback;
    let ready = crate::support::accepted(table.complete(
        callback,
        noble_kernel::async_tasks::Outcome::Success,
        crate::support::completion(),
    ))?;
    assert_eq!(ready.record.state, noble_kernel::async_tasks::State::Ready);
    assert_eq!(ready.record.returned_inputs, 1);
    assert_eq!(ready.record.results, 3);
    assert_eq!(ready.accounting.consumed_inputs, 2);
    assert!(!ready.record.native_stopped);
    assert_eq!(ready.record.pins, 3);
    let delivered =
        crate::support::accepted(table.deliver(admitted.task, crate::support::request().context))?;
    assert_eq!(
        delivered.accounting.delivered,
        noble_kernel::async_tasks::Obligations {
            inputs: 1,
            results: 3,
            buffers: 2
        }
    );
    let late_cancel = crate::support::accepted(
        table.retire(callback.task, noble_kernel::async_tasks::Failure::Cancelled),
    )?;
    assert_eq!(
        late_cancel.record.state,
        noble_kernel::async_tasks::State::Delivered
    );
    assert_eq!(
        late_cancel.accounting,
        noble_kernel::async_tasks::Accounting::empty()
    );
    settle_delivered_debt(&mut table, callback)
}

fn settle_delivered_debt(
    table: &mut noble_kernel::async_tasks::Table,
    callback: noble_kernel::async_tasks::Callback,
) -> Result<(), String> {
    assert_eq!(
        table.finish(callback.task),
        Err(noble_kernel::async_tasks::Error::NativeStillRunning)
    );
    assert_eq!(
        table.settle_pins(callback, 3),
        Err(noble_kernel::async_tasks::Error::NativeStillRunning)
    );
    crate::support::accepted(table.native_stopped(callback))?;
    assert_eq!(
        table.finish(callback.task),
        Err(noble_kernel::async_tasks::Error::PinsOutstanding)
    );
    assert_eq!(
        crate::support::accepted(table.settle_pins(callback, 1))?
            .accounting
            .pins_released,
        1
    );
    assert_eq!(
        table.settle_pins(callback, 3),
        Err(noble_kernel::async_tasks::Error::InvalidPins)
    );
    assert_eq!(table.observation().outstanding_pins, 1);
    crate::support::accepted(table.settle_pins(callback, 2))?;
    assert_eq!(
        table.finish(callback.task),
        Err(noble_kernel::async_tasks::Error::CleanupOutstanding)
    );
    crate::support::accepted(table.cleanup(
        callback.task,
        noble_kernel::async_tasks::Obligations {
            inputs: 4,
            results: 0,
            buffers: 5,
        },
    ))?;
    assert!(
        crate::support::accepted(table.finish(callback.task))?
            .accounting
            .reservation_released
    );
    assert_eq!(
        table.observation().reserved,
        noble_kernel::async_tasks::Footprint::empty()
    );
    Ok(())
}

#[test]
fn ready_domain_error_resources_remain_owned_until_delivery_or_cancellation() -> Result<(), String>
{
    let mut table = crate::support::table()?;
    let admitted = crate::support::accepted(table.admit(crate::support::request(), ()))?;
    let callback = admitted.callback;
    let ready = crate::support::accepted(table.complete(
        callback,
        noble_kernel::async_tasks::Outcome::DomainError,
        crate::support::completion(),
    ))?;
    assert_eq!(
        ready.action,
        noble_kernel::async_tasks::Action::ResultReady(
            noble_kernel::async_tasks::Outcome::DomainError
        )
    );
    crate::support::accepted(table.native_stopped(callback))?;
    crate::support::accepted(table.settle_pins(callback, 3))?;
    assert_eq!(
        table.cleanup(
            callback.task,
            noble_kernel::async_tasks::Obligations {
                inputs: 1,
                results: 3,
                buffers: 2
            }
        ),
        Err(noble_kernel::async_tasks::Error::InvalidCleanup)
    );
    crate::support::accepted(table.cleanup(
        callback.task,
        noble_kernel::async_tasks::Obligations {
            inputs: 4,
            results: 0,
            buffers: 5,
        },
    ))?;
    let cancelled =
        crate::support::accepted(table.cancel(admitted.task, crate::support::request().context))?;
    assert_eq!(
        cancelled.accounting.retired,
        noble_kernel::async_tasks::Obligations {
            inputs: 1,
            results: 3,
            buffers: 2
        }
    );
    crate::support::accepted(table.cleanup(callback.task, cancelled.accounting.retired))?;
    crate::support::accepted(table.finish(callback.task))?;
    let duplicate = crate::support::accepted(table.complete(
        callback,
        noble_kernel::async_tasks::Outcome::Success,
        crate::support::completion(),
    ))?;
    assert_eq!(
        duplicate.action,
        noble_kernel::async_tasks::Action::Duplicate
    );
    assert_eq!(
        duplicate.accounting,
        noble_kernel::async_tasks::Accounting::empty()
    );
    Ok(())
}

#[test]
fn duplicate_completion_cannot_replace_ready_result_or_deliver_it_twice() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let admitted = crate::support::accepted(table.admit(crate::support::request(), ()))?;
    let callback = admitted.callback;
    let early = table
        .deliver(admitted.task, crate::support::request().context)
        .err()
        .ok_or("pending delivery")?;
    assert_eq!(early.error, noble_kernel::async_tasks::Error::Pending);
    let ready = crate::support::accepted(table.complete(
        callback,
        noble_kernel::async_tasks::Outcome::Success,
        crate::support::completion(),
    ))?;
    let duplicate = crate::support::accepted(table.complete(
        callback,
        noble_kernel::async_tasks::Outcome::DomainError,
        crate::support::completion(),
    ))?;
    assert_eq!(duplicate.record, ready.record);
    assert_eq!(
        duplicate.accounting,
        noble_kernel::async_tasks::Accounting::empty()
    );
    assert_eq!(
        table.finish(callback.task),
        Err(noble_kernel::async_tasks::Error::Ready)
    );
    let delivered =
        crate::support::accepted(table.deliver(early.input, crate::support::request().context))?;
    let repeated = crate::support::accepted(table.complete(
        callback,
        noble_kernel::async_tasks::Outcome::Success,
        crate::support::completion(),
    ))?;
    assert_eq!(repeated.record, delivered.record);
    assert_eq!(
        repeated.accounting,
        noble_kernel::async_tasks::Accounting::empty()
    );
    assert_eq!(
        noble_kernel::async_tasks::transition(
            delivered.record,
            callback.task,
            noble_kernel::async_tasks::Event::Deliver
        ),
        Err(noble_kernel::async_tasks::Error::Delivered)
    );
    Ok(())
}
