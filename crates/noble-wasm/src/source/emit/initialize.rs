#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; emission mutates only the fresh private output sink; checked plans and caller inputs remain immutable and no partial output escapes preparation."
)]

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; every closed Action variant must select its exact immutable recipe atom rather than a fallback; writing the allocating bounded sink cannot be const and output failures propagate diagnostics."
)]
fn atom(
    out: &mut crate::output::Buffer,
    operation: &super::super::plan::Operation,
    witness: u32,
) -> Result<(), crate::Diagnostic> {
    let action = operation.action;
    let (tag, value, child) = match action {
        super::super::plan::Action::I64(value) => (1, value, None),
        super::super::plan::Action::Bool(value) => (4, i64::from(value), None),
        super::super::plan::Action::Unit => (5, 0, None),
        super::super::plan::Action::Program(child) => (3, 0, Some(child)),
        super::super::plan::Action::Word(definition) => (2, i64::from(definition), None),
        super::super::plan::Action::Quote(_, _, _) => (2, 8, None),
        super::super::plan::Action::Call(_, identity) => {
            (15, i64::from_le_bytes(identity.to_le_bytes()), None)
        }
        super::super::plan::Action::Text(address, length) => {
            return text_atom(out, (address, length), witness);
        }
    };
    attempt!(out.append(b"(local.set $atom (call $new (i32.const 8) "));
    attempt!(out.i64(value));
    attempt!(out.append(b" "));
    match child {
        Some(child) => attempt!(super::get_global(out, child)),
        None => attempt!(out.i32(0)),
    }
    attempt!(out.append(b" (i32.const 0) (i32.const 0) "));
    attempt!(out.i32(tag));
    if tag == 2 || tag == 15 {
        attempt!(out.append(b" "));
        attempt!(out.i32(operation.input));
        attempt!(out.append(b" "));
        attempt!(out.i32(operation.output));
        attempt!(out.append(b" "));
        attempt!(out.i32(operation.effects));
    } else {
        attempt!(out.append(b" (i32.const 0) (i32.const 0) (i32.const 0)"));
    }
    attempt!(out.append(b" (i32.const 0)"));
    out.append(b"))\n")
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the checked layout contains bounded table/data spans and topologically ordered programs; initialization emits runtime quota checks in source order while sink exhaustion returns a diagnostic, not an assertion."
)]
pub(super) fn write(
    out: &mut crate::output::Buffer,
    plan: &super::super::plan::Layout,
    generation: u32,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(func $initialize (local $recipe i32) (local $atom i32)\n"));
    if generation == 0 {
        attempt!(out.append(b"(table.init $core (i32.const 0) (i32.const 0) (i32.const 4))\n"));
    }
    if plan.functions > plan.first_function {
        attempt!(out.append(b"(table.init $code "));
        attempt!(out.i32(plan.first_function));
        attempt!(out.append(b" (i32.const 0) "));
        attempt!(out.i32(plan.functions.saturating_sub(plan.first_function)));
        attempt!(out.append(b")\n"));
    }
    let mut segment = 0usize;
    let mut failure = None;
    while segment < plan.data.len() {
        match memory_segment(out, plan, segment) {
            Ok(()) => segment += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    let mut at = 0usize;
    let mut failure = None;
    while at < plan.order.len() {
        match program(out, plan, plan.order[at]) {
            Ok(()) => at += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => out.append(b")\n"),
    }
}

fn memory_segment(
    out: &mut crate::output::Buffer,
    plan: &super::super::plan::Layout,
    segment: usize,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(if (i32.eqz (call $runtime_tick)) (then (return)))\n(memory.init $d"));
    attempt!(out.index(segment));
    attempt!(out.append(b" "));
    attempt!(out.i32(plan.data[segment].0));
    attempt!(out.append(b" (i32.const 0) "));
    attempt!(out.i32(attempt!(super::super::plan::number(
        plan.data[segment].1.len()
    ))));
    out.append(b")\n")
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the admitted program is topologically ordered and bounded before emission; recipe quota checks precede construction and the private sink propagates exhaustion rather than asserting on input."
)]
fn program(
    out: &mut crate::output::Buffer,
    plan: &super::super::plan::Layout,
    id: usize,
) -> Result<(), crate::Diagnostic> {
    let program = &plan.programs[id];
    attempt!(out.append(b"(if (i32.eqz (call $runtime_tick)) (then (return)))\n"));
    // Charge semantic size before any recipe construction for this body.
    attempt!(out.append(b"(if (i32.gt_u "));
    attempt!(out.i32(program.leaves));
    attempt!(
        out.append(b" (global.get $recipe_limit)) (then (call $quota (i32.const 2)) (return)))\n")
    );
    attempt!(out.append(b"(if (i32.gt_u "));
    attempt!(out.i32(program.depth));
    attempt!(out.append(b" (global.get $depth_limit)) (then (call $quota (i32.const 3)) (return)))\n(local.set $recipe (i32.const 0))\n"));
    let mut index = 0usize;
    let mut failure = None;
    while index < program.operations.len() {
        match operation(out, &program.operations[index], plan.text_witness) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    attempt!(out.append(b"(global.set "));
    attempt!(super::global(out, id));
    attempt!(out.append(b" (call $program "));
    attempt!(out.i32(program.entry));
    attempt!(out.append(b" "));
    attempt!(out.i32(program.input));
    attempt!(out.append(b" "));
    attempt!(out.i32(program.output));
    attempt!(out.append(b" (local.get $recipe) "));
    attempt!(out.i64(i64::from(program.effects)));
    attempt!(out.append(b" (i32.const 0) (i32.const 0) "));
    attempt!(out.i32(program.depth));
    attempt!(out.append(b" "));
    attempt!(out.i32(program.leaves));
    out.append(b"))\n(if (global.get $failure) (then (return)))\n")
}

fn operation(
    out: &mut crate::output::Buffer,
    operation: &super::super::plan::Operation,
    witness: u32,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(if (i32.eqz (call $runtime_tick)) (then (return)))\n"));
    attempt!(atom(out, operation, witness));
    out.append(b"(if (global.get $failure) (then (return)))\n(local.set $recipe (call $concat_recipe (local.get $recipe) (local.get $atom)))\n(if (global.get $failure) (then (return)))\n")
}

fn text_atom(
    out: &mut crate::output::Buffer,
    text: (u32, u32),
    witness: u32,
) -> Result<(), crate::Diagnostic> {
    let (address_bytes, length_bytes) = text;
    attempt!(out.append(b"(local.set $atom (call $new (i32.const 8) (i64.const 0) (call $text "));
    attempt!(out.i32(address_bytes));
    attempt!(out.append(b" "));
    attempt!(out.i32(length_bytes));
    attempt!(out.append(b") (i32.const 0) (i32.const 0) (i32.const 8) "));
    attempt!(out.i32(witness));
    out.append(b" (i32.const 0) (i32.const 0) (i32.const 0)))\n")
}
