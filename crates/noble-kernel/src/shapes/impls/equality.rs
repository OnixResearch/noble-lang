//! Structural comparison, separate from Clone and Debug boundaries.
//!
//! The comparison borrows both patterns: it never copies a subtree, so a deep
//! pattern is compared in one linear pass and a rejected pattern is not
//! duplicated first.

/// Structural equality of two patterns, decided by one explicit pairwise walk
/// that fails closed past the local work bound.
///
/// The first unequal pair, or an exhausted bound, returns `false` at once.
/// Every walk ends in `false` once a pair differs, because the verdict only
/// conjoins later steps, so stopping early decides exactly the same result.
///
/// A `Program` pair queues its element pairs in place, inputs then outputs,
/// through slice iterators. The selected Aeneas cannot translate a loop that
/// pushes borrows obtained by indexing, nor a helper that threads the
/// borrowed work stack through such a loop, nor a join of step verdicts after
/// those pushes.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; structural comparison returns false on unequal constructors, lengths or work exhaustion; ordinary unequal inputs must not panic."
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-kernel; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
pub(super) fn pattern_eq(left: &crate::shapes::Pattern, right: &crate::shapes::Pattern) -> bool {
    let mut work: alloc::vec::Vec<(&crate::shapes::Pattern, &crate::shapes::Pattern)> =
        alloc::vec::Vec::with_capacity(8);
    work.push((left, right));
    while !work.is_empty() {
        if work.len() >= super::super::WORK_CAP {
            return false;
        }
        // Unreachable: the stack was just checked non-empty, and an empty
        // stack ends the walk as equal.
        let Some((first, second)) = work.pop() else {
            return true;
        };
        match (first, second) {
            (crate::shapes::Pattern::Unit, crate::shapes::Pattern::Unit)
            | (crate::shapes::Pattern::Bool, crate::shapes::Pattern::Bool)
            | (crate::shapes::Pattern::I64, crate::shapes::Pattern::I64)
            | (crate::shapes::Pattern::Text, crate::shapes::Pattern::Text)
            | (crate::shapes::Pattern::Syntax, crate::shapes::Pattern::Syntax)
            | (crate::shapes::Pattern::Contract, crate::shapes::Pattern::Contract)
            | (crate::shapes::Pattern::Evidence, crate::shapes::Pattern::Evidence)
            | (crate::shapes::Pattern::Certified, crate::shapes::Pattern::Certified) => {}
            (
                crate::shapes::Pattern::Resource(first_kind),
                crate::shapes::Pattern::Resource(second_kind),
            ) if first_kind == second_kind => {}
            (
                crate::shapes::Pattern::Nominal(first_id, first_shape),
                crate::shapes::Pattern::Nominal(second_id, second_shape),
            ) if first_id == second_id && first_shape == second_shape => {}
            (
                crate::shapes::Pattern::GenericNominal(first_id, first_args, first_mapping),
                crate::shapes::Pattern::GenericNominal(second_id, second_args, second_mapping),
            ) if first_id == second_id && first_mapping == second_mapping => {
                work.push((&first_args[0], &second_args[0]));
                work.push((&first_args[1], &second_args[1]));
            }
            (crate::shapes::Pattern::Var(first_var), crate::shapes::Pattern::Var(second_var))
            | (
                crate::shapes::Pattern::StackVar(first_var),
                crate::shapes::Pattern::StackVar(second_var),
            ) if first_var == second_var => {}
            (
                crate::shapes::Pattern::Pair(first_head, first_tail),
                crate::shapes::Pattern::Pair(second_head, second_tail),
            )
            | (
                crate::shapes::Pattern::Sum(first_head, first_tail),
                crate::shapes::Pattern::Sum(second_head, second_tail),
            ) => {
                work.push((first_head, second_head));
                work.push((first_tail, second_tail));
            }
            (
                crate::shapes::Pattern::List(first_item),
                crate::shapes::Pattern::List(second_item),
            ) => {
                work.push((first_item, second_item));
            }
            (
                crate::shapes::Pattern::Program(a_in, a_out, a_eff),
                crate::shapes::Pattern::Program(b_in, b_out, b_eff),
            )
            | (
                crate::shapes::Pattern::LiveRef(a_in, a_out, a_eff),
                crate::shapes::Pattern::LiveRef(b_in, b_out, b_eff),
            ) => {
                if a_in.len() != b_in.len()
                    || a_out.len() != b_out.len()
                    || work.len() >= super::super::WORK_CAP
                {
                    return false;
                }
                let mut second_inputs = b_in.iter();
                for first_input in a_in.iter() {
                    if let Some(second_input) = second_inputs.next() {
                        work.push((first_input, second_input));
                    }
                }
                let mut second_outputs = b_out.iter();
                for first_output in a_out.iter() {
                    if let Some(second_output) = second_outputs.next() {
                        work.push((first_output, second_output));
                    }
                }
                if a_eff != b_eff {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}
