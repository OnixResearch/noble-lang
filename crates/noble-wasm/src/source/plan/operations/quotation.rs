#[expect(
    tigerstyle::assertion_density,
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; quotation lowering interns the captured witness and exact Program signatures in the unpublished prospective compiler and charges its private meter; missing operands, effects and metering failures return diagnostics rather than asserting on source input."
)]
pub(super) fn lower(
    interface: &noble_kernel::untrusted::Interface,
    compiler: &mut super::super::super::Compiler,
    work: &mut super::super::super::Work,
) -> Result<super::super::Action, crate::Diagnostic> {
    let captured = match interface.stack_in.last() {
        Some(value) => value,
        None => return Err(crate::Diagnostic::Invalid),
    };
    let witness = attempt!(compiler.signature(core::slice::from_ref(captured), work));
    match interface.stack_out.last() {
        Some(noble_kernel::types::Ty::Program(input, output, effects)) => {
            if !effects.is_empty() {
                return Err(crate::Diagnostic::Invalid);
            }
            Ok(super::super::Action::Quote(
                attempt!(compiler.signature(input, work)),
                attempt!(compiler.signature(output, work)),
                witness,
            ))
        }
        Some(_) | None => Err(crate::Diagnostic::Invalid),
    }
}
