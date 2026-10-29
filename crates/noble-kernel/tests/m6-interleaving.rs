#![feature(register_tool)]
#![register_tool(tigerstyle)]
//! Issue #46 negative controls: mutation guards over the real M6 async table API.
//!
//! These controls exist to show four specific guards are *load-bearing*: an early pin release, a
//! dropped generation check, a double delivery accounting and an overwritten primary failure each
//! change the documented observation, so a candidate implementation that dropped one of them would
//! be detected rather than pass silently. Each control drives the real
//! `noble_kernel::async_tasks::Table` and asserts the real outcome as an independently stated
//! expectation before it compares the mutated rule's outcome against the documented one.
//!
//! What this is not: the mutant is a rule applied to the real table's recorded state, not a second
//! implementation of the table. No correspondence claim is made between these controls and the Lean
//! `TaskInterleaving` model, and a caught mutation is not a theorem.

#[path = "m6_interleaving/capacity.rs"]
mod capacity;

#[path = "m6_interleaving/support.rs"]
mod support;

/// One mutated rule. `Documented` is the rule the table actually implements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    Documented,
    /// Release the admitted pins as soon as a completion arrives, instead of only after the
    /// exact settlement acknowledgement.
    EarlyPinRelease,
    /// Accept a callback whose handle generation does not match the retained record.
    DroppedGenerationCheck,
    /// Charge a second delivery for a task that is already delivered.
    DoubleDeliveryAccounting,
    /// Replace the first abnormal failure with a later completion's failure.
    OverwrittenPrimaryFailure,
}

const fn pins_after_completion(rule: Rule, record: &noble_kernel::async_tasks::Snapshot) -> u64 {
    match rule {
        Rule::EarlyPinRelease => 0,
        _ => record.pins,
    }
}

const fn generation_accepted(
    rule: Rule,
    claimed: &noble_kernel::async_tasks::Handle,
    retained: &noble_kernel::async_tasks::Snapshot,
) -> bool {
    match rule {
        Rule::DroppedGenerationCheck => true,
        _ => claimed.generation == retained.handle.generation,
    }
}

const fn delivered_count(rule: Rule, deliveries: usize) -> usize {
    match rule {
        Rule::DoubleDeliveryAccounting => deliveries.saturating_mul(2),
        _ => deliveries,
    }
}

const fn failure_after_late_completion(
    rule: Rule,
    first: noble_kernel::async_tasks::Failure,
    later: noble_kernel::async_tasks::Failure,
) -> noble_kernel::async_tasks::Failure {
    match rule {
        Rule::OverwrittenPrimaryFailure => later,
        _ => first,
    }
}

#[test]
fn an_early_pin_release_is_detected_by_the_settlement_guard() -> Result<(), String> {
    let mut table = support::table(2)?;
    let admitted = support::accepted(table.admit(support::request(), ()))?;
    let callback = admitted.callback;
    let completed = support::accepted(table.complete(
        callback,
        noble_kernel::async_tasks::Outcome::Success,
        support::completion(),
    ))?;
    // A successful completion makes the task deliverable; it does not settle the admitted pins.
    assert_eq!(
        completed.record.state,
        noble_kernel::async_tasks::State::Ready
    );
    let retained = completed.record.pins;
    assert_eq!(retained, 3, "the admitted callback pins stay outstanding");

    // Retirement keeps the outstanding pins, and `finish` is then refused because the native
    // operation has not stopped: an early release would lose that refusal.
    let retiring =
        support::accepted(table.retire(callback.task, noble_kernel::async_tasks::Failure::Trap))?;
    assert_eq!(
        retiring.record.state,
        noble_kernel::async_tasks::State::Retiring
    );
    assert_eq!(retiring.record.pins, retained);
    let premature = table
        .finish(callback.task)
        .err()
        .ok_or("finish succeeded before native stop and settlement")?;
    assert_eq!(
        premature,
        noble_kernel::async_tasks::Error::NativeStillRunning
    );

    support::accepted(table.native_stopped(callback))?;
    let unsettled = table
        .cleanup(
            callback.task,
            noble_kernel::async_tasks::Obligations {
                inputs: 0,
                results: 0,
                buffers: 0,
            },
        )
        .err()
        .ok_or("cleanup succeeded with pins unsettled")?;
    assert_eq!(unsettled, noble_kernel::async_tasks::Error::PinsOutstanding);

    let documented = pins_after_completion(Rule::Documented, &completed.record);
    let mutated = pins_after_completion(Rule::EarlyPinRelease, &completed.record);
    assert_eq!(documented, 3);
    assert_eq!(mutated, 0);
    assert_ne!(
        documented, mutated,
        "an early pin release must change the documented pin observation"
    );
    Ok(())
}

