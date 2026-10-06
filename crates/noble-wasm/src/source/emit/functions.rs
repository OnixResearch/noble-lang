#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; emission mutates only the fresh private output sink; checked plans and caller inputs remain immutable and no partial output escapes preparation."
)]

pub(super) fn write(
    out: &mut crate::output::Buffer,
    program: &super::super::plan::Program,
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    let mut failure = None;
    while index < program.operations.len() {
        match function(out, program, index) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; allocation bounds the program's complete function range, while non-const checked numbering and allocating sink writes preserve overflow and output-exhaustion diagnostics rather than assertions."
)]
fn function(
    out: &mut crate::output::Buffer,
    program: &super::super::plan::Program,
    index: usize,
) -> Result<(), crate::Diagnostic> {
    let function = match program
        .entry
        .checked_add(attempt!(super::super::plan::number(index)))
    {
        Some(value) => value,
        None => return Err(crate::Diagnostic::Exhausted),
    };
    attempt!(out.append(b"(func $f"));
    attempt!(out.number(u64::from(function)));
    attempt!(out
        .append(b" (type $entry) (param $env i32)\n(if (global.get $failure) (then (return)))\n"));
    if index.saturating_add(1) < program.operations.len() {
        attempt!(out.append(b"(call $enqueue "));
        let next_function = match function.checked_add(1) {
            Some(value) => value,
            None => return Err(crate::Diagnostic::Exhausted),
        };
        attempt!(out.i32(next_function));
        attempt!(out.append(b" (local.get $env))\n"));
    }
    attempt!(out.append(b"(if (i32.eqz (global.get $failure)) (then\n"));
    attempt!(instruction(out, program.operations[index].action));
    out.append(b"))\n)\n")
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; each closed Action variant requires its exact native instruction path, so new variants must force review; bounded sink output failures remain diagnostics."
)]
fn instruction(
    out: &mut crate::output::Buffer,
    action: super::super::plan::Action,
) -> Result<(), crate::Diagnostic> {
    attempt!(match action {
        super::super::plan::Action::I64(value) => push_integer(out, value),
        super::super::plan::Action::Bool(value) => push_boolean(out, value),
        super::super::plan::Action::Unit => out.append(b"(call $push_unit"),
        super::super::plan::Action::Text(address, length) => push_text(
            out,
            TextSpan {
                address,
                length_bytes: length,
            },
        ),
        super::super::plan::Action::Program(program) => push_program(out, program),
        super::super::plan::Action::Word(definition) => word_call(out, definition),
        super::super::plan::Action::TextByte => out.append(b"(call $op_text_byte"),
        super::super::plan::Action::Quote(input, output_signature, witness) => typed_quote(
            out,
            QuoteOperands {
                input,
                output_signature,
                witness,
            },
        ),
        super::super::plan::Action::Call(program, _) => enqueue_program(out, program),
        super::super::plan::Action::NominalNew(module, ordinal, ty) => nominal_construct(
            out,
            noble_kernel::types::NominalTypeId { module, ordinal },
            ty,
            b" (i32.const 0)",
        ),
        super::super::plan::Action::NominalInto(module, ordinal, ty) => nominal_into(
            out,
            noble_kernel::types::NominalTypeId { module, ordinal },
            ty,
        ),
        super::super::plan::Action::NominalLeft(module, ordinal, ty) => nominal_construct(
            out,
            noble_kernel::types::NominalTypeId { module, ordinal },
            ty,
            b" (i32.const 1)",
        ),
        super::super::plan::Action::NominalRight(module, ordinal, ty) => nominal_construct(
            out,
            noble_kernel::types::NominalTypeId { module, ordinal },
            ty,
            b" (i32.const 2)",
        ),
        super::super::plan::Action::NominalMatch(module, ordinal, nominal, left, right) =>
            nominal_match(
                out,
                noble_kernel::types::NominalTypeId { module, ordinal },
                MatchShapes {
                    nominal,
                    left,
                    right,
                },
            ),
        super::super::plan::Action::EmitBound(slot) => bound_call(out, slot),
        super::super::plan::Action::ClockBound(slot) => clock_call(out, slot),
        super::super::plan::Action::LivePropose(owner) => {
            attempt!(out.append(b"(call $op_live_propose "));
            out.i64(owner as i64)
        }
        super::super::plan::Action::LiveGeneration => out.append(b"(call $op_live_generation"),
        super::super::plan::Action::SlotInvoke(site, _) => {
            attempt!(out.append(b"(call $slot_site_"));
            out.number(u64::from(site))
        }
    });
    out.append(b")\n")
}

