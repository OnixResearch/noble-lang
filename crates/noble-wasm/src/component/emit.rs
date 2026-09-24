#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; emission mutates only the fresh compiler-owned bounded byte sink; checked plans, data segments and regenerated WIT bindings remain immutable and no partial module escapes."
)]

mod task;

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; independently admitted imports, plans and data segments are emitted in their fixed order; every bounded sink failure propagates a diagnostic, and asserting on producer-derived contents would replace required refusals."
)]
pub(super) fn module(
    world: &noble_contracts::component::World,
    plans: &[super::lower::Plan],
    data: &super::lower::Data,
) -> Result<alloc::vec::Vec<u8>, crate::Diagnostic> {
    let mut buffer = crate::output::Buffer::new(16_384);
    attempt!(buffer.append(b"(module\n"));
    let mut at = 0usize;
    let mut failure = None;
    let operation_count = world.imports().len();
    while at < operation_count && failure.is_none() {
        if let Err(error) = import(&world.imports()[at], &mut buffer) {
            failure = Some(error);
        }
        at = at.saturating_add(1);
    }
    if let Some(error) = failure {
        return Err(error);
    }
    if world.is_async() {
        attempt!(async_imports(plans, &mut buffer));
    }
    attempt!(buffer.append(include_str!("runtime.wat").as_bytes()));
    if world.is_async() {
        attempt!(buffer.append(include_str!("async.wat").as_bytes()));
    }
    at = 0;
    let plan_count = plans.len();
    while at < plan_count && failure.is_none() {
        if let Err(error) = function(&plans[at], &mut buffer) {
            failure = Some(error);
        }
        at = at.saturating_add(1);
    }
    if let Some(error) = failure {
        return Err(error);
    }
    at = 0;
    let segment_count = data.segments.len();
    while at < segment_count && failure.is_none() {
        if let Err(error) = data_segment(data, at, &mut buffer) {
            failure = Some(error);
        }
        at = at.saturating_add(1);
    }
    if let Some(error) = failure {
        return Err(error);
    }
    attempt!(buffer.append(b")\n"));
    Ok(buffer.finish())
}

fn data_segment(
    data: &super::lower::Data,
    at: usize,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    let (offset, bytes) = &data.segments[at];
    segment(*offset, bytes, buffer)
}

