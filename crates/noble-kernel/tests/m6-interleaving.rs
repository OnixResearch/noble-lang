#![feature(register_tool)]
#![register_tool(octet)]
#![register_tool(tigerstyle)]
//! Issue #46 negative controls: each control drives the real M6 async table and
//! asserts the exact observation a dropped guard would change.
//!
//! Four guards are load-bearing, and each test below reaches the real
//! `noble_kernel::async_tasks::Table` for the outcome it protects: a completion
//! must not release the admitted pins, a callback's handle generation must match
//! the retained record, a duplicate completion callback must charge nothing, and a
//! late completion must not overwrite the first abnormal failure. The expectations
//! are stated independently of the table, so removing or weakening one of those
//! guards makes the corresponding expectation fail.
//!
//! What the four negative controls do not claim: no mutant rule is re-implemented
//! or compared here — a local restatement cannot detect anything. They make no
//! correspondence claim with the Lean model, and a caught mutation is not a theorem.
//! The separate #115 scope modules below exercise finite, explicitly generated
//! table traces; those tests do not establish a Rust-to-Lean refinement theorem.

#[path = "m6_interleaving/capacity.rs"]
mod capacity;

#[path = "m6_interleaving/support.rs"]
mod support;

#[path = "m6_interleaving/scope/manager.rs"]
mod manager;

#[path = "m6_interleaving/scope/registry.rs"]
mod registry;

#[path = "m6_interleaving/scope/support.rs"]
mod scope_support;

#[path = "m6_interleaving/scope/traces.rs"]
mod traces;

#[path = "m6_interleaving/scope/corpus/retirement.rs"]
mod retirement;

#[path = "m6_interleaving/scope/controls.rs"]
mod controls;

#[path = "m6_interleaving/scope/corpus.rs"]
mod corpus;

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
        "a duplicate callback changes no observable count"
    );
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
    Ok(())
}
