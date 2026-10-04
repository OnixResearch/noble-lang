#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; emission mutates only the fresh private output sink; checked plans and caller inputs remain immutable and no partial output escapes preparation."
)]

pub(super) fn write(
    out: &mut crate::output::Buffer,
    plan: &super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(module\n(type $entry (func (param i32)))\n(import \"noble\" \"memory\" (memory 16 16))\n(import \"noble\" \"table\" (table 16384 16384 funcref))\n"));
    if plan.has_core_emit {
        attempt!(out.append(
            b"(import \"noble\" \"test_emit\" (func $host_emit (param i32 i32) (result i32)))\n"
        ));
    }
    if plan.has_core_abort {
        attempt!(out.append(b"(import \"noble\" \"test_abort\" (func $host_abort (result i32)))\n"));
    }
    if plan.has_bound_emit {
        attempt!(out.append(b"(import \"noble\" \"test_emit_bound\" (func $host_emit_bound (param i32 i32 i32) (result i32)))\n"));
    }
    if plan.has_bound_clock {
        attempt!(out.append(b"(import \"noble\" \"test_clock_bound\" (func $host_clock_bound (param i32) (result i32 i64)))\n"));
    }
    if plan.has_live_propose {
        attempt!(out.append(b"(import \"noble\" \"live_propose\" (func $host_live_propose (param i64 i64 i32) (result i32)))\n"));
    }
    if plan.has_live_generation {
        attempt!(out.append(b"(import \"noble\" \"live_generation\" (func $host_live_generation (result i64)))\n"));
    }
    attempt!(globals(out));
    attempt!(global_import(out, b"allocated_total", b"i64"));
    attempt!(global_import(out, b"released_total", b"i64"));
    out.append(b"(export \"memory\" (memory 0))\n(global $source_reflection i32 (i32.const 1))\n")
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the fixed ABI global imports are appended in their required order through the private bounded sink; output exhaustion propagates a diagnostic without asserting over producer data."
)]
fn globals(out: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
    attempt!(global_import(out, b"failure", b"i32"));
    attempt!(global_import(out, b"quota_reason", b"i32"));
    attempt!(global_import(out, b"phase", b"i32"));
    attempt!(global_import(out, b"heap_cursor", b"i32"));
    attempt!(global_import(out, b"heap_baseline", b"i32"));
    attempt!(global_import(out, b"allocation_limit", b"i32"));
    attempt!(global_import(out, b"recipe_limit", b"i32"));
    attempt!(global_import(out, b"depth_limit", b"i32"));
    attempt!(global_import(out, b"operand_limit", b"i32"));
    attempt!(global_import(out, b"continuation_limit", b"i32"));
    attempt!(global_import(out, b"step_limit", b"i32"));
    attempt!(global_import(out, b"allocation_peak", b"i32"));
    attempt!(global_import(out, b"operand_peak", b"i32"));
    attempt!(global_import(out, b"continuation_peak", b"i32"));
    attempt!(global_import(out, b"steps", b"i32"));
    attempt!(global_import(out, b"quote_invocations", b"i32"));
    attempt!(global_import(out, b"sp", b"i32"));
    attempt!(global_import(out, b"cp", b"i32"));
    attempt!(global_import(out, b"observed_program", b"i32"));
    attempt!(global_import(out, b"recipe_count", b"i32"));
    attempt!(global_import(out, b"reflection_steps", b"i32"));
    attempt!(global_import(out, b"rp", b"i32"));
    global_import(out, b"generation", b"i32")
}

fn global_import(
    out: &mut crate::output::Buffer,
    name: &[u8],
    ty: &[u8],
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(import \"noble\" \""));
    attempt!(out.append(name));
    attempt!(out.append(b"\" (global $"));
    attempt!(out.append(name));
    attempt!(out.append(b" (mut "));
    attempt!(out.append(ty));
    out.append(b")))\n")
}

fn fragment(out: &mut crate::output::Buffer, text: &str) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(text.as_bytes()));
    out.append(b"\n")
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; immutable manifest-root runtime fragments are appended in their required order to the private bounded sink; output exhaustion propagates a diagnostic and no producer invariant needs an assertion."
)]
pub(super) fn runtime(
    out: &mut crate::output::Buffer,
    plan: &super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    attempt!(fragment(
        out,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/runtime/accounting.wat"
        ))
    ));
    attempt!(fragment(
        out,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/runtime/linear-storage.wat"
        ))
    ));
    attempt!(fragment(
        out,
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/stack.wat"))
    ));
    attempt!(fragment(
        out,
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/programs.wat"))
    ));
    attempt!(fragment(
        out,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/runtime/operations.wat"
        ))
    ));
    attempt!(fragment(
        out,
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/data.wat"))
    ));
    if plan.has_core_emit {
        attempt!(out.append(
            b"(func $op_emit (local $text i32)
 (local.set $text (i32.wrap_i64 (call $pop_kind (i32.const 11))))
 (if (global.get $failure) (then (return)))
 ;; A native host throw also leaves the session poisoned. Successful return is
 ;; the only place this provisional host-call failure may be cleared.
 (global.set $failure (i32.const 5))
 (if (i32.eqz (call $host_emit (call $x (local.get $text)) (call $y (local.get $text))))
  (then (global.set $failure (i32.const 0)))))
"
        ));
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
        attempt!(fragment(out, include_str!("nominal.wat")));
    }
    if plan.has_bound_emit {
        attempt!(fragment(out, include_str!("bound-emit.wat")));
    }
    if plan.has_bound_clock {
        attempt!(out.append(b"(func $op_clock_bound (param $slot i32) (local $status i32) (local $value i64)\n (global.set $failure (i32.const 5))\n (call $host_clock_bound (local.get $slot))\n (local.set $value)\n (local.set $status)\n (if (i32.eqz (local.get $status)) (then\n   (global.set $failure (i32.const 0))\n   (call $push_i64 (local.get $value)))))\n"));
    }
    if plan.has_live_propose {
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
        attempt!(out.append(b"(func $op_live_generation (local $current i64)
 (if (global.get $failure) (then (return)))
 (global.set $failure (i32.const 5))
 (local.set $current (call $host_live_generation))
 (global.set $failure (i32.const 0))
 (call $push_i64 (local.get $current)))
"));
    }
    attempt!(super::reflection::write(out, plan.declared_modules));
    attempt!(fragment(
        out,
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/source.wat"))
    ));
    Ok(())
}
