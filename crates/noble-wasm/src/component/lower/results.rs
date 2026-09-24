#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; canonical result lowering writes only the compiler-owned local table, operand stack and bounded byte sink; dynamic checks are emitted into the guest and borrowed signatures and source values stay immutable."
)]

pub(super) fn validate(
    ty: noble_contracts::component::Type,
    value: &super::Value,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    match ty {
        noble_contracts::component::Type::Boolean => {
            attempt!(read_lane(value, 0, buffer));
            buffer.append(b" i32.const 1 i32.gt_u if unreachable end\n")
        }
        noble_contracts::component::Type::S64
        | noble_contracts::component::Type::StreamU8
        | noble_contracts::component::Type::FutureS64
        | noble_contracts::component::Type::FutureResultS64String
        | noble_contracts::component::Type::Own(_)
        | noble_contracts::component::Type::Borrow(_) => Ok(()),
        noble_contracts::component::Type::String | noble_contracts::component::Type::Bytes => {
            attempt!(read_lane(value, 0, buffer));
            attempt!(read_lane(value, 1, buffer));
            if ty == noble_contracts::component::Type::String {
                buffer.append(b" call $utf8\n")
            } else {
                buffer.append(b" call $range\n")
            }
        }
        noble_contracts::component::Type::ResultS64String => {
            attempt!(read_lane(value, 0, buffer));
            attempt!(buffer.append(b" i32.const 1 i32.gt_u if unreachable end\n"));
            attempt!(read_lane(value, 0, buffer));
            attempt!(buffer.append(b" if\n"));
            attempt!(read_lane(value, 1, buffer));
            attempt!(buffer.append(b" i64.const 4294967295 i64.gt_u if unreachable end\n"));
            attempt!(read_lane(value, 1, buffer));
            attempt!(buffer.append(b" i32.wrap_i64\n"));
            attempt!(read_lane(value, 2, buffer));
            buffer.append(b" call $utf8 end\n")
        }
        noble_contracts::component::Type::ResultBytesString => {
            attempt!(read_lane(value, 0, buffer));
            attempt!(buffer.append(b" i32.const 1 i32.gt_u if unreachable end\n"));
            attempt!(read_lane(value, 0, buffer));
            attempt!(buffer.append(b" if\n"));
            attempt!(read_lane(value, 1, buffer));
            attempt!(read_lane(value, 2, buffer));
            attempt!(buffer.append(b" call $utf8\n else\n"));
            attempt!(read_lane(value, 1, buffer));
            attempt!(read_lane(value, 2, buffer));
            buffer.append(b" call $range\n end\n")
        }
    }
}

pub(super) fn allocate(
    ty: noble_contracts::component::Type,
    state: &mut super::State,
) -> Result<u32, crate::Diagnostic> {
    allocate_layout(super::super::abi::memory_layout(ty), state)
}

pub(super) fn allocate_layout(
    layout: (u32, u32),
    state: &mut super::State,
) -> Result<u32, crate::Diagnostic> {
    let (align, size_bytes) = layout;
    let area = attempt!(state.local(super::super::abi::Lane::I32));
    attempt!(state.code.i32(align));
    attempt!(state.code.i32(size_bytes));
    attempt!(state.code.append(b" call $alloc\n"));
    attempt!(super::super::abi::set(&mut state.code, area));
    Ok(area)
}

