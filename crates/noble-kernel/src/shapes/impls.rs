//! Structural trait implementations avoid Aeneas's derived-method forward references.

/// Copy one pattern stack explicitly. Extraction assumes this helper: Aeneas
/// cannot prove monotonicity of its loop inside Clone's mutual instance block.
#[charon::opaque]
fn clone_parts(stack: &[crate::shapes::Pattern]) -> alloc::vec::Vec<crate::shapes::Pattern> {
    let mut out: alloc::vec::Vec<crate::shapes::Pattern> =
        alloc::vec::Vec::with_capacity(stack.len());
    let mut index = 0;
    while index < stack.len() {
        out.push(stack[index].clone());
        index += 1;
    }
    out
}

impl Clone for crate::shapes::Pattern {
    fn clone(&self) -> crate::shapes::Pattern {
        match self {
            crate::shapes::Pattern::Unit => crate::shapes::Pattern::Unit,
            crate::shapes::Pattern::Bool => crate::shapes::Pattern::Bool,
            crate::shapes::Pattern::I64 => crate::shapes::Pattern::I64,
            crate::shapes::Pattern::Text => crate::shapes::Pattern::Text,
            crate::shapes::Pattern::Syntax => crate::shapes::Pattern::Syntax,
            crate::shapes::Pattern::Resource(kind) => crate::shapes::Pattern::Resource(*kind),
            crate::shapes::Pattern::Var(variable) => crate::shapes::Pattern::Var(*variable),
            crate::shapes::Pattern::StackVar(variable) => {
                crate::shapes::Pattern::StackVar(*variable)
            }
            crate::shapes::Pattern::Pair(head, tail) => crate::shapes::Pattern::Pair(
                alloc::boxed::Box::new((**head).clone()),
                alloc::boxed::Box::new((**tail).clone()),
            ),
            crate::shapes::Pattern::Sum(head, tail) => crate::shapes::Pattern::Sum(
                alloc::boxed::Box::new((**head).clone()),
                alloc::boxed::Box::new((**tail).clone()),
            ),
            crate::shapes::Pattern::List(item) => {
                crate::shapes::Pattern::List(alloc::boxed::Box::new((**item).clone()))
            }
            crate::shapes::Pattern::Program(parts_in, parts_out, slots) => {
                crate::shapes::Pattern::Program(
                    alloc::boxed::Box::new(clone_parts(parts_in.as_slice())),
                    alloc::boxed::Box::new(clone_parts(parts_out.as_slice())),
                    alloc::boxed::Box::new(clone_slots(slots.as_slice())),
                )
            }
        }
    }
}

fn clone_slots(stack: &[crate::shapes::EffectSlot]) -> alloc::vec::Vec<crate::shapes::EffectSlot> {
    let mut out: alloc::vec::Vec<crate::shapes::EffectSlot> =
        alloc::vec::Vec::with_capacity(stack.len());
    let mut index = 0;
    while index < stack.len() {
        out.push(stack[index].clone());
        index += 1;
    }
    out
}

/// Queue one `Program` pair's element checks; the flag fails closed.
fn push_pattern_program(
    mut work: alloc::vec::Vec<(crate::shapes::Pattern, crate::shapes::Pattern)>,
    first_in: &[crate::shapes::Pattern],
    first_out: &[crate::shapes::Pattern],
    second_in: &[crate::shapes::Pattern],
    second_out: &[crate::shapes::Pattern],
) -> (
    alloc::vec::Vec<(crate::shapes::Pattern, crate::shapes::Pattern)>,
    bool,
) {
    let is_comparable = first_in.len() == second_in.len()
        && first_out.len() == second_out.len()
        && work.len() < super::WORK_CAP;
    let mut index = 0;
    while index < first_in.len() && is_comparable {
        work.push((first_in[index].clone(), second_in[index].clone()));
        index += 1;
    }
    index = 0;
    while index < first_out.len() && is_comparable {
        work.push((first_out[index].clone(), second_out[index].clone()));
        index += 1;
    }
    (work, is_comparable)
}

