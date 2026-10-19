#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; emission mutates only the fresh private output sink; checked plans and caller inputs remain immutable and no partial output escapes preparation."
)]

pub(super) fn write(
    out: &mut crate::output::Buffer,
    plan: &super::super::plan::Layout,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(module\n(type $entry (func (param i32)))\n"));
    if plan.live_slots {
        attempt!(out.append(b"(import \"noble\" \"memory\" (memory 20 20))\n"));
    } else if plan.calculator {
        attempt!(out.append(b"(import \"noble\" \"memory\" (memory 16 64))\n"));
    } else {
        attempt!(out.append(b"(import \"noble\" \"memory\" (memory 16 16))\n"));
    }
    attempt!(out.append(b"(import \"noble\" \"table\" (table 16384 16384 funcref))\n"));
    if plan.live_slots {
        attempt!(out.append(
            b"(import \"noble\" \"live_select\" (func $host_live_select (param i32 i32) (result i32)))\n\
(import \"noble\" \"live_bind_validate\" (func $host_live_bind_validate (param i32 i32) (result i32)))\n\
(import \"noble\" \"live_frame_enter\" (func $host_live_frame_enter (param i32 i32 i32) (result i32)))\n\
(import \"noble\" \"live_frame_leave\" (func $host_live_frame_leave (param i32) (result i32)))\n\
(import \"noble\" \"resource_validate\" (func $host_resource_validate (param i32 i32) (result i32)))\n\
(import \"noble\" \"resource_bind_validate\" (func $host_resource_bind_validate (param i32 i32 i32 i32 i32 i32) (result i32)))\n"
        ));
    }
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
        attempt!(out.append(
            b"(import \"noble\" \"live_generation\" (func $host_live_generation (result i64)))\n"
        ));
    }
    attempt!(globals(out));
    if plan.calculator {
        // Immutable and per instance: the compiler generation this module was
        // validated against, bound by the engine at instantiation.
        attempt!(out
            .append(b"(import \"noble\" \"submit_generation\" (global $submit_generation i32))\n"));
    }
    attempt!(global_import(out, b"allocated_total", b"i64"));
    attempt!(global_import(out, b"released_total", b"i64"));
    if plan.live_slots {
        attempt!(global_import(out, b"live_target", b"i32"));
        attempt!(global_import(out, b"live_frame", b"i32"));
        attempt!(out.append(b"(global $live_module_installed (mut i32) (i32.const 0))\n"));
    }
    attempt!(out.append(
        b"(export \"memory\" (memory 0))\n(global $source_reflection i32 (i32.const 1))\n"
    ));
    if plan.calculator {
        out.append(b"(global $cell_limit i32 (i32.const 72362))\n(global $allocation_ceiling i32 (i32.const 1376208))\n")
    } else {
        out.append(b"(global $cell_limit i32 (i32.const 4096))\n(global $allocation_ceiling i32 (i32.const 196608))\n")
    }
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
#[path = "imports/entry.rs"]
pub(in crate::source::emit) mod entry;