pub(super) fn load(
    ty: Option<noble_contracts::component::Type>,
    area: u32,
    value: &super::Value,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    match ty {
        Some(noble_contracts::component::Type::Boolean) => {
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load8_u\n"));
            capture_lane(value, 0, buffer)
        }
        Some(noble_contracts::component::Type::S64) => {
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i64.load\n"));
            capture_lane(value, 0, buffer)
        }
        Some(
            noble_contracts::component::Type::Own(_)
            | noble_contracts::component::Type::Borrow(_)
            | noble_contracts::component::Type::StreamU8
            | noble_contracts::component::Type::FutureS64
            | noble_contracts::component::Type::FutureResultS64String,
        ) => {
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load\n"));
            capture_lane(value, 0, buffer)
        }
        Some(
            noble_contracts::component::Type::String | noble_contracts::component::Type::Bytes,
        ) => {
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load\n"));
            attempt!(capture_lane(value, 0, buffer));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load offset=4\n"));
            capture_lane(value, 1, buffer)
        }
        Some(noble_contracts::component::Type::ResultS64String) => {
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load8_u\n"));
            attempt!(capture_lane(value, 0, buffer));
            attempt!(read_lane(value, 0, buffer));
            attempt!(buffer.append(b" i32.eqz if\n"));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i64.load offset=8\n"));
            attempt!(capture_lane(value, 1, buffer));
            attempt!(buffer.append(b" else\n"));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load offset=8 i64.extend_i32_u\n"));
            attempt!(capture_lane(value, 1, buffer));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load offset=12\n"));
            attempt!(capture_lane(value, 2, buffer));
            buffer.append(b" end\n")
        }
        Some(noble_contracts::component::Type::ResultBytesString) => {
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load8_u\n"));
            attempt!(capture_lane(value, 0, buffer));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load offset=4\n"));
            attempt!(capture_lane(value, 1, buffer));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load offset=8\n"));
            capture_lane(value, 2, buffer)
        }
        None => Err(crate::Diagnostic::Defective),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the complete result stack, exact type, value validity, copy and storage operations are checked before emission completes; partial trusted result publication is not an assertion-based fallback."
)]
pub(super) fn finish(
    result: Option<noble_contracts::component::Type>,
    asynchronous: bool,
    state: &mut super::State,
) -> Result<(), crate::Diagnostic> {
    let ty = match result {
        Some(ty) => ty,
        None => {
            if state.stack.is_empty() {
                return Ok(());
            }
            return Err(crate::Diagnostic::Invalid);
        }
    };
    let value = attempt!(state.pop());
    if !state.stack.is_empty() {
        return Err(crate::Diagnostic::Invalid);
    }
    if value.ty != ty.noble() {
        return Err(crate::Diagnostic::Invalid);
    }
    attempt!(validate(ty, &value, &mut state.code));
    // task.return lifts its parameters synchronously, including pointed-to
    // strings/lists, before returning control to guest cleanup. It uses the
    // sixteen-lane parameter ABI, not the synchronous one-result return ABI.
    if asynchronous || super::super::abi::lane_count(ty) == 1 {
        return state.read(&value);
    }
    attempt!(copy_result(ty, &value, &mut state.code));
    let area = attempt!(allocate(ty, state));
    attempt!(store(ty, area, &value, &mut state.code));
    super::super::abi::get(&mut state.code, area)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; canonical aggregate results emit allocating guest copy instructions through the bounded non-const compiler sink; explicit scalar rejection prevents a new type from silently inheriting a representation."
)]
fn copy_result(
    ty: noble_contracts::component::Type,
    value: &super::Value,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    match ty {
        noble_contracts::component::Type::String | noble_contracts::component::Type::Bytes => {
            attempt!(read_lane(value, 0, buffer));
            attempt!(read_lane(value, 1, buffer));
            attempt!(buffer.append(b" call $copy\n"));
            capture_lane(value, 0, buffer)
        }
        noble_contracts::component::Type::ResultS64String => {
            attempt!(read_lane(value, 0, buffer));
            attempt!(buffer.append(b" if\n"));
            attempt!(read_lane(value, 1, buffer));
            attempt!(buffer.append(b" i32.wrap_i64\n"));
            attempt!(read_lane(value, 2, buffer));
            attempt!(buffer.append(b" call $copy i64.extend_i32_u\n"));
            attempt!(capture_lane(value, 1, buffer));
            buffer.append(b" end\n")
        }
        noble_contracts::component::Type::ResultBytesString => {
            attempt!(read_lane(value, 1, buffer));
            attempt!(read_lane(value, 2, buffer));
            attempt!(buffer.append(b" call $copy\n"));
            capture_lane(value, 1, buffer)
        }
        noble_contracts::component::Type::Boolean
        | noble_contracts::component::Type::S64
        | noble_contracts::component::Type::StreamU8
        | noble_contracts::component::Type::FutureS64
        | noble_contracts::component::Type::FutureResultS64String
        | noble_contracts::component::Type::Own(_)
        | noble_contracts::component::Type::Borrow(_) => Err(crate::Diagnostic::Defective),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; canonical storage emits exact scalar widths, discriminant and payload offsets through the non-const bounded sink; checked lane lookup propagates diagnostics rather than asserting on plans."
)]
pub(super) fn store(
    ty: noble_contracts::component::Type,
    area: u32,
    value: &super::Value,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    attempt!(super::super::abi::get(buffer, area));
    attempt!(read_lane(value, 0, buffer));
    match ty {
        noble_contracts::component::Type::Boolean => buffer.append(b" i32.store8\n"),
        noble_contracts::component::Type::S64 => buffer.append(b" i64.store\n"),
        noble_contracts::component::Type::Own(_)
        | noble_contracts::component::Type::Borrow(_)
        | noble_contracts::component::Type::StreamU8
        | noble_contracts::component::Type::FutureS64
        | noble_contracts::component::Type::FutureResultS64String => buffer.append(b" i32.store\n"),
        noble_contracts::component::Type::String | noble_contracts::component::Type::Bytes => {
            attempt!(buffer.append(b" i32.store\n"));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(read_lane(value, 1, buffer));
            buffer.append(b" i32.store offset=4\n")
        }
        noble_contracts::component::Type::ResultS64String => {
            attempt!(buffer.append(b" i32.store\n"));
            attempt!(read_lane(value, 0, buffer));
            attempt!(buffer.append(b" i32.eqz if\n"));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(read_lane(value, 1, buffer));
            attempt!(buffer.append(b" i64.store offset=8\n else\n"));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(read_lane(value, 1, buffer));
            attempt!(buffer.append(b" i32.wrap_i64 i32.store offset=8\n"));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(read_lane(value, 2, buffer));
            buffer.append(b" i32.store offset=12\n end\n")
        }
        noble_contracts::component::Type::ResultBytesString => {
            attempt!(buffer.append(b" i32.store8\n"));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(read_lane(value, 1, buffer));
            attempt!(buffer.append(b" i32.store offset=4\n"));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(read_lane(value, 2, buffer));
            buffer.append(b" i32.store offset=8\n")
        }
    }
}

fn read_lane(
    value: &super::Value,
    at: usize,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    let local = attempt!(super::slot(value, at));
    super::super::abi::get(buffer, local)
}

fn capture_lane(
    value: &super::Value,
    at: usize,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    let local = attempt!(super::slot(value, at));
    super::super::abi::set(buffer, local)
}
