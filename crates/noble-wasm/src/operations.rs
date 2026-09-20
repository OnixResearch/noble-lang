#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; instruction writers mutate only the fresh private emission sink; semantic operation values are copied and caller inputs and host state are never changed."
)]

/// Definitions are resolved against the real bootstrap environment before this
/// mapping is used. These names select static native helpers, not recipe data.
pub(crate) const fn supported(definition: u32) -> bool {
    matches!(definition, 0..=2 | 4..=7 | 9..=14 | 18..=21)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; the selected fixed symbol is appended to an allocating Vec-backed sink through its bounded non-const append method."
)]
fn word(out: &mut crate::output::Buffer, definition: u32) -> Result<(), crate::Diagnostic> {
    match definition {
        0 => out.append(b"$op_dup"),
        1 => out.append(b"$op_drop"),
        2 => out.append(b"$op_swap"),
        4 => out.append(b"$op_add"),
        5 => out.append(b"$op_sub"),
        6 => out.append(b"$op_mul"),
        7 => out.append(b"$op_equals"),
        9 => out.append(b"$op_compose"),
        10 => out.append(b"$op_run"),
        11 => out.append(b"$op_reflect"),
        12 => out.append(b"$push_unit"),
        13 => out.append(b"$op_pair"),
        14 => out.append(b"$op_unpair"),
        18 => out.append(b"$op_if"),
        19 => out.append(b"$op_nil"),
        20 => out.append(b"$op_cons"),
        21 => out.append(b"$op_list_case"),
        _ => Err(crate::Diagnostic::Unsupported),
    }
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; every lowered operation requires an explicit native instruction path, not a fallback; bounded output failures return diagnostics and cannot be replaced by assertions."
)]
pub(crate) fn instruction(
    out: &mut crate::output::Buffer,
    operation: crate::lowering::Operation,
    node: noble_kernel::untrusted::NodeId,
) -> Result<(), crate::Diagnostic> {
    match operation {
        crate::lowering::Operation::I64(value) => {
            attempt!(out.append(b"(call $push_i64 "));
            attempt!(out.i64(value));
        }
        crate::lowering::Operation::Bool(value) => {
            attempt!(out.append(b"(call $push_bool "));
            attempt!(out.i32(if value { 1 } else { 0 }));
        }
        crate::lowering::Operation::Unit => attempt!(out.append(b"(call $push_unit")),
        crate::lowering::Operation::Program(_) => {
            attempt!(out.append(b"(call $push_ref (global.get "));
            attempt!(out.program_global(Some(node.0)));
            attempt!(out.append(b")"));
        }
        crate::lowering::Operation::Word(definition) => {
            attempt!(out.append(b"(call "));
            attempt!(word(out, definition));
        }
        crate::lowering::Operation::Quote(interface) => {
            attempt!(out.append(b"(call $op_quote "));
            attempt!(out.i32(interface.input));
            attempt!(out.append(b" "));
            attempt!(out.i32(interface.output));
        }
    }
    out.append(b")\n")
}

/// Saving the fixed caller continuation before the operation makes nested run,
/// if and list.case return to it. Enqueue failure suppresses the operation.
pub(crate) fn continuation(
    out: &mut crate::output::Buffer,
    function: u32,
    next: Option<u32>,
    operation: crate::lowering::Operation,
    node: noble_kernel::untrusted::NodeId,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(func $f"));
    attempt!(out.number(u64::from(function)));
    attempt!(out.append(b" (type $entry) (param $env i32)\n"));
    attempt!(out.append(b"(if (global.get $failure) (then (return)))\n"));
    if let Some(next) = next {
        attempt!(out.append(b"(call $enqueue "));
        attempt!(out.i32(next));
        attempt!(out.append(b" (local.get $env))\n"));
    }
    attempt!(out.append(b"(if (i32.eqz (global.get $failure)) (then\n"));
    attempt!(instruction(out, operation, node));
    out.append(b"))\n)\n")
}
