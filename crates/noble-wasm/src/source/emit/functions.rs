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
    tigerstyle::missing_const_fn,
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; each closed Action variant requires its exact native instruction path, so new variants must force review; writing the allocating bounded sink cannot be const and output failures remain diagnostics."
)]
fn instruction(
    out: &mut crate::output::Buffer,
    action: super::super::plan::Action,
) -> Result<(), crate::Diagnostic> {
    match action {
        super::super::plan::Action::I64(value) => {
            attempt!(out.append(b"(call $push_i64 "));
            attempt!(out.i64(value));
        }
        super::super::plan::Action::Bool(value) => {
            attempt!(out.append(b"(call $push_bool "));
            attempt!(out.i32(if value { 1 } else { 0 }));
        }
        super::super::plan::Action::Unit => attempt!(out.append(b"(call $push_unit")),
        super::super::plan::Action::Text(address, length) => {
            attempt!(out.append(b"(call $push_ref (call $text "));
            attempt!(out.i32(address));
            attempt!(out.append(b" "));
            attempt!(out.i32(length));
            attempt!(out.append(b")"));
        }
        super::super::plan::Action::Program(program) => {
            attempt!(out.append(b"(call $push_ref "));
            attempt!(super::get_global(out, program));
        }
        super::super::plan::Action::Word(definition) => {
            attempt!(out.append(b"(call "));
            attempt!(crate::operations::word(out, definition));
        }
        super::super::plan::Action::Quote(input, output_signature, witness) => {
            attempt!(out.append(b"(call $op_quote_typed "));
            attempt!(out.i32(input));
            attempt!(out.append(b" "));
            attempt!(out.i32(output_signature));
            attempt!(out.append(b" "));
            attempt!(out.i32(witness));
        }
        super::super::plan::Action::Call(program, _) => {
            attempt!(out.append(b"(call $enqueue_program "));
            attempt!(super::get_global(out, program));
        }
    }
    out.append(b")\n")
}