fn segment(
    offset_bytes: u32,
    bytes: &[u8],
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    attempt!(buffer.append(b" (data "));
    attempt!(buffer.i32(offset_bytes));
    attempt!(buffer.append(b" "));
    attempt!(quoted(bytes, buffer));
    buffer.append(b")\n")
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; import signatures require checked canonical parameter flattening and writes to the non-const bounded output sink; missing definitions and unsupported result arities remain diagnostics."
)]
fn import(
    operation: &noble_contracts::component::Operation,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    let definition = match operation.definition {
        Some(definition) => definition,
        None => return Err(crate::Diagnostic::Invalid),
    };
    attempt!(buffer.append(b" (import "));
    attempt!(quoted(operation.core_module.as_bytes(), buffer));
    attempt!(buffer.append(b" "));
    let prefix: &[u8] = if operation.asynchronous {
        b"[async-lower]"
    } else {
        b""
    };
    attempt!(quoted_prefixed(
        prefix,
        operation.core_name.as_bytes(),
        buffer
    ));
    attempt!(buffer.append(b" (func $i"));
    attempt!(buffer.number(u64::from(definition.0)));
    if super::abi::indirect_parameters(operation) {
        attempt!(buffer.append(b" (param i32)"));
    } else {
        attempt!(super::abi::parameters(&operation.parameters, buffer));
    }
    let returned = attempt!(super::abi::result(&operation.results));
    if operation.asynchronous {
        if returned.is_some() {
            attempt!(buffer.append(b" (param i32)"));
        }
        attempt!(buffer.append(b" (result i32)"));
    } else {
        match returned {
            Some(ty) if super::abi::lane_count(ty) > 1 => attempt!(buffer.append(b" (param i32)")),
            Some(ty) => attempt!(result(Some(ty), buffer)),
            None => {}
        }
    }
    buffer.append(b"))\n")
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; async intrinsic signatures are fixed canonical ABI declarations and each bounded sink write propagates exhaustion; task-return signatures come from checked plans rather than assertion preconditions."
)]
fn async_imports(
    plans: &[super::lower::Plan],
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    attempt!(buffer.append(
        b" (import \"$root\" \"[waitable-set-new]\" (func $waitable-set-new (result i32)))\n\
           (import \"$root\" \"[waitable-set-wait]\" (func $waitable-set-wait (param i32 i32) (result i32)))\n\
           (import \"$root\" \"[waitable-set-drop]\" (func $waitable-set-drop (param i32)))\n\
           (import \"$root\" \"[waitable-join]\" (func $waitable-join (param i32 i32)))\n\
           (import \"$root\" \"[subtask-drop]\" (func $subtask-drop (param i32)))\n"
    ));
    let mut at = 0usize;
    let mut failure = None;
    let count = plans.len();
    while at < count && failure.is_none() {
        let plan = &plans[at];
        if plan.asynchronous {
            if let Err(error) = task::write(plan, buffer) {
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

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; checked function plans retain their exact canonical parameters, locals, entry and post-return cleanup; bounded sink writes preserve exhaustion diagnostics rather than asserting on module input."
)]
fn function(
    plan: &super::lower::Plan,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    attempt!(buffer.append(b" (func (export \""));
    if plan.asynchronous {
        attempt!(buffer.append(b"[async-lift-stackful]"));
    } else {
        // Keep Standard32 for synchronous members, including in mixed worlds,
        // so legal WIT names cannot collide with runtime ABI exports.
        attempt!(buffer.append(b"cm32p2||"));
    }
    attempt!(buffer.append(plan.name.as_bytes()));
    attempt!(buffer.append(b"\")"));
    attempt!(super::abi::parameters(&plan.parameters, buffer));
    if !plan.asynchronous {
        attempt!(result(plan.result, buffer));
    }
    let mut at = 0usize;
    let mut failure = None;
    let end = plan.locals.len();
    while at < end && failure.is_none() {
        let lane = plan.locals[at];
        if let Err(error) = local(lane, buffer) {
            failure = Some(error);
        }
        at = at.saturating_add(1);
    }
    if let Some(error) = failure {
        return Err(error);
    }
    attempt!(buffer.append(b"\n call $enter\n"));
    attempt!(buffer.append(&plan.code));
    attempt!(buffer.append(b")\n"));
    // Native task.return lifts the result before guest cleanup; the async ABI
    // forbids post-return. Synchronous members of an async world still use it.
    if plan.asynchronous {
        return Ok(());
    }
    attempt!(buffer.append(b" (func (export \"cm32p2||"));
    attempt!(buffer.append(plan.name.as_bytes()));
    attempt!(buffer.append(b"_post\")"));
    let result_type = plan.result;
    if let Some(ty) = result_type {
        attempt!(buffer.append(b" (param "));
        attempt!(attempt!(super::abi::lane(ty, 0)).write(buffer));
        attempt!(buffer.append(b")"));
    }
    buffer.append(b" call $cleanup)\n")
}

fn local(
    lane: super::abi::Lane,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    attempt!(buffer.append(b" (local "));
    attempt!(lane.write(buffer));
    buffer.append(b")")
}

fn result(
    ty: Option<noble_contracts::component::Type>,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    if let Some(ty) = ty {
        attempt!(buffer.append(b" (result "));
        attempt!(attempt!(super::abi::lane(ty, 0)).write(buffer));
        attempt!(buffer.append(b")"));
    }
    Ok(())
}

fn quoted(bytes: &[u8], buffer: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
    quoted_prefixed(b"", bytes, buffer)
}

fn quoted_prefixed(
    prefix: &[u8],
    bytes: &[u8],
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    attempt!(buffer.append(b"\""));
    attempt!(escaped(prefix, buffer));
    attempt!(escaped(bytes, buffer));
    buffer.append(b"\"")
}

fn escaped(bytes: &[u8], buffer: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
    let mut at = 0usize;
    let mut failure = None;
    let end = bytes.len();
    while at < end && failure.is_none() {
        let byte = bytes[at];
        if let Err(error) = quoted_byte(byte, buffer) {
            failure = Some(error);
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn quoted_byte(byte: u8, buffer: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
    let escaped = [b'\\', hex(byte >> 4), hex(byte & 15)];
    buffer.append(&escaped)
}

const fn hex(nibble: u8) -> u8 {
    match nibble {
        0..=9 => b'0'.saturating_add(nibble),
        _ => b'a'.saturating_add(nibble.saturating_sub(10)),
    }
}
