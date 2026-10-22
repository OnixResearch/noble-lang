//! Runtime entry emission for the module wrapper.

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-wasm-maintainers; the selected lowering or preflight stage keeps its checked step order in one reviewable unit, and splitting it would separate the emitted sequence from its bounds checks."
)]
pub(in crate::source::emit) fn runtime(
    out: &mut crate::output::Buffer,
    plan: &super::super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    if plan.live_slots {
        attempt!(super::fragment(
            out,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/runtime/live-accounting.wat"
            ))
        ));
        attempt!(super::fragment(
            out,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/runtime/live-linear-storage.wat"
            ))
        ));
        attempt!(super::fragment(
            out,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/runtime/live-borrows.wat"
            ))
        ));
        attempt!(super::fragment(
            out,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/runtime/live-stack.wat"
            ))
        ));
    } else {
        attempt!(accounting(out, plan.worker_interruption));
        if plan.calculator {
            attempt!(super::fragment(
                out,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/runtime/calculator-linear-address.wat"
                ))
            ));
        } else {
            attempt!(super::fragment(
                out,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/runtime/linear-address.wat"
                ))
            ));
        }
        attempt!(super::fragment(
            out,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/runtime/linear-storage.wat"
            ))
        ));
        attempt!(super::fragment(
            out,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/stack.wat"))
        ));
        if plan.core_live {
            attempt!(super::fragment(
                out,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/runtime/core-live-owners.wat"
                ))
            ));
        }
    }
    attempt!(super::fragment(
        out,
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/programs.wat"))
    ));
    attempt!(super::fragment(
        out,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/runtime/operations.wat"
        ))
    ));
    if plan.calculator {
        attempt!(super::fragment(
            out,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/runtime/calculator.wat"
            ))
        ));
    }
    attempt!(super::fragment(
        out,
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/data.wat"))
    ));
    if plan.has_text_byte {
        attempt!(super::fragment(
            out,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/runtime/text-byte.wat"
            ))
        ));
    }
    if plan.has_i64_less {
        attempt!(super::fragment(
            out,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/i64-less.wat"))
        ));
    }
    if plan.has_core_emit {
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
            attempt!(
                out.append(b" (if (i32.eqz (global.get $failure)) (then (call $push_unit)))\n")
            );
        }
        attempt!(out.append(b")\n"));
    }
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
        attempt!(super::fragment(out, include_str!("../nominal.wat")));
    }
    if plan.has_bound_emit {
        attempt!(super::fragment(out, include_str!("../bound-emit.wat")));
    }
    if plan.has_bound_clock {
        attempt!(out.append(b"(func $op_clock_bound (param $slot i32) (local $status i32) (local $value i64)\n (global.set $failure (i32.const 5))\n (call $host_clock_bound (local.get $slot))\n (local.set $value)\n (local.set $status)\n (if (i32.eqz (local.get $status)) (then\n   (global.set $failure (i32.const 0))\n   (call $push_i64 (local.get $value)))))\n"));
    }
    if plan.has_bound_code_export {
        attempt!(out.append(b"(func $op_code_export_bound (param $slot i32)\n (if (global.get $failure) (then (return)))\n ;; A source-bound export request is deny-only: poison first, send only its slot, and never let host success clear the failure.\n (global.set $failure (i32.const 5))\n (drop (call $host_code_export_bound (local.get $slot))))\n"));
    }
    if plan.has_live_propose && plan.core_live {
        attempt!(out.append(
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
"
        ));
    } else if plan.has_live_propose {
        attempt!(out.append(b"(func $op_live_propose (param $owner i64)
 (local $program i32) (local $expected i64)
 (local.set $program (i32.wrap_i64 (call $pop_kind (i32.const 4))))
 (local.set $expected (call $pop_kind (i32.const 1)))
 (if (global.get $failure) (then (return)))
 ;; A rejected queue request or a native throw poisons this invocation.
 (global.set $failure (i32.const 5))
 (if (i32.eqz (call $host_live_propose (local.get $owner) (local.get $expected) (local.get $program)))
  (then (global.set $failure (i32.const 0)) (call $push_unit))))
"));
    }
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
    attempt!(super::super::reflection::write(
        out,
        plan.declared_modules,
        plan.worker_interruption
    ));
    attempt!(super::fragment(
        out,
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/source.wat"))
    ));
    if plan.live_slots {
        attempt!(super::fragment(
            out,
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/resource.wat"))
        ));
        attempt!(super::fragment(
            out,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/runtime/live-source.wat"
            ))
        ));
    }
    if plan.worker_inputs {
        attempt!(super::fragment(
            out,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/runtime/worker-input.wat"
            ))
        ));
    }
    Ok(())
}

const ACCOUNTING: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/runtime/accounting.wat"
));
const CHARGING_TICK: &str = "(func $runtime_tick (result i32)";

/// The shared charging semantics. The typed-worker profile keeps that tick
/// as `$runtime_charge` and defines `$runtime_tick` and `$observation_tick`
/// over the charges with an interruption point every eighth unit.
fn accounting(
    out: &mut crate::output::Buffer,
    interruptible: bool,
) -> Result<(), crate::Diagnostic> {
    if !interruptible {
        return super::fragment(out, ACCOUNTING);
    }
    let Some((before, after)) = ACCOUNTING.split_once(CHARGING_TICK) else {
        return Err(crate::Diagnostic::Defective);
    };
    attempt!(out.append(before.as_bytes()));
    attempt!(out.append(b"(func $runtime_charge (result i32)"));
    attempt!(out.append(after.as_bytes()));
    attempt!(out.append(b"\n"));
    super::fragment(
        out,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/runtime/worker-interruption.wat"
        )),
    )
}
