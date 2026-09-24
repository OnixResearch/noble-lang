#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; lowering mutates only fresh compiler-owned locals, stack, data segments and bounded output bytes; borrowed exports and regenerated WIT bindings remain immutable and no partial plan is published."
)]

mod calls;
mod operations;
mod results;

pub(super) struct Data {
    pub(super) segments: alloc::vec::Vec<(u32, alloc::vec::Vec<u8>)>,
    pub(super) end: u32,
}
pub(super) struct Plan {
    pub(super) name: alloc::string::String,
    pub(super) parameters: alloc::vec::Vec<noble_contracts::component::Type>,
    pub(super) result: Option<noble_contracts::component::Type>,
    pub(super) locals: alloc::vec::Vec<super::abi::Lane>,
    pub(super) code: alloc::vec::Vec<u8>,
}

const VALUE_LANES: usize = 3;
#[derive(Clone)]
struct Value {
    ty: noble_kernel::types::Ty,
    locals: [Option<u32>; VALUE_LANES],
}
struct State {
    locals: alloc::vec::Vec<super::abi::Lane>,
    parameters: usize,
    stack: alloc::vec::Vec<Value>,
    code: crate::output::Buffer,
}
impl State {
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; allocating a checked local requires non-const TryFrom and Vec growth in compiler-owned scratch; unrepresentable or exhausted local indices return diagnostics."
    )]
    fn local(&mut self, lane: super::abi::Lane) -> Result<u32, crate::Diagnostic> {
        let count = self.locals.len().saturating_add(self.parameters);
        if count >= super::LOCAL_LIMIT {
            return Err(crate::Diagnostic::Exhausted);
        }
        let index = match u32::try_from(count) {
            Ok(index) => index,
            Err(_) => return Err(crate::Diagnostic::Exhausted),
        };
        self.locals.push(lane);
        Ok(index)
    }
    fn value(&mut self, ty: noble_kernel::types::Ty) -> Result<Value, crate::Diagnostic> {
        let canonical = attempt!(super::abi::value_type(&ty));
        let mut locals = [None; VALUE_LANES];
        let mut at = 0usize;
        let mut failure = None;
        if let Some(canonical) = canonical {
            let count = super::abi::lane_count(canonical);
            while at < count && failure.is_none() {
                if at >= locals.len() {
                    failure = Some(crate::Diagnostic::Defective);
                } else {
                    match self.typed_local(canonical, at) {
                        Ok(local) => locals[at] = Some(local),
                        Err(error) => failure = Some(error),
                    }
                }
                at = at.saturating_add(1);
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(Value { ty, locals }),
        }
    }
    fn typed_local(
        &mut self,
        ty: noble_contracts::component::Type,
        at: usize,
    ) -> Result<u32, crate::Diagnostic> {
        let lane = attempt!(super::abi::lane(ty, at));
        self.local(lane)
    }
    fn pop(&mut self) -> Result<Value, crate::Diagnostic> {
        self.stack.pop().ok_or(crate::Diagnostic::Invalid)
    }
    fn read(&mut self, value: &Value) -> Result<(), crate::Diagnostic> {
        let mut at = 0usize;
        let mut failure = None;
        let count = value.locals.len();
        while at < count && failure.is_none() {
            let local = value.locals[at];
            if let Some(local) = local {
                if let Err(error) = super::abi::get(&mut self.code, local) {
                    failure = Some(error);
                }
            }
            at = at.saturating_add(1);
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
    fn capture(&mut self, value: &Value) -> Result<(), crate::Diagnostic> {
        let mut at = value.locals.len();
        let mut failure = None;
        while at > 0 && failure.is_none() {
            at = at.saturating_sub(1);
            let local = value.locals[at];
            if let Some(local) = local {
                if let Err(error) = super::abi::set(&mut self.code, local) {
                    failure = Some(error);
                }
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every admitted node is looked up with checked width conversion before native lowering; bounded stack and result checks propagate diagnostics while the caller-owned export stays immutable and the incomplete plan remains private."
)]
pub(super) fn function(
    world: &noble_contracts::component::World,
    export: &noble_contracts::component::CheckedExport,
    data: &mut Data,
) -> Result<Plan, crate::Diagnostic> {
    let operation = match world.exports().get(export.index()) {
        Some(operation) => operation,
        None => return Err(crate::Diagnostic::Invalid),
    };
    let body = match export.submission() {
        Some(submission) => &submission.body,
        None => return Err(crate::Diagnostic::Invalid),
    };
    let mut state = attempt!(initial(&operation.parameters));
    attempt!(nodes(world, body, data, &mut state));
    let result = attempt!(super::abi::result(&operation.results));
    attempt!(results::finish(result, &mut state));
    Ok(Plan {
        name: operation.export_name.clone(),
        parameters: operation.parameters.clone(),
        result,
        locals: state.locals,
        code: state.code.finish(),
    })
}

fn nodes(
    world: &noble_contracts::component::World,
    body: &noble_kernel::execution::Body,
    data: &mut Data,
    state: &mut State,
) -> Result<(), crate::Diagnostic> {
    let mut at = 0usize;
    let mut failure = None;
    let end = body.candidate.body.len();
    while at < end && failure.is_none() {
        let id = body.candidate.body[at];
        if let Err(error) = operations::node(id, body, world, data, state) {
            failure = Some(error);
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; each input's lane assignment and emitted value validation must succeed before the compiler state is returned; failures remain typed and unpublished."
)]
fn initial(parameters: &[noble_contracts::component::Type]) -> Result<State, crate::Diagnostic> {
    let mut state = State {
        locals: alloc::vec::Vec::new(),
        parameters: 0,
        stack: alloc::vec::Vec::with_capacity(parameters.len()),
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

fn input(ty: noble_contracts::component::Type, state: &mut State) -> Result<(), crate::Diagnostic> {
    let value = attempt!(input_value(ty, &mut state.parameters));
    attempt!(results::validate(ty, &value, &mut state.code));
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
) -> Result<Value, crate::Diagnostic> {
    let mut locals = [None; VALUE_LANES];
    let mut lane = 0usize;
    let mut failure = None;
    let count = super::abi::lane_count(ty);
    while lane < count && failure.is_none() {
        if let Err(error) = input_lane(parameters, &mut locals, lane) {
            failure = Some(error);
        }
        lane = lane.saturating_add(1);
    }
    if let Some(error) = failure {
        return Err(error);
    }
    if *parameters > super::FLAT_PARAMETER_LIMIT {
        return Err(crate::Diagnostic::Unsupported);
    }
    Ok(Value {
        ty: ty.noble(),
        locals,
    })
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; local assignment requires non-const TryFrom and checked mutable lane lookup, preserving Exhausted or Defective instead of truncating a local index."
)]
fn input_lane(
    parameters: &mut usize,
    locals: &mut [Option<u32>; VALUE_LANES],
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

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; canonical lane lookup uses non-const slice indexing and rejects absent lanes with Invalid rather than defaulting a local index."
)]
fn slot(value: &Value, at: usize) -> Result<u32, crate::Diagnostic> {
    let local = value.locals.get(at).copied();
    match local {
        Some(Some(local)) => Ok(local),
        Some(None) | None => Err(crate::Diagnostic::Invalid),
    }
}
