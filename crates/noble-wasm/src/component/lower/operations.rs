#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; native lowering updates only compiler-owned temporary locals, operand stack, data segments and bounded output bytes; borrowed source, literal metadata and WIT operations remain immutable."
)]

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; node lookup, supported opcode shape and stack ceiling are checked through typed errors before publishing code; producer indexes are not assertion preconditions."
)]
pub(super) fn node(
    id: noble_kernel::untrusted::NodeId,
    body: &noble_kernel::execution::Body,
    world: &noble_contracts::component::World,
    data: &mut super::Data,
    state: &mut super::State,
) -> Result<(), crate::Diagnostic> {
    let index = match usize::try_from(id.0) {
        Ok(index) => index,
        Err(_) => return Err(crate::Diagnostic::Invalid),
    };
    let node = match body.candidate.nodes.get(index) {
        Some(node) => node,
        None => return Err(crate::Diagnostic::Invalid),
    };
    match node {
        noble_kernel::untrusted::Node::Literal { lit, .. } => {
            attempt!(literal(*lit, id, body, data, state))
        }
        noble_kernel::untrusted::Node::Invocation { def, .. } => {
            attempt!(invoke(*def, world, state))
        }
        noble_kernel::untrusted::Node::Quotation { .. } => {
            return Err(crate::Diagnostic::Unsupported)
        }
    }
    if state.stack.len() > super::super::STACK_LIMIT {
        return Err(crate::Diagnostic::Exhausted);
    }
    Ok(())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; literal lowering allocates compiler locals and writes the bounded non-const instruction sink, including retained text data segments."
)]
fn literal(
    lit: noble_kernel::untrusted::Lit,
    id: noble_kernel::untrusted::NodeId,
    body: &noble_kernel::execution::Body,
    data: &mut super::Data,
    state: &mut super::State,
) -> Result<(), crate::Diagnostic> {
    let value = attempt!(state.value(lit.ty()));
    match lit {
        noble_kernel::untrusted::Lit::Unit => {}
        noble_kernel::untrusted::Lit::Bool(value) => {
            attempt!(state.code.i32(u32::from(value)));
        }
        noble_kernel::untrusted::Lit::I64(value) => {
            attempt!(state.code.i64(value));
        }
        noble_kernel::untrusted::Lit::Text => attempt!(text(id, body, data, &mut state.code)),
    }
    attempt!(state.capture(&value));
    state.stack.push(value);
    Ok(())
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; checked text lowering resolves borrowed metadata and clones the one retained data segment before writing the allocating bounded sink; address overflow and heap overlap remain diagnostics."
)]
fn text(
    id: noble_kernel::untrusted::NodeId,
    body: &noble_kernel::execution::Body,
    data: &mut super::Data,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    let position = body.texts.iter().position(|text| text.node == id);
    let index = match position {
        Some(index) => index,
        None => return Err(crate::Diagnostic::Invalid),
    };
    let bytes = match body.texts.get(index) {
        Some(text) => &text.bytes,
        None => return Err(crate::Diagnostic::Invalid),
    };
    let length_bytes = match u32::try_from(bytes.len()) {
        Ok(length) => length,
        Err(_) => return Err(crate::Diagnostic::Exhausted),
    };
    let start = data.end;
    data.end = match data.end.checked_add(length_bytes) {
        Some(end) if end < super::super::HEAP_START => end,
        Some(_) | None => return Err(crate::Diagnostic::Exhausted),
    };
    data.segments.push((start, bytes.clone()));
    attempt!(buffer.i32(start));
    buffer.i32(length_bytes)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; invocation lowering performs runtime stack/type checks, allocates locals and emits bounded import or arithmetic instructions."
)]
fn invoke(
    definition: noble_kernel::contracts::Definition,
    world: &noble_contracts::component::World,
    state: &mut super::State,
) -> Result<(), crate::Diagnostic> {
    match definition.0 {
        0 => {
            let value = attempt!(state.pop());
            // Independent kernel acceptance enforces Data; lowering also
            // rejects handle duplication rather than trusting machine width.
            if !value.ty.is_data() {
                return Err(crate::Diagnostic::Invalid);
            }
            state.stack.push(value.clone());
            state.stack.push(value);
            Ok(())
        }
        1 => {
            let value = attempt!(state.pop());
            if !value.ty.is_data() {
                return Err(crate::Diagnostic::Invalid);
            }
            Ok(())
        }
        2 => {
            let right = attempt!(state.pop());
            let left = attempt!(state.pop());
            state.stack.push(right);
            state.stack.push(left);
            Ok(())
        }
        4..=7 => arithmetic(definition.0, state),
        12 => {
            let value = attempt!(state.value(noble_kernel::types::Ty::Unit));
            state.stack.push(value);
            Ok(())
        }
        24.. => {
            let imports = world.imports();
            let position = imports
                .iter()
                .position(|operation| operation.definition == Some(definition));
            let index = match position {
                Some(index) => index,
                None => return Err(crate::Diagnostic::Invalid),
            };
            let operation = match imports.get(index) {
                Some(operation) => operation,
                None => return Err(crate::Diagnostic::Invalid),
            };
            super::calls::invoke(operation, state)
        }
        _ => Err(crate::Diagnostic::Unsupported),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; arithmetic lowering checks non-const structural type equality and allocates private locals before emitting native instructions through the bounded sink; invalid operands return diagnostics."
)]
fn arithmetic(definition: u32, state: &mut super::State) -> Result<(), crate::Diagnostic> {
    let right = attempt!(state.pop());
    let left = attempt!(state.pop());
    if right.ty != noble_kernel::types::Ty::I64 {
        return Err(crate::Diagnostic::Invalid);
    }
    if left.ty != noble_kernel::types::Ty::I64 {
        return Err(crate::Diagnostic::Invalid);
    }
    let ty = if definition == 7 {
        noble_kernel::types::Ty::Bool
    } else {
        noble_kernel::types::Ty::I64
    };
    let result = attempt!(state.value(ty));
    attempt!(state.read(&left));
    attempt!(state.read(&right));
    attempt!(arithmetic_instruction(definition, &mut state.code));
    attempt!(state.capture(&result));
    state.stack.push(result);
    Ok(())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; instruction selection writes the non-const bounded output buffer and propagates allocation exhaustion."
)]
fn arithmetic_instruction(
    definition: u32,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    match definition {
        4 => buffer.append(b" i64.add\n"),
        5 => buffer.append(b" i64.sub\n"),
        6 => buffer.append(b" i64.mul\n"),
        7 => buffer.append(b" i64.eq\n"),
        _ => Err(crate::Diagnostic::Invalid),
    }
}
