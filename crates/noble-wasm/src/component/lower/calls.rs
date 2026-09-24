#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; call lowering mutates only the compiler-owned operand stack, local table and bounded output sink; WIT signatures and caller-owned input stay immutable, and no incomplete plan escapes."
)]

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; all arguments are checked before emitting an owned transfer or host call; exact retptr loading and dynamic result validation preserve runtime refusal paths, while bounded compiler failures return diagnostics rather than assertions."
)]
pub(super) fn invoke(
    operation: &noble_contracts::component::Operation,
    state: &mut super::State,
) -> Result<(), crate::Diagnostic> {
    let arguments = attempt!(arguments(operation, state));
    // Validate every argument before committing any owned transfer or calling
    // the host. Borrowed owners are absent from the guest stack throughout.
    attempt!(validate_arguments(operation, &arguments, &mut state.code));
    let result_type = attempt!(super::super::abi::result(&operation.results));
    let returned = match result_type {
        Some(ty) => Some(attempt!(state.value(ty.noble()))),
        None => None,
    };
    let area = match result_type {
        Some(ty) if operation.asynchronous || super::super::abi::lane_count(ty) > 1 => {
            Some(attempt!(super::results::allocate(ty, state)))
        }
        Some(_) | None => None,
    };
    if super::super::abi::indirect_parameters(operation) {
        attempt!(indirect_arguments(operation, &arguments, state));
    } else {
        attempt!(read_arguments(&arguments, state));
    }
    if let Some(area) = area {
        attempt!(super::super::abi::get(&mut state.code, area));
    }
    let definition = match operation.definition {
        Some(definition) => definition,
        None => return Err(crate::Diagnostic::Invalid),
    };
    attempt!(state.code.append(b" call $i"));
    attempt!(state.code.number(u64::from(definition.0)));
    attempt!(state.code.append(b"\n"));
    if operation.asynchronous {
        // No following source instruction is emitted before this subtask has
        // returned and its native handle has been dropped. The result area and
        // indirect argument tuple remain allocated across engine suspension.
        attempt!(state.code.append(b" call $await-subtask\n"));
    }
    if let Some(value) = &returned {
        match area {
            Some(area) => attempt!(super::results::load(
                result_type,
                area,
                value,
                &mut state.code
            )),
            None => attempt!(state.capture(value)),
        }
        let ty = match result_type {
            Some(ty) => ty,
            None => return Err(crate::Diagnostic::Defective),
        };
        attempt!(super::results::validate(ty, value, &mut state.code));
    }
    attempt!(restore_borrows(operation, arguments, state));
    if let Some(value) = returned {
        state.stack.push(value);
    }
    Ok(())
}

struct IndirectLocals {
    area: u32,
    field: u32,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; invocation has already collected exactly the checked signature's arguments; tuple layout, private local allocation and every emitted store preserve exhaustion diagnostics rather than assertion failures."
)]
fn indirect_arguments(
    operation: &noble_contracts::component::Operation,
    arguments: &[super::Value],
    state: &mut super::State,
) -> Result<(), crate::Diagnostic> {
    let layout = attempt!(super::super::abi::tuple_layout(&operation.parameters));
    let area = attempt!(super::results::allocate_layout(layout, state));
    let locals = IndirectLocals {
        area,
        field: attempt!(state.local(super::super::abi::Lane::I32)),
    };
    let mut offset_bytes = 0u32;
    let mut at = 0usize;
    let mut failure = None;
    let count = arguments.len();
    while at < count && failure.is_none() {
        let ty = operation.parameters[at];
        match store_argument(ty, &arguments[at], &locals, offset_bytes, &mut state.code) {
            Ok(next) => offset_bytes = next,
            Err(error) => failure = Some(error),
        }
        at = at.saturating_add(1);
    }
    if let Some(error) = failure {
        return Err(error);
    }
    super::super::abi::get(&mut state.code, area)
}

fn store_argument(
    ty: noble_contracts::component::Type,
    value: &super::Value,
    locals: &IndirectLocals,
    offset_bytes: u32,
    buffer: &mut crate::output::Buffer,
) -> Result<u32, crate::Diagnostic> {
    let (align, size_bytes) = super::super::abi::memory_layout(ty);
    let offset_bytes = attempt!(super::super::abi::align_to(offset_bytes, align));
    attempt!(super::super::abi::get(buffer, locals.area));
    attempt!(buffer.i32(offset_bytes));
    attempt!(buffer.append(b" i32.add\n"));
    attempt!(super::super::abi::set(buffer, locals.field));
    attempt!(super::results::store(ty, locals.field, value, buffer));
    offset_bytes
        .checked_add(size_bytes)
        .ok_or(crate::Diagnostic::Exhausted)
}

fn validate_arguments(
    operation: &noble_contracts::component::Operation,
    arguments: &[super::Value],
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    let mut at = 0usize;
    let mut failure = None;
    let count = arguments.len();
    while at < count && failure.is_none() {
        let ty = operation.parameters[at];
        if let Err(error) = super::results::validate(ty, &arguments[at], buffer) {
            failure = Some(error);
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn read_arguments(
    arguments: &[super::Value],
    state: &mut super::State,
) -> Result<(), crate::Diagnostic> {
    let mut at = 0usize;
    let mut failure = None;
    let count = arguments.len();
    while at < count && failure.is_none() {
        if let Err(error) = state.read(&arguments[at]) {
            failure = Some(error);
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn arguments(
    operation: &noble_contracts::component::Operation,
    state: &mut super::State,
) -> Result<alloc::vec::Vec<super::Value>, crate::Diagnostic> {
    let mut arguments = alloc::vec::Vec::with_capacity(operation.parameters.len());
    let mut at = operation.parameters.len();
    let mut failure = None;
    while at > 0 && failure.is_none() {
        at = at.saturating_sub(1);
        let ty = operation.parameters[at];
        match argument(ty, state) {
            Ok(value) => arguments.push(value),
            Err(error) => failure = Some(error),
        }
    }
    if let Some(error) = failure {
        return Err(error);
    }
    arguments.reverse();
    Ok(arguments)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; argument admission uses runtime stack pop and non-const polymorphic type equality before any host call is emitted."
)]
fn argument(
    ty: noble_contracts::component::Type,
    state: &mut super::State,
) -> Result<super::Value, crate::Diagnostic> {
    let value = attempt!(state.pop());
    if value.ty != ty.noble() {
        return Err(crate::Diagnostic::Invalid);
    }
    Ok(value)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the bounded owning iterator is consumed exactly once and only declared borrowed owners are restored in order; malformed iteration is Defective, not an assertion or implicit owner return."
)]
fn restore_borrows(
    operation: &noble_contracts::component::Operation,
    arguments: alloc::vec::Vec<super::Value>,
    state: &mut super::State,
) -> Result<(), crate::Diagnostic> {
    let count = arguments.len();
    let mut arguments = arguments.into_iter();
    let mut at = 0usize;
    let mut failure = None;
    while at < count && failure.is_none() {
        match arguments.next() {
            Some(value) => {
                let ty = operation.parameters[at];
                if matches!(ty, noble_contracts::component::Type::Borrow(_)) {
                    state.stack.push(value);
                }
            }
            None => failure = Some(crate::Diagnostic::Defective),
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
