#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; initialization serializes into the private fresh emission sink; checked plan and candidate are read-only and no host state is observed."
)]

pub(crate) fn write(
    out: &mut crate::output::Buffer,
    candidate: &noble_kernel::untrusted::Candidate,
    plan: &crate::lowering::Plan,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(func $init_programs (local $recipe i32) (local $atom i32)\n"));
    attempt!(failure_return(out));
    let mut index = 0usize;
    let mut failure = None;
    while index < plan.programs.len() {
        match initialize_program(out, candidate, plan, &plan.programs[index]) {
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
    out.append(b")\n")
}

fn initialize_program(
    out: &mut crate::output::Buffer,
    candidate: &noble_kernel::untrusted::Candidate,
    plan: &crate::lowering::Plan,
    program: &crate::lowering::Program,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(local.set $recipe (i32.const 0))\n"));
    let body = attempt!(crate::lowering::topology::body(candidate, program.owner));
    attempt!(body_recipe(out, body, plan));
    attempt!(create_program(out, program));
    failure_return(out)
}

fn body_recipe(
    out: &mut crate::output::Buffer,
    body: &[noble_kernel::untrusted::NodeId],
    plan: &crate::lowering::Plan,
) -> Result<(), crate::Diagnostic> {
    let mut position = 0usize;
    let mut failure = None;
    while position < body.len() {
        match recipe_atom(out, plan, body[position], position == 0) {
            Ok(()) => position += 1,
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

fn recipe_atom(
    out: &mut crate::output::Buffer,
    plan: &crate::lowering::Plan,
    node: noble_kernel::untrusted::NodeId,
    first: bool,
) -> Result<(), crate::Diagnostic> {
    let operation = attempt!(plan.operation(node));
    attempt!(atom(out, operation, node.0));
    attempt!(failure_return(out));
    if first {
        attempt!(out.append(b"(local.set $recipe (local.get $atom))\n"));
    } else {
        attempt!(out.append(
            b"(local.set $recipe (call $concat_recipe (local.get $recipe) (local.get $atom)))\n"
        ));
        attempt!(failure_return(out));
    }
    Ok(())
}

fn create_program(
    out: &mut crate::output::Buffer,
    program: &crate::lowering::Program,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(global.set "));
    attempt!(out.program_global(program.owner));
    attempt!(out.append(b" (call $program "));
    attempt!(out.i32(program.entry));
    attempt!(out.append(b" "));
    attempt!(out.i32(program.interface.input));
    attempt!(out.append(b" "));
    attempt!(out.i32(program.interface.output));
    out.append(b" (local.get $recipe) (i64.const 0) (i32.const 0) (i32.const 0) (i32.const 1) (i32.const 1)))\n")
}

fn failure_return(out: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
    out.append(b"(if (global.get $failure) (then (return)))\n")
}

/// Recipes preserve the source body's atoms in order. A Program literal names
/// an already initialized child, independently of its executable entry index.
#[expect(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; each operation requires its exact semantic recipe atom, so new variants must force review. Writing the allocating sink cannot be const and its bounded failures propagate as diagnostics."
)]
fn atom(
    out: &mut crate::output::Buffer,
    operation: crate::lowering::Operation,
    node: u32,
) -> Result<(), crate::Diagnostic> {
    let (kind, payload, child) = match operation {
        crate::lowering::Operation::I64(value) => (1, value, None),
        crate::lowering::Operation::Bool(value) => (4, if value { 1 } else { 0 }, None),
        crate::lowering::Operation::Unit => (5, 0, None),
        crate::lowering::Operation::Program(_) => (3, 0, Some(node)),
        crate::lowering::Operation::Word(definition) => (2, i64::from(definition), None),
        crate::lowering::Operation::Quote(_) => (2, 8, None),
    };
    attempt!(out.append(b"(local.set $atom (call $atom "));
    attempt!(out.i32(kind));
    attempt!(out.append(b" "));
    attempt!(out.i64(payload));
    attempt!(out.append(b" "));
    match child {
        Some(child) => {
            attempt!(out.append(b"(global.get "));
            attempt!(out.program_global(Some(child)));
            attempt!(out.append(b")"));
        }
        None => attempt!(out.i32(0)),
    }
    out.append(b"))\n")
}
