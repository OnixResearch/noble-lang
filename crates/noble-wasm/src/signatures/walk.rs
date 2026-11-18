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
    compact: bool,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the bounded owned worklist returns exhaustion or unsupported-type diagnostics, and never asserts on a caller-provided structural stack."
)]
pub(super) fn encode(
    stack: &[noble_kernel::types::Ty],
    extended: bool,
    compact: bool,
) -> Result<alloc::vec::Vec<u8>, crate::Diagnostic> {
    if stack.len() > WORK_LIMIT / 2 {
        return Err(crate::Diagnostic::Exhausted);
    }
    let mut walk = State {
        work: alloc::vec::Vec::with_capacity(16),
        out: crate::output::Buffer::new(64),
        fuel: WORK_LIMIT,
        extended,
        compact,
    };
    walk.work.push(Step::Stack(copy_roots(stack)));
    let mut failure = None;
    while !walk.work.is_empty() {
        if let Err(error) = advance(&mut walk) {
            failure = Some(error);
            break;
        }
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
    tigerstyle::mutating_input_in_pure,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; traversal mutates only the private owned worklist and Vec-backed sink. WORK_LIMIT is 4096, so its fixed six-slot reserve cannot underflow; zero fuel is rejected before decrement."
)]
fn advance(walk: &mut State) -> Result<(), crate::Diagnostic> {
    if walk.fuel == 0 || walk.work.len() > WORK_LIMIT - 6 {
        return Err(crate::Diagnostic::Exhausted);
    }
    walk.fuel -= 1;
    match walk.work.pop() {
        None => Ok(()),
        Some(Step::Byte(byte)) => walk.out.append(&[byte]),
        Some(Step::Effects(effects)) => effects::emit_bounded(&effects, &mut walk.out),
        Some(Step::Stack(entries)) => push_stack(entries, &mut walk.work, &mut walk.out),
        Some(Step::Type(ty)) => push_type(
            ty,
            &mut walk.work,
            &mut walk.out,
            walk.extended,
            walk.compact,
        ),
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
    tigerstyle::function_length,
    reason = "Owner: noble-wasm-maintainers; the selected lowering or preflight stage keeps its checked step order in one reviewable unit, and splitting it would separate the emitted sequence from its bounds checks."
)]
#[allow(
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
    compact: bool,
) -> Result<(), crate::Diagnostic> {
    if is_core_unavailable(&ty, extended) {
        return Err(crate::Diagnostic::Unsupported);
    }
    // Every arm finishes the step itself, so no arm rejoins another.
    match ty {
        noble_kernel::types::Ty::Unit => spell(out, compact, b"u", b"Unit"),
        noble_kernel::types::Ty::Bool => spell(out, compact, b"b", b"Bool"),
        noble_kernel::types::Ty::I64 => spell(out, compact, b"i", b"I64"),
        noble_kernel::types::Ty::Text => spell(out, compact, b"t", b"Text"),
        noble_kernel::types::Ty::Syntax => spell(out, compact, b"x", b"Syntax"),
        noble_kernel::types::Ty::Contract => spell(out, compact, b"c", b"Contract"),
        noble_kernel::types::Ty::Evidence => spell(out, compact, b"e", b"Evidence"),
        noble_kernel::types::Ty::Certified => spell(out, compact, b"f", b"Certified"),
        noble_kernel::types::Ty::Resource(kind) => {
            if !extended {
                return Err(crate::Diagnostic::Unsupported);
            }
            attempt!(spell(out, compact, b"R(", b"Resource("));
            attempt!(out.number(u64::from(kind.0)));
            out.append(b")")
        }
        noble_kernel::types::Ty::Nominal(id, shape) => {
            attempt!(spell(out, compact, b"N(", b"Nominal("));
            attempt!(out.number(id.module));
            attempt!(out.append(b":"));
            attempt!(out.number(u64::from(id.ordinal)));
            match *shape {
                noble_kernel::types::NominalShape::Opaque(value) => {
                    attempt!(spell(out, compact, b",O(", b",Opaque("));
                    work.push(Step::Byte(b')'));
                    work.push(Step::Byte(b')'));
                    work.push(Step::Type(*value));
                    Ok(())
                }
                noble_kernel::types::NominalShape::Variant(left, right) => {
                    attempt!(spell(out, compact, b",V(", b",Variant("));
                    work.push(Step::Byte(b')'));
                    work.push(Step::Byte(b')'));
                    work.push(Step::Type(*right));
                    work.push(Step::Byte(b','));
                    work.push(Step::Type(*left));
                    Ok(())
                }
            }
        }
        noble_kernel::types::Ty::GenericNominal(id, mut arguments, shape) => {
            // The selected descriptor adds up to seven queued steps beyond
            // the ordinary argument walk. Keep the owned queue bounded even
            // when a deeply nested but otherwise valid type reaches it.
            if is_descriptor_queue_full(work.len(), compact) {
                return Err(crate::Diagnostic::Exhausted);
            }
            attempt!(spell(out, compact, b"G(", b"GenericNominal("));
            attempt!(out.number(id.module));
            attempt!(out.append(b":"));
            attempt!(out.number(u64::from(id.ordinal)));
            attempt!(out.append(b","));
            // Move both owned arguments out without cloning them; Aeneas
            // cannot translate a by-value array destructuring pattern.
            let first = core::mem::replace(&mut arguments[0], noble_kernel::types::Ty::Unit);
            let second = core::mem::replace(&mut arguments[1], noble_kernel::types::Ty::Unit);
            work.push(Step::Byte(b')'));
            work.push(Step::Type(second));
            work.push(Step::Byte(b','));
            work.push(Step::Type(first));
            if compact {
                // A family ID and arguments alone do not identify a concrete
                // type across independently checked source environments:
                // swapped generic payload parameters can produce a different
                // canonical descriptor for those same arguments.
                work.push(Step::Byte(b','));
                match *shape {
                    noble_kernel::types::NominalShape::Opaque(value) => {
                        work.push(Step::Byte(b')'));
                        work.push(Step::Type(*value));
                        work.push(Step::Byte(b'('));
                        work.push(Step::Byte(b'O'));
                    }
                    noble_kernel::types::NominalShape::Variant(left, right) => {
                        work.push(Step::Byte(b')'));
                        work.push(Step::Type(*right));
                        work.push(Step::Byte(b','));
                        work.push(Step::Type(*left));
                        work.push(Step::Byte(b'('));
                        work.push(Step::Byte(b'V'));
                    }
                }
            }
            Ok(())
        }
        noble_kernel::types::Ty::Pair(left, right) => {
            attempt!(spell(out, compact, b"P(", b"Pair("));
            work.push(Step::Byte(b')'));
            work.push(Step::Type(*right));
            work.push(Step::Byte(b','));
            work.push(Step::Type(*left));
            Ok(())
        }
        noble_kernel::types::Ty::Sum(left, right) => {
            attempt!(spell(out, compact, b"S(", b"Sum("));
            work.push(Step::Byte(b')'));
            work.push(Step::Type(*right));
            work.push(Step::Byte(b','));
            work.push(Step::Type(*left));
            Ok(())
        }
        noble_kernel::types::Ty::List(item) => {
            attempt!(spell(out, compact, b"L(", b"List("));
            work.push(Step::Byte(b')'));
            work.push(Step::Type(*item));
            Ok(())
        }
        noble_kernel::types::Ty::Program(input_stack, output_stack, effects) => {
            attempt!(spell(out, compact, b"Q(", b"Program("));
            work.push(Step::Byte(b')'));
            if !effects.is_empty() {
                work.push(Step::Effects(effects));
            }
            work.push(Step::Stack(*output_stack));
            work.push(Step::Byte(b'>'));
            work.push(Step::Byte(b'-'));
            work.push(Step::Stack(*input_stack));
            Ok(())
        }
        noble_kernel::types::Ty::LiveRef(input_stack, output_stack, effects) => {
            attempt!(out.append(b"LiveRef("));
            work.push(Step::Byte(b')'));
            if !effects.is_empty() {
                work.push(Step::Effects(effects));
            }
            work.push(Step::Stack(*output_stack));
            work.push(Step::Byte(b'>'));
            work.push(Step::Byte(b'-'));
            work.push(Step::Stack(*input_stack));
            Ok(())
        }
    }
}

/// Core (non-extended) signatures cannot spell this type.
fn is_core_unavailable(ty: &noble_kernel::types::Ty, extended: bool) -> bool {
    !extended && effects::unavailable_in_core(ty)
}

/// A generic descriptor queues up to seven steps beyond the argument walk.
const fn is_descriptor_queue_full(queued: usize, compact: bool) -> bool {
    compact && queued > WORK_LIMIT - 11
}

/// Append the compact or the descriptive spelling of one type token.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; this appends one fixed spelling to the walk's private output sink; the chosen spelling depends only on the compact flag."
)]
fn spell(
    out: &mut crate::output::Buffer,
    compact: bool,
    short: &[u8],
    long: &[u8],
) -> Result<(), crate::Diagnostic> {
    if compact {
        out.append(short)
    } else {
        out.append(long)
    }
}

#[path = "walk/effects.rs"]
mod effects;
