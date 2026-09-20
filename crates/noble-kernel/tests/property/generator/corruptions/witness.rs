//! Arity, kind, stack-size, and cyclic-variable witness corruptions.

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; apply deliberately constructs invalid arity, kind and stack witnesses, returning errors only for failed random-index conversion; asserting witness validity would defeat the malformed lane."
)]
pub(super) fn apply(
    rng: &mut crate::rng::Stream,
    candidate: &mut noble_kernel::untrusted::Candidate,
    choice: u64,
) -> Result<(), String> {
    let Some(first) = candidate.nodes.first_mut() else {
        return Ok(());
    };
    let inst = crate::malform::inst_of(first);
    match choice {
        5 | 6 => {
            if choice == 5 && inst.bindings.len() > 1 {
                #[expect(
                    tigerstyle::raw_arithmetic_overflow,
                    reason = "Owner: noble-maintainers; arity-minus reaches this truncation only after proving the binding count exceeds one."
                )]
                inst.bindings.truncate(inst.bindings.len() - 1);
            } else {
                inst.bindings.push(noble_kernel::words::Binding::Value(
                    noble_kernel::types::Ty::Unit,
                ));
            }
        }
        7 => {
            if let Some(binding) = inst.bindings.first_mut() {
                *binding = match binding {
                    noble_kernel::words::Binding::Stack(_) => {
                        noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Unit)
                    }
                    _ => noble_kernel::words::Binding::Stack(Vec::new()),
                };
            }
        }
        8 => {
            if let Some(binding) = inst.bindings.first_mut() {
                *binding =
                    noble_kernel::words::Binding::Stack(vec![noble_kernel::types::Ty::I64; 40]);
            }
        }
        _ => crate::malform::witness::cycle(rng, inst)?,
    }
    Ok(())
}

fn cycle(rng: &mut crate::rng::Stream, inst: &mut noble_kernel::words::Inst) -> Result<(), String> {
    if inst.bindings.is_empty() {
        return Ok(());
    }
    let target = rng.index(inst.bindings.len())?;
    let variable = u32::try_from(target)
        .map_err(|error| format!("cyclic witness variable exceeds u32: {error}"))?;
    inst.bindings[target] =
        noble_kernel::words::Binding::Ref(noble_kernel::words::Variable(variable));
    Ok(())
}
