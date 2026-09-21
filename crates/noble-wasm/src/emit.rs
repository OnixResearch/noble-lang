#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; emission helpers mutate only the fresh sink allocated by module, not caller-owned candidate or plan state; no mutable capability or partial output escapes compile."
)]

mod initialization;

// Keep textual assets as strings: Aeneas can retain their literal UTF-8 model
// instead of elaborating tens of thousands of individual scalar array entries.
// as_bytes borrows exactly those static bytes without copying or allocating.
const COMMON: &str = include_str!("../runtime/common.wat");
const ACCOUNTING: &str = include_str!("../runtime/accounting.wat");
const GC_STORAGE: &str = include_str!("../runtime/gc-storage.wat");
const LINEAR_STORAGE: &str = include_str!("../runtime/linear-storage.wat");
const STACK: &str = include_str!("../runtime/stack.wat");
const PROGRAMS: &str = include_str!("../runtime/programs.wat");
const OPERATIONS: &str = include_str!("../runtime/operations.wat");
const REFLECTION: &str = include_str!("../runtime/reflection.wat");
const EXPERIMENT: &str = include_str!("../runtime/experiment.wat");

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; every representation must select its real storage implementation explicitly. Fallible bounded emission propagates diagnostics and cannot substitute assertions or a default representation."
)]
pub(crate) fn module(
    representation: crate::Representation,
    candidate: &noble_kernel::untrusted::Candidate,
    plan: &crate::lowering::Plan,
) -> Result<alloc::vec::Vec<u8>, crate::Diagnostic> {
    let mut out = crate::output::Buffer::new(4096);
    attempt!(out.append(b"(module\n"));
    attempt!(fragment(&mut out, COMMON));
    attempt!(fragment(&mut out, ACCOUNTING));
    match representation {
        crate::Representation::WasmGc => attempt!(fragment(&mut out, GC_STORAGE)),
        crate::Representation::ManagedLinearMemory => attempt!(fragment(&mut out, LINEAR_STORAGE)),
    }
    attempt!(fragment(&mut out, STACK));
    attempt!(fragment(&mut out, PROGRAMS));
    attempt!(fragment(&mut out, OPERATIONS));
    attempt!(fragment(&mut out, REFLECTION));
    attempt!(fragment(&mut out, EXPERIMENT));
    attempt!(plan.signatures.emit(&mut out));
    attempt!(declarations(&mut out, plan));
    attempt!(functions(&mut out, candidate, plan));
    attempt!(initialization::write(&mut out, candidate, plan));
    attempt!(out.append(b")\n"));
    Ok(out.finish())
}

fn fragment(out: &mut crate::output::Buffer, source: &str) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(source.as_bytes()));
    out.append(b"\n")
}

fn constant(
    out: &mut crate::output::Buffer,
    name: &[u8],
    value: u32,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(global "));
    attempt!(out.append(name));
    attempt!(out.append(b" i32 "));
    attempt!(out.i32(value));
    out.append(b")\n")
}

fn declarations(
    out: &mut crate::output::Buffer,
    plan: &crate::lowering::Plan,
) -> Result<(), crate::Diagnostic> {
    attempt!(constant(out, b"$registry_count", plan.registry_slots));
    attempt!(constant(out, b"$sig_empty", plan.host_signatures[0]));
    attempt!(constant(out, b"$sig_i64", plan.host_signatures[1]));
    attempt!(constant(out, b"$sig_i64_i64", plan.host_signatures[2]));
    attempt!(program_globals(out, plan));
    attempt!(out.append(b"(table "));
    attempt!(out.number(u64::from(plan.functions)));
    attempt!(out.append(b" "));
    attempt!(out.number(u64::from(plan.functions)));
    attempt!(out.append(b" funcref)\n(elem (i32.const 0) $empty_entry $quote_entry $compose_entry"));
    table_entries(out, plan.functions)
}

fn program_globals(
    out: &mut crate::output::Buffer,
    plan: &crate::lowering::Plan,
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    while index < plan.programs.len() {
        attempt!(out.append(b"(global "));
        attempt!(out.program_global(plan.programs[index].owner));
        attempt!(out.append(b" (mut i32) (i32.const 0))\n"));
        index += 1;
    }
    Ok(())
}

fn table_entries(out: &mut crate::output::Buffer, count: u32) -> Result<(), crate::Diagnostic> {
    let mut function = 3u32;
    while function < count {
        attempt!(out.append(b" $f"));
        attempt!(out.number(u64::from(function)));
        function += 1;
    }
    out.append(b")\n")
}

fn functions(
    out: &mut crate::output::Buffer,
    candidate: &noble_kernel::untrusted::Candidate,
    plan: &crate::lowering::Plan,
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    while index < plan.programs.len() {
        attempt!(body_functions(out, candidate, plan, &plan.programs[index]));
        index += 1;
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the admitted plan bounds entries by FUNCTION_LIMIT=1024 and positions by BODY_OPERATION_LIMIT=128, so increments fit. Missing nodes and output limits propagate diagnostics instead of assertions, and failure prevents later emission."
)]
fn body_functions(
    out: &mut crate::output::Buffer,
    candidate: &noble_kernel::untrusted::Candidate,
    plan: &crate::lowering::Plan,
    program: &crate::lowering::Program,
) -> Result<(), crate::Diagnostic> {
    let body = attempt!(crate::lowering::topology::body(candidate, program.owner));
    let mut position = 0usize;
    let mut function = program.entry;
    let mut failure = None;
    while position < body.len() && failure.is_none() {
        // The private plan bounds every table index below FUNCTION_LIMIT.
        let successor = function + 1;
        let next = if position + 1 < body.len() {
            Some(successor)
        } else {
            None
        };
        if let Err(error) = body_function(out, plan, body[position], function, next) {
            failure = Some(error);
        }
        position += 1;
        function = successor;
    }
    match failure {
        None => Ok(()),
        Some(error) => Err(error),
    }
}

fn body_function(
    out: &mut crate::output::Buffer,
    plan: &crate::lowering::Plan,
    node: noble_kernel::untrusted::NodeId,
    function: u32,
    next: Option<u32>,
) -> Result<(), crate::Diagnostic> {
    let operation = attempt!(plan.operation(node));
    crate::operations::continuation(out, function, next, operation, node)
}
