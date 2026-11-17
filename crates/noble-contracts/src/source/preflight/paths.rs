/// The type reached from `root` by following `path` in order, stopping at the
/// first invalid step. It recurses over the path rather than looping: the
/// pinned Aeneas accepts no loop that carries a borrow into the type tree.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-maintainers; each call consumes one step of a path that the bounded private traversal builds within the admitted type-size and depth limits, so the recursion depth is the path length."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; borrowed child selection uses non-const Box dereferencing and constructs owned diagnostics on mismatched paths."
)]
pub(in crate::source) fn locate<'a>(
    root: &'a noble_kernel::types::Ty,
    path: &[super::PathStep],
    span: crate::Span,
) -> Result<&'a noble_kernel::types::Ty, crate::Diagnostic> {
    match path.split_first() {
        None => Ok(root),
        Some((step, rest)) => match child(root, *step, span) {
            Ok(next) => locate(next, rest, span),
            Err(problem) => Err(problem),
        },
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; borrowed child selection uses non-const Box dereferencing and constructs owned diagnostics on mismatched paths."
)]
fn child(
    ty: &noble_kernel::types::Ty,
    step: super::PathStep,
    span: crate::Span,
) -> Result<&noble_kernel::types::Ty, crate::Diagnostic> {
    match (step, ty) {
        (super::PathStep::Root, _) => Ok(ty),
        (
            super::PathStep::Left,
            noble_kernel::types::Ty::Pair(left, _) | noble_kernel::types::Ty::Sum(left, _),
        ) => Ok(left),
        (
            super::PathStep::Right,
            noble_kernel::types::Ty::Pair(_, right) | noble_kernel::types::Ty::Sum(_, right),
        ) => Ok(right),
        (super::PathStep::Item, noble_kernel::types::Ty::List(item)) => Ok(item),
        (
            super::PathStep::GenericArgument(index),
            noble_kernel::types::Ty::GenericNominal(_, arguments, _),
        ) => arguments.get(index).ok_or_else(|| crate::internal(span)),
        (
            super::PathStep::Input(index),
            noble_kernel::types::Ty::Program(input, _, _)
            | noble_kernel::types::Ty::LiveRef(input, _, _),
        ) => match input.get(index) {
            Some(child) => Ok(child),
            None => Err(crate::internal(span)),
        },
        (
            super::PathStep::Output(index),
            noble_kernel::types::Ty::Program(_, output, _)
            | noble_kernel::types::Ty::LiveRef(_, output, _),
        ) => match output.get(index) {
            Some(child) => Ok(child),
            None => Err(crate::internal(span)),
        },
        _ => Err(crate::internal(span)),
    }
}
