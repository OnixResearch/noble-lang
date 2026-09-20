/// Complete a pair, sum, or list from its finished segments.
#[expect(
    tigerstyle::assertion_density,
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; required children and constructor kind fail through InstError, preserving right-then-left pops. Every new Pattern must require a reviewed completion rule, not a fallback."
)]
pub(super) fn apply(node: crate::shapes::Pattern, mut walk: super::Walk) -> super::StepState {
    // A list completes from its single segment; pair and sum from two.
    if let crate::shapes::Pattern::List(_) = node {
        return match walk
            .segments
            .pop()
            .map(crate::words::segments::list_segment)
        {
            Some(Ok(segment)) => {
                walk.segments.push(segment);
                (walk, Ok(()))
            }
            Some(Err(problem)) => (walk, Err(problem)),
            None => (walk, Err(crate::words::InstError::OversizedType)),
        };
    }
    // The first pop is the top: the right child's finished segment.
    let popped = (walk.segments.pop(), walk.segments.pop());
    let (left, right) = match popped {
        (Some(right), Some(left)) => (left, right),
        _ => return (walk, Err(crate::words::InstError::OversizedType)),
    };
    let built = match node {
        crate::shapes::Pattern::Pair(_, _) => {
            crate::words::segments::pair_segment(left, right, false)
        }
        crate::shapes::Pattern::Sum(_, _) => {
            crate::words::segments::pair_segment(left, right, true)
        }
        crate::shapes::Pattern::Var(_)
        | crate::shapes::Pattern::StackVar(_)
        | crate::shapes::Pattern::List(_)
        | crate::shapes::Pattern::Program(_, _, _)
        | crate::shapes::Pattern::Unit
        | crate::shapes::Pattern::Bool
        | crate::shapes::Pattern::I64
        | crate::shapes::Pattern::Text
        | crate::shapes::Pattern::Syntax
        | crate::shapes::Pattern::Resource(_) => Err(crate::words::InstError::KindMismatch),
    };
    match built {
        Ok(segment) => {
            walk.segments.push(segment);
            (walk, Ok(()))
        }
        Err(problem) => (walk, Err(problem)),
    }
}