struct TextSpan {
    address: u32,
    length_bytes: u32,
}

fn push_integer(out: &mut crate::output::Buffer, value: i64) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call $push_i64 "));
    out.i64(value)
}

fn push_boolean(out: &mut crate::output::Buffer, value: bool) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call $push_bool "));
    out.i32(u32::from(value))
}

fn push_text(out: &mut crate::output::Buffer, span: TextSpan) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call $push_ref (call $text "));
    attempt!(out.i32(span.address));
    attempt!(out.append(b" "));
    attempt!(out.i32(span.length_bytes));
    out.append(b")")
}

fn push_program(out: &mut crate::output::Buffer, program: usize) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call $push_ref "));
    super::get_global(out, program)
}

fn word_call(out: &mut crate::output::Buffer, definition: u32) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call "));
    crate::operations::word(out, definition)
}

struct QuoteOperands {
    input: u32,
    output_signature: u32,
    witness: u32,
}

fn typed_quote(
    out: &mut crate::output::Buffer,
    operands: QuoteOperands,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call $op_quote_typed "));
    attempt!(out.i32(operands.input));
    attempt!(out.append(b" "));
    attempt!(out.i32(operands.output_signature));
    attempt!(out.append(b" "));
    out.i32(operands.witness)
}

fn enqueue_program(
    out: &mut crate::output::Buffer,
    program: usize,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call $enqueue_program "));
    super::get_global(out, program)
}

fn bound_call(out: &mut crate::output::Buffer, slot: u32) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call $op_emit_bound "));
    out.i32(slot)
}

fn clock_call(out: &mut crate::output::Buffer, slot: u32) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call $op_clock_bound "));
    out.i32(slot)
}

fn nominal_construct(
    out: &mut crate::output::Buffer,
    id: noble_kernel::types::NominalTypeId,
    shape: u32,
    branch: &[u8],
) -> Result<(), crate::Diagnostic> {
    attempt!(checked_slot(out, SlotGuard { shape, depth: 1 }));
    attempt!(nominal_call(out, b"$op_nominal_construct", id));
    out.append(branch)
}

fn nominal_into(
    out: &mut crate::output::Buffer,
    id: noble_kernel::types::NominalTypeId,
    shape: u32,
) -> Result<(), crate::Diagnostic> {
    attempt!(checked_slot(out, SlotGuard { shape, depth: 1 }));
    nominal_call(out, b"$op_nominal_into", id)
}

struct MatchShapes {
    nominal: u32,
    left: u32,
    right: u32,
}

fn nominal_match(
    out: &mut crate::output::Buffer,
    id: noble_kernel::types::NominalTypeId,
    shapes: MatchShapes,
) -> Result<(), crate::Diagnostic> {
    let right = SlotGuard {
        shape: shapes.right,
        depth: 1,
    };
    attempt!(checked_slot(out, right));
    let left = SlotGuard {
        shape: shapes.left,
        depth: 2,
    };
    attempt!(checked_slot(out, left));
    let nominal = SlotGuard {
        shape: shapes.nominal,
        depth: 3,
    };
    attempt!(checked_slot(out, nominal));
    nominal_call(out, b"$op_nominal_match", id)
}

fn nominal_call(
    out: &mut crate::output::Buffer,
    function: &[u8],
    id: noble_kernel::types::NominalTypeId,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call "));
    attempt!(out.append(function));
    attempt!(out.append(b" "));
    attempt!(out.i64(id.module as i64));
    attempt!(out.append(b" "));
    out.i32(id.ordinal)
}

struct SlotGuard {
    shape: u32,
    depth: u32,
}

fn checked_slot(
    out: &mut crate::output::Buffer,
    guard: SlotGuard,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(if (i32.eqz (call $need "));
    attempt!(out.i32(guard.depth));
    attempt!(out.append(b")) (then (return)))\n(if (i32.eqz (call $t"));
    attempt!(out.number(u64::from(guard.shape)));
    attempt!(out.append(b" (call $slot_kind (i32.sub (global.get $sp) "));
    attempt!(out.i32(guard.depth));
    attempt!(out.append(b")) (call $slot_value (i32.sub (global.get $sp) "));
    attempt!(out.i32(guard.depth));
    out.append(b")))) (then (call $fail (i32.const 4)) (return)))\n")
}
