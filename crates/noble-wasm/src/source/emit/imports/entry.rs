//! Runtime entry emission for the module wrapper.

// Runtime fragments are named constants so that their large literal text
// stays out of the branching emitters that select them.
const LIVE_ACCOUNTING: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/live-accounting.wat"
));
const LIVE_LINEAR_STORAGE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/live-linear-storage.wat"
));
const LIVE_BORROWS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/live-borrows.wat"
));
const LIVE_STACK: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/live-stack.wat"
));
const CALCULATOR_LINEAR_ADDRESS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/calculator-linear-address.wat"
));
const CALCULATOR: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/calculator.wat"
));
const CHECKED_I64: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/checked-i64.wat"
));
const TEXT_BYTE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/text-byte.wat"
));
const I64_LESS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/i64-less.wat"));
const EXPERIMENT_CLEANUP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/experiment-cleanup.wat"
));
const RESOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/resource.wat"));
const LIVE_SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/live-source.wat"
));
const WORKER_INPUT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/worker-input.wat"
));
const WORKER_INTERRUPTION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/worker-interruption.wat"
));
const NOMINAL: &str = include_str!("../nominal.wat");
const BOUND_EMIT: &str = include_str!("../bound-emit.wat");
const BOUND_SEND: &str = include_str!("../bound-send.wat");

/// The ordered runtime fragments and native operations the plan selects.
///
/// Each group is its own short function: Aeneas joins the symbolic state
/// after every continuing branch, and one long sequence of optional
/// fragments multiplies its paths.
pub(in crate::source::emit) fn runtime(
    out: &mut crate::output::Buffer,
    plan: &super::super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    attempt!(storage_runtime(out, plan));
    attempt!(evaluation_runtime(out, plan));
    attempt!(scalar_operations(out, plan));
    attempt!(host_operations(out, plan));
    attempt!(bound_operations(out, plan));
    attempt!(live_operations(out, plan));
    attempt!(super::super::reflection::write(
        out,
        plan.declared_modules,
        plan.worker_interruption
    ));
    attempt!(super::fragment(out, super::SOURCE));
    profile_runtime(out, plan)
}

/// Accounting, addressing, storage and stack fragments for the plan's profile.
fn storage_runtime(
    out: &mut crate::output::Buffer,
    plan: &super::super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    if plan.live_slots {
        attempt!(super::fragment(out, LIVE_ACCOUNTING));
        attempt!(super::fragment(out, LIVE_LINEAR_STORAGE));
        attempt!(super::fragment(out, LIVE_BORROWS));
        attempt!(super::fragment(out, LIVE_STACK));
    } else {
        attempt!(linear_storage_runtime(out, plan));
    }
    Ok(())
}

/// Accounting, linear addressing, storage and stack fragments outside the
/// live-slot profile.
fn linear_storage_runtime(
    out: &mut crate::output::Buffer,
    plan: &super::super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    attempt!(accounting(out, plan.worker_interruption));
    if plan.calculator {
        attempt!(super::fragment(out, CALCULATOR_LINEAR_ADDRESS));
    } else {
        attempt!(super::fragment(out, super::LINEAR_ADDRESS));
    }
    attempt!(super::fragment(out, super::LINEAR_STORAGE));
    attempt!(super::fragment(out, super::STACK));
    if plan.core_live {
        attempt!(super::fragment(out, super::CORE_LIVE_OWNERS));
    }
    Ok(())
}

/// Program, operation and data fragments with the profile's arithmetic.
fn evaluation_runtime(
    out: &mut crate::output::Buffer,
    plan: &super::super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    attempt!(super::fragment(out, super::PROGRAMS));
    attempt!(super::fragment(out, super::OPERATIONS));
    if plan.calculator {
        attempt!(super::fragment(out, CALCULATOR));
    }
    if plan.checked_i64 {
        attempt!(super::fragment(out, CHECKED_I64));
    }
    attempt!(super::fragment(out, super::DATA));
    Ok(())
}

/// Text-byte and I64 comparison fragments and the Core emit entry.
fn scalar_operations(
    out: &mut crate::output::Buffer,
    plan: &super::super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    if plan.has_text_byte {
        attempt!(super::fragment(out, TEXT_BYTE));
    }
    if plan.has_i64_less {
        attempt!(super::fragment(out, I64_LESS));
    }
    if plan.has_core_emit {
        attempt!(core_text_output(out, plan));
    }
    Ok(())
}

/// The Core host emit entry, which hands one Text value to the host.
fn core_text_output(
    out: &mut crate::output::Buffer,
    plan: &super::super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(
        b"(func $op_emit (local $text i32)
 (local.set $text (i32.wrap_i64 (call $pop_kind (i32.const 11))))
 (if (global.get $failure) (then (return)))
 ;; A native host throw also leaves the session poisoned. Successful return is
 ;; the only place this provisional host-call failure may be cleared.
 (global.set $failure (i32.const 5))
 (if (i32.eqz (call $host_emit (call $x (local.get $text)) (call $y (local.get $text))))
  (then (global.set $failure (i32.const 0))))
"
    ));
    if plan.live_test_hosts {
        // The kernel's live-slot bootstrap emits Unit; Core source keeps
        // its historical no-result effect. Do not materialize a response
        // when the independently authorized host call denied the effect.
        attempt!(out.append(b" (if (i32.eqz (global.get $failure)) (then (call $push_unit)))\n"));
    }
    out.append(b")\n")
}