/// Structural equality of two patterns, decided by one explicit pairwise walk.
/// The work stack holds cloned node pairs; the walk fails closed past the bound.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; pattern_eq is a total comparison predicate that returns false for mismatched constructors, lengths or work bounds; asserting comparability would break equality on ordinary unequal inputs."
)]
fn pattern_eq(left: &crate::shapes::Pattern, right: &crate::shapes::Pattern) -> bool {
    let mut work: alloc::vec::Vec<(crate::shapes::Pattern, crate::shapes::Pattern)> =
        alloc::vec::Vec::with_capacity(8);
    work.push((left.clone(), right.clone()));
    let mut is_mismatch = false;
    while !work.is_empty() {
        if work.len() >= super::WORK_CAP {
            is_mismatch = true;
            break;
        }
        let pair = work.pop();
        let is_step_equal = match pair {
            Some((first, second)) => match (first, second) {
                (crate::shapes::Pattern::Unit, crate::shapes::Pattern::Unit) => true,
                (crate::shapes::Pattern::Bool, crate::shapes::Pattern::Bool) => true,
                (crate::shapes::Pattern::I64, crate::shapes::Pattern::I64) => true,
                (crate::shapes::Pattern::Text, crate::shapes::Pattern::Text) => true,
                (crate::shapes::Pattern::Syntax, crate::shapes::Pattern::Syntax) => true,
                (
                    crate::shapes::Pattern::Resource(first_kind),
                    crate::shapes::Pattern::Resource(second_kind),
                ) => first_kind == second_kind,
                (
                    crate::shapes::Pattern::Var(first_var),
                    crate::shapes::Pattern::Var(second_var),
                ) => first_var == second_var,
                (
                    crate::shapes::Pattern::StackVar(first_var),
                    crate::shapes::Pattern::StackVar(second_var),
                ) => first_var == second_var,
                (
                    crate::shapes::Pattern::Pair(first_head, first_tail),
                    crate::shapes::Pattern::Pair(second_head, second_tail),
                )
                | (
                    crate::shapes::Pattern::Sum(first_head, first_tail),
                    crate::shapes::Pattern::Sum(second_head, second_tail),
                ) => {
                    work.push((*first_head, *second_head));
                    work.push((*first_tail, *second_tail));
                    true
                }
                (
                    crate::shapes::Pattern::List(first_item),
                    crate::shapes::Pattern::List(second_item),
                ) => {
                    work.push((*first_item, *second_item));
                    true
                }
                (
                    crate::shapes::Pattern::Program(a_in, a_out, a_eff),
                    crate::shapes::Pattern::Program(b_in, b_out, b_eff),
                ) => {
                    let (next, is_program_equal) =
                        push_pattern_program(work, &a_in, &a_out, &b_in, &b_out);
                    work = next;
                    is_program_equal && a_eff == b_eff
                }
                _ => false,
            },
            None => true,
        };
        if !is_step_equal {
            is_mismatch = true;
        }
    }
    !is_mismatch
}

impl PartialEq for crate::shapes::Pattern {
    fn eq(&self, other: &crate::shapes::Pattern) -> bool {
        pattern_eq(self, other)
    }
}

