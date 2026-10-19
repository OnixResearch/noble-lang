//! Effect serialization for the signature walk.

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; Core signature refusal must inspect program effects via EffSet::is_empty, which is nonconst over the kernel-owned effect set; the borrowed classifier cannot be const."
)]
pub(super) fn unavailable_in_core(ty: &noble_kernel::types::Ty) -> bool {
    match ty {
        noble_kernel::types::Ty::Text
        | noble_kernel::types::Ty::Sum(_, _)
        | noble_kernel::types::Ty::Contract
        | noble_kernel::types::Ty::Evidence
        | noble_kernel::types::Ty::Certified
        | noble_kernel::types::Ty::Nominal(_, _)
        | noble_kernel::types::Ty::GenericNominal(_, _, _)
        | noble_kernel::types::Ty::LiveRef(_, _, _) => true,
        noble_kernel::types::Ty::Program(_, _, effects) => !effects.is_empty(),
        noble_kernel::types::Ty::Unit
        | noble_kernel::types::Ty::Bool
        | noble_kernel::types::Ty::I64
        | noble_kernel::types::Ty::Syntax
        | noble_kernel::types::Ty::Resource(_)
        | noble_kernel::types::Ty::Pair(_, _)
        | noble_kernel::types::Ty::List(_) => false,
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the bounded effect scan propagates Buffer::append failures as Diagnostic instead of asserting on effect data; reassess when the heuristic understands fallible validators."
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; effect serialization writes only the walk's fresh owned emission sink, leaving caller-owned effects unchanged."
)]
pub(super) fn emit_effects(
    effects: &noble_kernel::types::EffSet,
    out: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    // Empty effects retain the experimental signature spelling.
    if effects.is_empty() {
        return Ok(());
    }
    attempt!(out.append(b"!{"));
    let mut index = 0usize;
    let mut failure = None;
    while index < effects.as_slice().len() {
        match emit_effect(out, effects.as_slice()[index].0, index != 0) {
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
    out.append(b"}")
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; effect serialization mutates only the fresh owned emission sink, not the borrowed effect identity."
)]
fn emit_effect(
    out: &mut crate::output::Buffer,
    effect: u32,
    separated: bool,
) -> Result<(), crate::Diagnostic> {
    if separated {
        attempt!(out.append(b","));
    }
    out.number(u64::from(effect))
}