#[test]
fn a_dropped_generation_check_is_detected_by_the_handle_guard() -> Result<(), String> {
    let mut table = support::table(2)?;
    let admitted = support::accepted(table.admit(support::request(), ()))?;
    let callback = admitted.callback;
    let retained = admitted.decision.record;
    assert_eq!(retained.handle.generation, callback.task.generation);

    // A handle that names the same slot and context but an older generation is refused by the real
    // table, so a callback from a previous occupant of the slot cannot mutate the retained record.
    let stale = noble_kernel::async_tasks::Handle {
        generation: callback.task.generation.saturating_sub(1),
        ..callback.task
    };
    let refused = table.complete(
        noble_kernel::async_tasks::Callback {
            task: stale,
            native: callback.native,
        },
        noble_kernel::async_tasks::Outcome::Success,
        support::completion(),
    );
    assert_eq!(
        refused.err(),
        Some(noble_kernel::async_tasks::Error::WrongGeneration)
    );
    assert_eq!(
        table.snapshot(callback.task.slot),
        Some(retained),
        "the refused callback changed nothing"
    );

    assert!(!generation_accepted(Rule::Documented, &stale, &retained));
    assert!(generation_accepted(
        Rule::DroppedGenerationCheck,
        &stale,
        &retained
    ));
    Ok(())
}

#[test]
fn double_delivery_accounting_is_detected_by_the_duplicate_callback_guard() -> Result<(), String> {
    let mut table = support::table(2)?;
    let admitted = support::accepted(table.admit(support::request(), ()))?;
    let callback = admitted.callback;
    support::accepted(table.complete(
        callback,
        noble_kernel::async_tasks::Outcome::Success,
        support::completion(),
    ))?;
    let delivered = support::accepted(table.deliver(admitted.task, support::request().context))?;
    assert_eq!(
        delivered.record.state,
        noble_kernel::async_tasks::State::Delivered
    );
    let after_first = table.observation();
    assert_eq!(after_first.delivered, 1);
    assert_eq!(after_first.retiring, 0);

    // A caller cannot express a repeated `deliver` at all: the task handle is move-only and the
    // first delivery consumed it. What a second delivery would charge is therefore measured on the
    // reachable duplicate surface instead: a repeated completion callback for a delivered task.
    let repeated = support::accepted(table.complete(
        callback,
        noble_kernel::async_tasks::Outcome::Success,
        support::completion(),
    ))?;
    assert_eq!(
        repeated.record.state,
        noble_kernel::async_tasks::State::Delivered
    );
    assert_eq!(
        repeated.accounting,
        noble_kernel::async_tasks::Accounting::empty(),
        "a duplicate callback charges nothing"
    );
    let after_second = table.observation();
    assert_eq!(
        after_second, after_first,
        "a duplicate callback changes no documented count"
    );

    let documented = delivered_count(Rule::Documented, after_first.delivered);
    let mutated = delivered_count(Rule::DoubleDeliveryAccounting, after_first.delivered);
    assert_eq!(documented, 1);
    assert_eq!(mutated, 2);
    assert_ne!(documented, mutated);
    Ok(())
}

#[test]
fn an_overwritten_primary_failure_is_detected_by_the_sticky_failure_rule() -> Result<(), String> {
    let mut table = support::table(2)?;
    let admitted = support::accepted(table.admit(support::request(), ()))?;
    let callback = admitted.callback;
    let cancelled = support::accepted(
        table.retire(callback.task, noble_kernel::async_tasks::Failure::Cancelled),
    )?;
    assert_eq!(
        cancelled.record.failure,
        Some(noble_kernel::async_tasks::Failure::Cancelled)
    );
    // A late completion keeps the first abnormal failure; the shell's retirement accounting
    // withdraws the delivered obligations instead of overwriting the cause.
    let late = support::accepted(table.complete(
        callback,
        noble_kernel::async_tasks::Outcome::DomainError,
        support::completion(),
    ))?;
    assert_eq!(
        late.record.state,
        noble_kernel::async_tasks::State::Retiring
    );
    let first = late.record.failure.ok_or("the sticky failure was lost")?;
    assert_eq!(first, noble_kernel::async_tasks::Failure::Cancelled);

    let documented = failure_after_late_completion(
        Rule::Documented,
        noble_kernel::async_tasks::Failure::Cancelled,
        noble_kernel::async_tasks::Failure::Internal,
    );
    let mutated = failure_after_late_completion(
        Rule::OverwrittenPrimaryFailure,
        noble_kernel::async_tasks::Failure::Cancelled,
        noble_kernel::async_tasks::Failure::Internal,
    );
    assert_eq!(documented, noble_kernel::async_tasks::Failure::Cancelled);
    assert_eq!(mutated, noble_kernel::async_tasks::Failure::Internal);
    assert_ne!(documented, mutated);
    Ok(())
}
