const WORK_LIMIT: usize = 4096;

// Like the kernel's type walk, the queue owns its nodes. Aeneas cannot carry
// references into a type tree across an owned work stack. Clone each root
// once, then move every descendant rather than repeatedly cloning subtrees.
enum Step {
    Type(noble_kernel::types::Ty),
    Stack(alloc::vec::Vec<noble_kernel::types::Ty>),
    Effects(noble_kernel::types::EffSet),
    Byte(u8),
}

struct State {
    work: alloc::vec::Vec<Step>,
    out: crate::output::Buffer,
    fuel: usize,
    extended: bool,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the bounded owned worklist returns exhaustion or unsupported-type diagnostics, and never asserts on a caller-provided structural stack."
)]
pub(super) fn encode(
    stack: &[noble_kernel::types::Ty],
    extended: bool,
) -> Result<alloc::vec::Vec<u8>, crate::Diagnostic> {
    if stack.len() > WORK_LIMIT / 2 {
        return Err(crate::Diagnostic::Exhausted);
    }
    let mut walk = State {
        work: alloc::vec::Vec::with_capacity(16),
        out: crate::output::Buffer::new(64),
        fuel: WORK_LIMIT,
        extended,
    };
    walk.work.push(Step::Stack(copy_roots(stack)));
    let mut failure = None;
    while !walk.work.is_empty() && failure.is_none() {
        let (next, error) = advance(walk);
        walk = next;
        failure = error;
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(walk.out.finish()),
    }
}

fn copy_roots(stack: &[noble_kernel::types::Ty]) -> alloc::vec::Vec<noble_kernel::types::Ty> {
    let mut roots = alloc::vec::Vec::with_capacity(stack.len());
    let mut index = 0usize;
    while index < stack.len() {
        roots.push(stack[index].clone());
        index += 1;
    }
    roots
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; traversal consumes allocating Ty values and writes a Vec-backed sink. WORK_LIMIT is 4096, so its fixed six-slot reserve cannot underflow; zero fuel is rejected before decrement."
)]
fn advance(mut walk: State) -> (State, Option<crate::Diagnostic>) {
    if walk.fuel == 0 || walk.work.len() > WORK_LIMIT - 6 {
        return (walk, Some(crate::Diagnostic::Exhausted));
    }
    walk.fuel -= 1;
    let result = match walk.work.pop() {
        None => Ok(()),
        Some(Step::Byte(byte)) => walk.out.append(&[byte]),
        Some(Step::Effects(effects)) => emit_effects(&effects, &mut walk.out),
        Some(Step::Stack(entries)) => push_stack(entries, &mut walk.work, &mut walk.out),
        Some(Step::Type(ty)) => push_type(ty, &mut walk.work, &mut walk.out, walk.extended),
    };
    match result {
        Ok(()) => (walk, None),
        Err(error) => (walk, Some(error)),
    }
}

#[expect(
    tigerstyle::borrowed_argument_types,
    tigerstyle::mutating_input_in_pure,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; only the owned walk's work Vec and sink grow, requiring Vec rather than a slice. entries.len() <= 2048 bounds twice its length plus one to 4097; short-circuit rejection proves needed <= WORK_LIMIT before subtraction."
)]
fn push_stack(
    mut entries: alloc::vec::Vec<noble_kernel::types::Ty>,
    work: &mut alloc::vec::Vec<Step>,
    out: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    if entries.len() > WORK_LIMIT / 2 {
        return Err(crate::Diagnostic::Exhausted);
    }
    let needed = entries.len() * 2 + 1;
    if needed > WORK_LIMIT || work.len() > WORK_LIMIT - needed {
        return Err(crate::Diagnostic::Exhausted);
    }
    attempt!(out.append(b"["));
    work.push(Step::Byte(b']'));
    while let Some(ty) = entries.pop() {
        work.push(Step::Type(ty));
        if !entries.is_empty() {
            work.push(Step::Byte(b' '));
        }
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::borrowed_argument_types,
    tigerstyle::missing_const_fn,
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; this consumes owned type children, grows the walk's Vec and emits to its private sink; a slice cannot queue children and allocation cannot be const. Unsupported types/effects and output bounds remain diagnostic outcomes, not assertions."
)]
fn push_type(
    ty: noble_kernel::types::Ty,
    work: &mut alloc::vec::Vec<Step>,
    out: &mut crate::output::Buffer,
    extended: bool,
) -> Result<(), crate::Diagnostic> {
    if !extended {
        match &ty {
            noble_kernel::types::Ty::Text | noble_kernel::types::Ty::Sum(_, _) => {
                return Err(crate::Diagnostic::Unsupported);
            }
            noble_kernel::types::Ty::Contract
            | noble_kernel::types::Ty::Evidence
            | noble_kernel::types::Ty::Certified => {
                return Err(crate::Diagnostic::Unsupported);
            }
            noble_kernel::types::Ty::Program(_, _, effects) if !effects.is_empty() => {
                return Err(crate::Diagnostic::Unsupported);
            }
            _ => {}
        }
    }
    match ty {
        noble_kernel::types::Ty::Unit => attempt!(out.append(b"Unit")),
        noble_kernel::types::Ty::Bool => attempt!(out.append(b"Bool")),
        noble_kernel::types::Ty::I64 => attempt!(out.append(b"I64")),
        noble_kernel::types::Ty::Text => attempt!(out.append(b"Text")),
        noble_kernel::types::Ty::Syntax => attempt!(out.append(b"Syntax")),
        noble_kernel::types::Ty::Contract => attempt!(out.append(b"Contract")),
        noble_kernel::types::Ty::Evidence => attempt!(out.append(b"Evidence")),
        noble_kernel::types::Ty::Certified => attempt!(out.append(b"Certified")),
        noble_kernel::types::Ty::Resource(_) => return Err(crate::Diagnostic::Unsupported),
        noble_kernel::types::Ty::Pair(left, right) => {
            attempt!(out.append(b"Pair("));
            work.push(Step::Byte(b')'));
            work.push(Step::Type(*right));
            work.push(Step::Byte(b','));
            work.push(Step::Type(*left));
        }
        noble_kernel::types::Ty::List(item) => {
            attempt!(out.append(b"List("));
            work.push(Step::Byte(b')'));
            work.push(Step::Type(*item));
        }
        noble_kernel::types::Ty::Sum(left, right) => {
            attempt!(out.append(b"Sum("));
            work.push(Step::Byte(b')'));
            work.push(Step::Type(*right));
            work.push(Step::Byte(b','));
            work.push(Step::Type(*left));
        }
        noble_kernel::types::Ty::Program(input_stack, output_stack, effects) => {
            attempt!(out.append(b"Program("));
            work.push(Step::Byte(b')'));
            if !effects.is_empty() {
                work.push(Step::Effects(effects));
            }
            work.push(Step::Stack(*output_stack));
            work.push(Step::Byte(b'>'));
            work.push(Step::Byte(b'-'));
            work.push(Step::Stack(*input_stack));
        }
    }
    Ok(())
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; effect serialization writes only the walk's fresh owned emission sink, leaving caller-owned effects unchanged."
)]
fn emit_effects(
    effects: &noble_kernel::types::EffSet,
    out: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    // Empty effects retain the experimental signature spelling.
    if effects.is_empty() {
        return Ok(());
    }
    attempt!(out.append(b"!{"));
    let mut index = 0usize;
    while index < effects.as_slice().len() {
        if index != 0 {
            attempt!(out.append(b","));
        }
        attempt!(out.number(u64::from(effects.as_slice()[index].0)));
        index += 1;
    }
    out.append(b"}")
}
