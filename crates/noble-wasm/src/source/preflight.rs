//! Bounded path-only tree inspection before the kernel's owned clone boundary.
//! Paths contain child indices, never borrowed nodes or recursively cloned input.

#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; validation mutates only preparation-owned work accounting and fresh bounded scratch buffers; the borrowed submission and published compiler remain unchanged."
)]

const STACK_LIMIT: usize = 128;
const TYPE_LIMIT: u32 = 512;
const PATH_LIMIT: usize = 64;

mod concrete;
mod schemes;

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; borrowed preflight checks every candidate, quotation and binding bound before owned kernel cloning; oversized payloads and metered structural walks must return Exhausted, not assert on input."
)]
fn body(
    body: &noble_kernel::execution::Body,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    if body.candidate.nodes.len() > super::NODE_LIMIT
        || body.candidate.body.len() > super::NODE_LIMIT
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    attempt!(work.entries(body.candidate.body.len()));
    let mut index = 0usize;
    let mut failure = None;
    while index < body.candidate.nodes.len() {
        match node(&body.candidate.nodes[index], limits, work) {
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
    reason = "Owner: noble-maintainers; each borrowed node checks quotation and binding bounds before metered binding inspection; the first input failure is returned without asserting on producer data."
)]
fn node(
    node: &noble_kernel::untrusted::Node,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    let inst = match node {
        noble_kernel::untrusted::Node::Literal { inst, .. }
        | noble_kernel::untrusted::Node::Invocation { inst, .. } => inst,
        noble_kernel::untrusted::Node::Quotation { inst, body } => {
            if body.len() > super::NODE_LIMIT {
                return Err(crate::Diagnostic::Exhausted);
            }
            attempt!(work.entries(body.len()));
            inst
        }
    };
    if inst.bindings.len() > 8 {
        return Err(crate::Diagnostic::Exhausted);
    }
    attempt!(work.entries(inst.bindings.len().saturating_add(1)));
    let mut index = 0usize;
    let mut failure = None;
    while index < inst.bindings.len() {
        match binding(&inst.bindings[index], limits, work) {
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
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; borrowed binding inspection uses the non-const effect accessor and allocates bounded path scratch for metered structural type walks."
)]
fn binding(
    binding: &noble_kernel::words::Binding,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    match binding {
        noble_kernel::words::Binding::Stack(value) => concrete::stack(value, limits, work),
        noble_kernel::words::Binding::Value(value) => concrete::ty(value, limits.type_size, work),
        noble_kernel::words::Binding::Effect(value) => {
            if value.as_slice().len() > 2 {
                Err(crate::Diagnostic::Exhausted)
            } else {
                Ok(())
            }
        }
        noble_kernel::words::Binding::Ref(_) => Ok(()),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; effect slice access and the metered borrowed stack traversal are non-const operations; this helper preserves their diagnostic ordering before kernel cloning."
)]
fn expected(
    expected: &noble_kernel::untrusted::Expected,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    if expected.allowed_effects.as_slice().len() > 2 {
        return Err(crate::Diagnostic::Exhausted);
    }
    attempt!(concrete::stack(&expected.stack_in, limits, work));
    concrete::stack(&expected.stack_out, limits, work)
}

fn definition(
    definition: &noble_kernel::execution::Definition,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    attempt!(expected(&definition.expected, limits, work));
    body(&definition.body, limits, work)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; bounded environment schemes, expected interfaces and every body are inspected with one shared meter before any recursive clone; oversized untrusted submissions return diagnostics rather than panic."
)]
pub(super) fn check(
    submission: &noble_kernel::execution::Submission,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    if submission.environment.defs.len() > super::DEFINITION_LIMIT.saturating_add(24)
        || submission.definitions.len() > super::DEFINITION_LIMIT
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    let mut index = 0usize;
    let mut failure = None;
    while index < submission.environment.defs.len() {
        match schemes::check(&submission.environment.defs[index], work) {
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
    attempt!(expected(
        &submission.request.expected,
        submission.request.limits,
        work
    ));
    attempt!(body(&submission.body, submission.request.limits, work));
    index = 0;
    while index < submission.definitions.len() {
        match definition(
            &submission.definitions[index],
            submission.request.limits,
            work,
        ) {
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
