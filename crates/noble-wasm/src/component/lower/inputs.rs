#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; each input's lane assignment and emitted value validation must succeed before the compiler state is returned; failures remain typed and unpublished."
)]
pub(super) fn initial(
    parameters: &[noble_contracts::component::Type],
) -> Result<super::State, crate::Diagnostic> {
    let mut state = super::State {
        locals: alloc::vec::Vec::new(),
        parameters: 0,
        stack: alloc::vec::Vec::with_capacity(parameters.len()),
        pairs: alloc::vec::Vec::new(),
        code: crate::output::Buffer::new(1024),
    };
    let mut at = 0usize;
    let mut failure = None;
    let end = parameters.len();
    while at < end && failure.is_none() {
        let ty = parameters[at];
        if let Err(error) = input(ty, &mut state) {
            failure = Some(error);
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(state),
    }
}

fn input(
    ty: noble_contracts::component::Type,
    state: &mut super::State,
) -> Result<(), crate::Diagnostic> {
    let value = attempt!(input_value(ty, &mut state.parameters));
    attempt!(super::results::validate(ty, &value, &mut state.code));
    state.stack.push(value);
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; lane indexing, integer conversion and the canonical flat-parameter ceiling are checked fallibly; missing or excessive lanes cannot become a default local."
)]
fn input_value(
    ty: noble_contracts::component::Type,
    parameters: &mut usize,
) -> Result<super::Value, crate::Diagnostic> {
    let mut locals = [None; super::VALUE_LANES];
    let mut lane = 0usize;
    let mut failure = None;
    let count = super::super::abi::lane_count(ty);
    while lane < count && failure.is_none() {
        if let Err(error) = input_lane(parameters, &mut locals, lane) {
            failure = Some(error);
        }
        lane = lane.saturating_add(1);
    }
    if let Some(error) = failure {
        return Err(error);
    }
    if *parameters > super::super::FLAT_PARAMETER_LIMIT {
        return Err(crate::Diagnostic::Unsupported);
    }
    Ok(super::Value {
        ty: ty.noble(),
        storage: super::Storage::Flat(locals),
    })
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; local assignment requires non-const TryFrom and checked mutable lane lookup, preserving Exhausted or Defective instead of truncating a local index."
)]
fn input_lane(
    parameters: &mut usize,
    locals: &mut [Option<u32>; super::VALUE_LANES],
    lane: usize,
) -> Result<(), crate::Diagnostic> {
    let index = match u32::try_from(*parameters) {
        Ok(index) => index,
        Err(_) => return Err(crate::Diagnostic::Exhausted),
    };
    let slot = match locals.get_mut(lane) {
        Some(slot) => slot,
        None => return Err(crate::Diagnostic::Defective),
    };
    *slot = Some(index);
    *parameters = parameters.saturating_add(1);
    Ok(())
}
