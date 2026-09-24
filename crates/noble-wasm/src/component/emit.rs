#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; emission mutates only the fresh compiler-owned bounded byte sink; checked plans, data segments and regenerated WIT bindings remain immutable and no partial module escapes."
)]

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
    attempt!(buffer.append(include_str!("runtime.wat").as_bytes()));
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
    attempt!(quoted(operation.core_name.as_bytes(), buffer));
    attempt!(buffer.append(b" (func $i"));
    attempt!(buffer.number(u64::from(definition.0)));
    attempt!(super::abi::parameters(&operation.parameters, buffer));
    match attempt!(super::abi::result(&operation.results)) {
        Some(ty) if super::abi::lane_count(ty) > 1 => attempt!(buffer.append(b" (param i32)")),
        Some(ty) => attempt!(result(Some(ty), buffer)),
        None => {}
    }
    buffer.append(b"))\n")
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; checked function plans retain their exact canonical parameters, locals, entry and post-return cleanup; bounded sink writes preserve exhaustion diagnostics rather than asserting on module input."
)]
fn function(
    plan: &super::lower::Plan,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    // Standard32 function names cannot collide with runtime ABI exports.
    attempt!(buffer.append(b" (func (export \"cm32p2||"));
    attempt!(buffer.append(plan.name.as_bytes()));
    attempt!(buffer.append(b"\")"));
    attempt!(super::abi::parameters(&plan.parameters, buffer));
    attempt!(result(plan.result, buffer));
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
    attempt!(buffer.append(b" (func (export \"cm32p2||"));
    attempt!(buffer.append(plan.name.as_bytes()));
    attempt!(buffer.append(b"_post\")"));
    let result_type = plan.result;
    if let Some(ty) = result_type {
        attempt!(buffer.append(b" (param "));
        attempt!(result_lane(ty).write(buffer));
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
        attempt!(result_lane(ty).write(buffer));
        attempt!(buffer.append(b")"));
    }
    Ok(())
}

const fn result_lane(ty: noble_contracts::component::Type) -> super::abi::Lane {
    match ty {
        noble_contracts::component::Type::S64 => super::abi::Lane::I64,
        noble_contracts::component::Type::Boolean
        | noble_contracts::component::Type::String
        | noble_contracts::component::Type::Bytes
        | noble_contracts::component::Type::ResultS64String
        | noble_contracts::component::Type::Own(_)
        | noble_contracts::component::Type::Borrow(_) => super::abi::Lane::I32,
    }
}

fn quoted(bytes: &[u8], buffer: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
    attempt!(buffer.append(b"\""));
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
        None => buffer.append(b"\""),
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
