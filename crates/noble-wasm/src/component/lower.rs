#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; lowering mutates only fresh compiler-owned locals, stack, data segments and bounded output bytes; borrowed exports and regenerated WIT bindings remain immutable and no partial plan is published."
)]

mod calls;
mod inputs;
mod operations;
mod results;

pub(super) struct Data {
    pub(super) segments: alloc::vec::Vec<(u32, alloc::vec::Vec<u8>)>,
    pub(super) end: u32,
}
pub(super) struct Plan {
    pub(super) name: alloc::string::String,
    pub(super) ordinal: u32,
    pub(super) asynchronous: bool,
    pub(super) parameters: alloc::vec::Vec<noble_contracts::component::Type>,
    pub(super) result: Option<noble_contracts::component::Type>,
    pub(super) locals: alloc::vec::Vec<super::abi::Lane>,
    pub(super) code: alloc::vec::Vec<u8>,
}

const VALUE_LANES: usize = 3;
#[derive(Clone, Copy)]
#[octet::sealed_enum]
enum Storage {
    Flat([Option<u32>; VALUE_LANES]),
    Pair(usize),
}
#[derive(Clone)]
struct Value {
    ty: noble_kernel::types::Ty,
    storage: Storage,
}
impl Value {
    const fn flat(&self) -> Result<&[Option<u32>; VALUE_LANES], crate::Diagnostic> {
        match &self.storage {
            Storage::Flat(locals) => Ok(locals),
            Storage::Pair(_) => Err(crate::Diagnostic::Unsupported),
        }
    }
}
struct State {
    locals: alloc::vec::Vec<super::abi::Lane>,
    parameters: usize,
    stack: alloc::vec::Vec<Value>,
    // Internal source pairs only reference existing locals. The immutable,
    // bounded arena avoids recursive values or guest aggregate allocations.
    pairs: alloc::vec::Vec<(Value, Value)>,
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
            None => Ok(Value {
                ty,
                storage: Storage::Flat(locals),
            }),
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
        let locals = attempt!(value.flat());
        let mut at = 0usize;
        let mut failure = None;
        let count = locals.len();
        while at < count && failure.is_none() {
            let local = locals[at];
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
        let locals = attempt!(value.flat());
        let mut at = locals.len();
        let mut failure = None;
        while at > 0 && failure.is_none() {
            at = at.saturating_sub(1);
            let local = locals[at];
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
    let mut state = attempt!(inputs::initial(&operation.parameters));
    attempt!(nodes(world, body, data, &mut state));
    let result = attempt!(super::abi::result(&operation.results));
    attempt!(results::finish(result, operation.asynchronous, &mut state));
    let ordinal = match u32::try_from(export.index()) {
        Ok(ordinal) => ordinal,
        Err(_) => return Err(crate::Diagnostic::Exhausted),
    };
    if operation.asynchronous {
        attempt!(state.code.append(b" call $task-return-"));
        attempt!(state.code.number(u64::from(ordinal)));
        attempt!(state.code.append(b"\n call $cleanup\n"));
    }
    Ok(Plan {
        name: operation.export_name.clone(),
        ordinal,
        asynchronous: operation.asynchronous,
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
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; canonical lane lookup uses non-const slice indexing and rejects absent lanes with Invalid rather than defaulting a local index."
)]
fn slot(value: &Value, at: usize) -> Result<u32, crate::Diagnostic> {
    let locals = attempt!(value.flat());
    let local = locals.get(at).copied();
    match local {
        Some(Some(local)) => Ok(local),
        Some(None) | None => Err(crate::Diagnostic::Invalid),
    }
}
