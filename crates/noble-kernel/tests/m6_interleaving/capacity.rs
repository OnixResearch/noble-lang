//! Admission enabledness depends on retained task capacity, not just distinct task identities.

#[test]
fn admission_enabledness_depends_on_release_not_only_task_identity() -> Result<(), String> {
    // The deliberately unsound rule "different task ids are independent" is refuted on the real
    // table: with one task slot, the second id's admission depends on the first id's release.
    let mut table = super::support::table(1)?;
    let first = super::support::accepted(table.admit(super::support::request(), ()))?;
    let admitted_pins = first.decision.record.pins;
    let refused = table
        .admit(super::support::request(), ())
        .err()
        .ok_or("the second admission succeeded with no released slot")?;
    assert_eq!(
        refused.error,
        noble_kernel::async_tasks::Error::TaskCapacity
    );
    assert_eq!(table.observation().pending, 1);

    // Releasing the first task is what enables the second, so the two ids are dependent.
    let mut request = super::support::request();
    request.native = noble_kernel::async_tasks::NativeId(12);
    super::support::accepted(table.retire(
        first.callback.task,
        noble_kernel::async_tasks::Failure::Cancelled,
    ))?;
    // A late completion of a cancelled task withdraws the delivered obligations but keeps the
    // primary failure, exactly as the retirement lane records.
    super::support::accepted(table.complete(
        first.callback,
        noble_kernel::async_tasks::Outcome::Success,
        super::support::completion(),
    ))?;
    super::support::accepted(table.native_stopped(first.callback))?;
    super::support::accepted(table.settle_pins(first.callback, admitted_pins))?;
    // The cancelled task still owes every admitted input owner, result owner and buffer.
    let debt = noble_kernel::async_tasks::Obligations {
        inputs: 5,
        results: 3,
        buffers: 7,
    };
    super::support::accepted(table.cleanup(first.callback.task, debt))?;
    super::support::accepted(table.finish(first.callback.task))?;
    let observation = table.observation();
    assert_eq!(observation.retired, 1);
    let second = super::support::accepted(table.admit(request, ()))?;
    assert_eq!(
        second.decision.record.state,
        noble_kernel::async_tasks::State::Pending
    );
    assert_eq!(table.observation().pending, 1);
    Ok(())
}