/// Core abort, nominal and bound emit entries.
fn host_operations(
    out: &mut crate::output::Buffer,
    plan: &super::super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    if plan.has_core_abort {
        attempt!(out.append(
            b"(func $op_abort
 (if (global.get $failure) (then (return)))
 (call $fail (i32.const 6))
 (drop (call $host_abort)))
"
        ));
    }
    if plan.has_nominals {
        attempt!(super::fragment(out, NOMINAL));
    }
    if plan.has_bound_emit {
        attempt!(super::fragment(out, BOUND_EMIT));
    }
    Ok(())
}

/// Bound clock, code-export and send entries.
fn bound_operations(
    out: &mut crate::output::Buffer,
    plan: &super::super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    if plan.has_bound_clock {
        attempt!(out.append(b"(func $op_clock_bound (param $slot i32) (local $status i32) (local $value i64)\n (global.set $failure (i32.const 5))\n (call $host_clock_bound (local.get $slot))\n (local.set $value)\n (local.set $status)\n (if (i32.eqz (local.get $status)) (then\n   (global.set $failure (i32.const 0))\n   (call $push_i64 (local.get $value)))))\n"));
    }
    if plan.has_bound_code_export {
        attempt!(out.append(b"(func $op_code_export_bound (param $slot i32)\n (if (global.get $failure) (then (return)))\n ;; A source-bound export request is deny-only: poison first, send only its slot, and never let host success clear the failure.\n (global.set $failure (i32.const 5))\n (drop (call $host_code_export_bound (local.get $slot))))\n"));
    }
    if plan.has_bound_send {
        attempt!(super::fragment(out, BOUND_SEND));
    }
    Ok(())
}

/// Live proposal and generation entries.
fn live_operations(
    out: &mut crate::output::Buffer,
    plan: &super::super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    attempt!(live_propose(out, plan));
    if plan.has_live_generation {
        attempt!(out.append(
            b"(func $op_live_generation (local $current i64)
 (if (global.get $failure) (then (return)))
 (global.set $failure (i32.const 5))
 (local.set $current (call $host_live_generation))
 (global.set $failure (i32.const 0))
 (call $push_i64 (local.get $current)))
"
        ));
    }
    Ok(())
}

/// The live proposal entry, whose callback window depends on Core live.
fn live_propose(
    out: &mut crate::output::Buffer,
    plan: &super::super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    if plan.has_live_propose && plan.core_live {
        out.append(
            b"(func $op_live_propose (param $owner i64)
 (local $program i32) (local $expected i64) (local $approved i32)
 (local.set $program (i32.wrap_i64 (call $pop_kind (i32.const 4))))
 (local.set $expected (call $pop_kind (i32.const 1)))
 (if (global.get $failure) (then (return)))
 ;; Permit provisional saved-Program roots only in this exact host callback.
 (global.set $failure (i32.const 5))
 (global.set $core_live_proposal_callback_active (i32.const 1))
 (local.set $approved
  (call $host_live_propose (local.get $owner) (local.get $expected) (local.get $program)))
 (global.set $core_live_proposal_callback_active (i32.const 0))
 (if (i32.eqz (local.get $approved))
  (then (global.set $failure (i32.const 0)) (call $push_unit))))
",
        )
    } else if plan.has_live_propose {
        out.append(b"(func $op_live_propose (param $owner i64)
 (local $program i32) (local $expected i64)
 (local.set $program (i32.wrap_i64 (call $pop_kind (i32.const 4))))
 (local.set $expected (call $pop_kind (i32.const 1)))
 (if (global.get $failure) (then (return)))
 ;; A rejected queue request or a native throw poisons this invocation.
 (global.set $failure (i32.const 5))
 (if (i32.eqz (call $host_live_propose (local.get $owner) (local.get $expected) (local.get $program)))
  (then (global.set $failure (i32.const 0)) (call $push_unit))))
")
    } else {
        Ok(())
    }
}

/// Cleanup, live-slot resource and worker-input fragments.
fn profile_runtime(
    out: &mut crate::output::Buffer,
    plan: &super::super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    if plan.experimental_cleanup {
        attempt!(super::fragment(out, EXPERIMENT_CLEANUP));
    }
    if plan.live_slots {
        attempt!(super::fragment(out, RESOURCE));
        attempt!(super::fragment(out, LIVE_SOURCE));
    }
    if plan.worker_inputs {
        attempt!(super::fragment(out, WORKER_INPUT));
    }
    Ok(())
}

const CHARGING_TICK: &str = "(func $runtime_tick (result i32)";

/// The shared charging semantics. The typed-worker profile keeps that tick
/// as `$runtime_charge` and defines `$runtime_tick` and `$observation_tick`
/// over the charges with an interruption point every eighth unit.
fn accounting(
    out: &mut crate::output::Buffer,
    interruptible: bool,
) -> Result<(), crate::Diagnostic> {
    if !interruptible {
        return super::fragment(out, super::ACCOUNTING);
    }
    let Some((before, after)) =
        super::super::split_once_bytes(super::ACCOUNTING.as_bytes(), CHARGING_TICK.as_bytes())
    else {
        return Err(crate::Diagnostic::Defective);
    };
    attempt!(out.append(before));
    attempt!(out.append(b"(func $runtime_charge (result i32)"));
    attempt!(out.append(after));
    attempt!(out.append(b"\n"));
    super::fragment(out, WORKER_INTERRUPTION)
}