/// Render `[a, b]`. Extraction assumes this observation-only helper: Aeneas
/// cannot prove monotonicity of its loop inside Debug's mutual instance block.
#[charon::opaque]
#[allow(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers. Formatting writes only the explicit caller-owned Formatter required by core::fmt; text layout is outside the semantic refinement boundary."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; debug_parts uses length-bounded indexing and propagates the caller's first fmt::Error; formatting arbitrary patterns must not introduce assertion panics."
)]
fn debug_parts(
    stack: &[crate::shapes::Pattern],
    f: &mut core::fmt::Formatter<'_>,
) -> core::fmt::Result {
    attempt!(core::fmt::Formatter::write_str(f, "["));
    let mut index = 0;
    let mut failure: Option<core::fmt::Error> = None;
    while index < stack.len() {
        let step = if index == 0 {
            core::fmt::Debug::fmt(&stack[index], f)
        } else {
            match core::fmt::Formatter::write_str(f, ", ") {
                Ok(()) => core::fmt::Debug::fmt(&stack[index], f),
                Err(problem) => Err(problem),
            }
        };
        match step {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => core::fmt::Formatter::write_str(f, "]"),
    }
}

/// Render `[a, b]`. Opaque for the same mutual Debug-loop monotonicity
/// limitation as debug_parts; its body remains an extraction assumption.
#[charon::opaque]
#[allow(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers. Formatting writes only the explicit caller-owned Formatter required by core::fmt; text layout is outside the semantic refinement boundary."
)]
fn debug_slots(
    stack: &[crate::shapes::EffectSlot],
    f: &mut core::fmt::Formatter<'_>,
) -> core::fmt::Result {
    attempt!(core::fmt::Formatter::write_str(f, "["));
    let mut index = 0;
    while index < stack.len() {
        if index > 0 {
            attempt!(core::fmt::Formatter::write_str(f, ", "));
        }
        attempt!(core::fmt::Debug::fmt(&stack[index], f));
        index += 1;
    }
    core::fmt::Formatter::write_str(f, "]")
}

/// Render one pattern in its constructor form.
impl core::fmt::Debug for crate::shapes::Pattern {
    #[allow(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-maintainers. The standard Debug trait requires a mutable caller-owned Formatter; this performs no ambient observation or semantic state mutation."
    )]
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; this implements the non-const Debug::fmt trait method and calls runtime Formatter writes; reassess only if core::fmt gains a stable const trait contract."
    )]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            crate::shapes::Pattern::Unit => core::fmt::Formatter::write_str(f, "Unit"),
            crate::shapes::Pattern::Bool => core::fmt::Formatter::write_str(f, "Bool"),
            crate::shapes::Pattern::I64 => core::fmt::Formatter::write_str(f, "I64"),
            crate::shapes::Pattern::Text => core::fmt::Formatter::write_str(f, "Text"),
            crate::shapes::Pattern::Syntax => core::fmt::Formatter::write_str(f, "Syntax"),
            crate::shapes::Pattern::Resource(kind) => {
                attempt!(core::fmt::Formatter::write_str(f, "Resource("));
                attempt!(core::fmt::Debug::fmt(kind, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::shapes::Pattern::Var(variable) => {
                attempt!(core::fmt::Formatter::write_str(f, "Var("));
                attempt!(core::fmt::Debug::fmt(variable, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::shapes::Pattern::StackVar(variable) => {
                attempt!(core::fmt::Formatter::write_str(f, "StackVar("));
                attempt!(core::fmt::Debug::fmt(variable, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::shapes::Pattern::Pair(left, right) => {
                attempt!(core::fmt::Formatter::write_str(f, "Pair("));
                attempt!(core::fmt::Debug::fmt(&**left, f));
                attempt!(core::fmt::Formatter::write_str(f, ", "));
                attempt!(core::fmt::Debug::fmt(&**right, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::shapes::Pattern::Sum(left, right) => {
                attempt!(core::fmt::Formatter::write_str(f, "Sum("));
                attempt!(core::fmt::Debug::fmt(&**left, f));
                attempt!(core::fmt::Formatter::write_str(f, ", "));
                attempt!(core::fmt::Debug::fmt(&**right, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::shapes::Pattern::List(item) => {
                attempt!(core::fmt::Formatter::write_str(f, "List("));
                attempt!(core::fmt::Debug::fmt(&**item, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::shapes::Pattern::Program(parts_in, parts_out, slots) => {
                attempt!(core::fmt::Formatter::write_str(f, "Program("));
                attempt!(debug_parts(parts_in.as_slice(), f));
                attempt!(core::fmt::Formatter::write_str(f, ", "));
                attempt!(debug_parts(parts_out.as_slice(), f));
                attempt!(core::fmt::Formatter::write_str(f, ", "));
                attempt!(debug_slots(slots.as_slice(), f));
                core::fmt::Formatter::write_str(f, ")")
            }
        }
    }
}

#[cfg(test)]
impl Eq for crate::shapes::Pattern {}
