#[test]
fn cancellation_before_completion_never_restores_input_or_result_owners() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let admitted = crate::support::accepted(table.admit(crate::support::request(), ()))?;
    let callback = admitted.callback;
    let cancelled = crate::support::accepted(
        table.retire(callback.task, noble_kernel::async_tasks::Failure::Cancelled),
    )?;
    assert_eq!(
        cancelled.record.state,
        noble_kernel::async_tasks::State::Retiring
    );
    assert_eq!(cancelled.accounting.retired.inputs, 7);
    assert_eq!(cancelled.record.pins, 3);
    assert!(!cancelled.record.native_stopped);
    let late = crate::support::accepted(table.complete(
        callback,
        noble_kernel::async_tasks::Outcome::Success,
        crate::support::completion(),
    ))?;
    assert_eq!(
        late.record.state,
        noble_kernel::async_tasks::State::Retiring
    );
    assert_eq!(
        late.record.failure,
        Some(noble_kernel::async_tasks::Failure::Cancelled)
    );
    assert_eq!(late.accounting.retirement_withdrawn_inputs, 2);
    assert_eq!(late.record.retiring_inputs, 5);
    assert_eq!(late.record.returned_inputs, 0);
    assert_eq!(
        late.accounting.delivered,
        noble_kernel::async_tasks::Obligations::empty()
    );
    let rejected = table
        .deliver(admitted.task, crate::support::request().context)
        .err()
        .ok_or("delivery succeeded")?;
    assert_eq!(rejected.error, noble_kernel::async_tasks::Error::Retiring);
    let repeated =
        crate::support::accepted(table.cancel(rejected.input, crate::support::request().context))?;
    assert_eq!(
        repeated.accounting,
        noble_kernel::async_tasks::Accounting::empty()
    );
    crate::support::accepted(table.native_stopped(callback))?;
    crate::support::accepted(table.settle_pins(callback, 3))?;
    let debt = noble_kernel::async_tasks::Obligations {
        inputs: 5,
        results: 3,
        buffers: 7,
    };
    crate::support::accepted(table.cleanup(callback.task, debt))?;
    assert_eq!(
        table.cleanup(callback.task, debt),
        Err(noble_kernel::async_tasks::Error::InvalidCleanup)
    );
    assert_eq!(
        crate::support::accepted(table.finish(callback.task))?
            .record
            .state,
        noble_kernel::async_tasks::State::Retired
    );
    Ok(())
}

#[test]
fn oversized_result_retires_without_admitting_unreserved_bytes() -> Result<(), String> {
    let mut table = crate::support::table()?;
    let admitted = crate::support::accepted(table.admit(crate::support::request(), ()))?;
    let mut oversized = crate::support::completion();
    oversized.bytes = crate::support::request()
        .result_bytes
        .checked_add(1)
        .ok_or("test result length overflow")?;
    let result = crate::support::accepted(table.complete(
        admitted.callback,
        noble_kernel::async_tasks::Outcome::Success,
        oversized,
    ))?;
    assert_eq!(
        result.record.state,
        noble_kernel::async_tasks::State::Retiring
    );
    assert_eq!(
        result.record.failure,
        Some(noble_kernel::async_tasks::Failure::Budget)
    );
    assert_eq!(result.accounting.rejected_result_bytes, oversized.bytes);
    assert_eq!(result.accounting.acquired.buffers, 0);
    assert_eq!(result.accounting.retired.inputs, 5);
    assert_eq!(result.accounting.retired.results, 3);
    assert_eq!(result.record.buffers, 5);
    assert_eq!(result.record.pins, 3);
    assert_eq!(table.observation().reserved.bytes, 28);
    let rejected = table
        .deliver(admitted.task, crate::support::request().context)
        .err()
        .ok_or("oversized delivery")?;
    assert_eq!(rejected.error, noble_kernel::async_tasks::Error::Retiring);
    crate::support::accepted(table.native_stopped(admitted.callback))?;
    crate::support::accepted(table.settle_pins(admitted.callback, 3))?;
    crate::support::accepted(table.cleanup(
        admitted.callback.task,
        noble_kernel::async_tasks::Obligations {
            inputs: 5,
            results: 3,
            buffers: 5,
        },
    ))?;
    crate::support::accepted(table.finish(admitted.callback.task))?;
    Ok(())
}

#[test]
fn every_abnormal_exit_preserves_its_primary_failure_through_late_success() -> Result<(), String> {
    for failure in [
        noble_kernel::async_tasks::Failure::Trap,
        noble_kernel::async_tasks::Failure::Deadline,
        noble_kernel::async_tasks::Failure::Budget,
        noble_kernel::async_tasks::Failure::Internal,
        noble_kernel::async_tasks::Failure::Cancelled,
    ] {
        let mut table = crate::support::table()?;
        let admitted = crate::support::accepted(table.admit(crate::support::request(), ()))?;
        crate::support::accepted(table.retire(admitted.callback.task, failure))?;
        crate::support::accepted(table.retire(
            admitted.callback.task,
            noble_kernel::async_tasks::Failure::Internal,
        ))?;
        let late = crate::support::accepted(table.complete(
            admitted.callback,
            noble_kernel::async_tasks::Outcome::Success,
            crate::support::completion(),
        ))?;
        assert_eq!(late.record.failure, Some(failure));
        assert_eq!(
            late.record.state,
            noble_kernel::async_tasks::State::Retiring
        );
        assert_eq!(
            late.accounting.delivered,
            noble_kernel::async_tasks::Obligations::empty()
        );
        assert_eq!(late.accounting.pins_released, 0);
    }
    Ok(())
}
