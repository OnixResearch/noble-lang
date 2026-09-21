#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; emission mutates only the fresh private output sink; checked plans and caller inputs remain immutable and no partial output escapes preparation."
)]

pub(super) fn write(out: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(module\n(type $entry (func (param i32)))\n(import \"noble\" \"memory\" (memory 16 16))\n(import \"noble\" \"table\" (table 16384 16384 funcref))\n(import \"noble\" \"test_emit\" (func $host_emit (param i32 i32) (result i32)))\n(import \"noble\" \"test_abort\" (func $host_abort (result i32)))\n"));
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
pub(super) fn runtime(out: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
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
    attempt!(fragment(
        out,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/runtime/reflection.wat"
        ))
    ));
    attempt!(fragment(
        out,
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/source.wat"))
    ));
    Ok(())
}
