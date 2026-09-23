//! Bounded structural comparison, separate from Clone and Debug boundaries.

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
        && work.len() < super::super::WORK_CAP;
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

pub(super) fn pattern_eq(left: &crate::shapes::Pattern, right: &crate::shapes::Pattern) -> bool {
    let mut work = alloc::vec::Vec::with_capacity(8);
    work.push((left.clone(), right.clone()));
    let mut is_equal = true;
    while !work.is_empty() {
        if work.len() >= super::super::WORK_CAP {
            is_equal = false;
            break;
        }
        if let Some((first, second)) = work.pop() {
            let (next, is_step_equal) = compare(first, second, work);
            work = next;
            is_equal = is_equal && is_step_equal;
        }
    }
    is_equal
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; structural comparison returns false on unequal constructors, lengths or work exhaustion; ordinary unequal inputs must not panic."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; comparison consumes heap-owned Pattern nodes, calls their non-const equality implementations, and pushes queued operands into Vec. Those operations are not const on the pinned compiler."
)]
fn compare(
    first: crate::shapes::Pattern,
    second: crate::shapes::Pattern,
    mut work: alloc::vec::Vec<(crate::shapes::Pattern, crate::shapes::Pattern)>,
) -> (
    alloc::vec::Vec<(crate::shapes::Pattern, crate::shapes::Pattern)>,
    bool,
) {
    let is_equal = match (first, second) {
        (crate::shapes::Pattern::Unit, crate::shapes::Pattern::Unit) => true,
        (crate::shapes::Pattern::Bool, crate::shapes::Pattern::Bool) => true,
        (crate::shapes::Pattern::I64, crate::shapes::Pattern::I64) => true,
        (crate::shapes::Pattern::Text, crate::shapes::Pattern::Text) => true,
        (crate::shapes::Pattern::Syntax, crate::shapes::Pattern::Syntax) => true,
        (crate::shapes::Pattern::Contract, crate::shapes::Pattern::Contract) => true,
        (crate::shapes::Pattern::Evidence, crate::shapes::Pattern::Evidence) => true,
        (crate::shapes::Pattern::Certified, crate::shapes::Pattern::Certified) => true,
        (
            crate::shapes::Pattern::Resource(first_kind),
            crate::shapes::Pattern::Resource(second_kind),
        ) => first_kind == second_kind,
        (crate::shapes::Pattern::Var(first_var), crate::shapes::Pattern::Var(second_var)) => {
            first_var == second_var
        }
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
        (crate::shapes::Pattern::List(first_item), crate::shapes::Pattern::List(second_item)) => {
            work.push((*first_item, *second_item));
            true
        }
        (
            crate::shapes::Pattern::Program(a_in, a_out, a_eff),
            crate::shapes::Pattern::Program(b_in, b_out, b_eff),
        ) => {
            let (next, is_program_equal) = push_pattern_program(work, &a_in, &a_out, &b_in, &b_out);
            work = next;
            is_program_equal && a_eff == b_eff
        }
        _ => false,
    };
    (work, is_equal)
}
