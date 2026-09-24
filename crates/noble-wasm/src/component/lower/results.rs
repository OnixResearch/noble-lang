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
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 0))
            ));
            buffer.append(b" i32.const 1 i32.gt_u if unreachable end\n")
        }
        noble_contracts::component::Type::S64
        | noble_contracts::component::Type::Own(_)
        | noble_contracts::component::Type::Borrow(_) => Ok(()),
        noble_contracts::component::Type::String | noble_contracts::component::Type::Bytes => {
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 0))
            ));
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 1))
            ));
            if ty == noble_contracts::component::Type::String {
                buffer.append(b" call $utf8\n")
            } else {
                buffer.append(b" call $range\n")
            }
        }
        noble_contracts::component::Type::ResultS64String => {
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 0))
            ));
            attempt!(buffer.append(b" i32.const 1 i32.gt_u if unreachable end\n"));
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 0))
            ));
            attempt!(buffer.append(b" if\n"));
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 1))
            ));
            attempt!(buffer.append(b" i64.const 4294967295 i64.gt_u if unreachable end\n"));
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 1))
            ));
            attempt!(buffer.append(b" i32.wrap_i64\n"));
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 2))
            ));
            buffer.append(b" call $utf8 end\n")
        }
    }
}

pub(super) fn allocate(
    ty: noble_contracts::component::Type,
    state: &mut super::State,
) -> Result<u32, crate::Diagnostic> {
    let area = attempt!(state.local(super::super::abi::Lane::I32));
    let (align, size) = super::super::abi::memory_layout(ty);
    attempt!(state.code.i32(align));
    attempt!(state.code.i32(size));
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
        Some(
            noble_contracts::component::Type::String | noble_contracts::component::Type::Bytes,
        ) => {
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load\n"));
            attempt!(super::super::abi::set(
                buffer,
                attempt!(super::slot(value, 0))
            ));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load offset=4\n"));
            super::super::abi::set(buffer, attempt!(super::slot(value, 1)))
        }
        Some(noble_contracts::component::Type::ResultS64String) => {
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load8_u\n"));
            attempt!(super::super::abi::set(
                buffer,
                attempt!(super::slot(value, 0))
            ));
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 0))
            ));
            attempt!(buffer.append(b" i32.eqz if\n"));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i64.load offset=8\n"));
            attempt!(super::super::abi::set(
                buffer,
                attempt!(super::slot(value, 1))
            ));
            attempt!(buffer.append(b" else\n"));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load offset=8 i64.extend_i32_u\n"));
            attempt!(super::super::abi::set(
                buffer,
                attempt!(super::slot(value, 1))
            ));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(buffer.append(b" i32.load offset=12\n"));
            attempt!(super::super::abi::set(
                buffer,
                attempt!(super::slot(value, 2))
            ));
            buffer.append(b" end\n")
        }
        Some(
            noble_contracts::component::Type::Boolean
            | noble_contracts::component::Type::S64
            | noble_contracts::component::Type::Own(_)
            | noble_contracts::component::Type::Borrow(_),
        )
        | None => Err(crate::Diagnostic::Defective),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the complete result stack, exact type, value validity, copy and storage operations are checked before emission completes; partial trusted result publication is not an assertion-based fallback."
)]
pub(super) fn finish(
    result: Option<noble_contracts::component::Type>,
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
    if super::super::abi::lane_count(ty) == 1 {
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
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 0))
            ));
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 1))
            ));
            attempt!(buffer.append(b" call $copy\n"));
            super::super::abi::set(buffer, attempt!(super::slot(value, 0)))
        }
        noble_contracts::component::Type::ResultS64String => {
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 0))
            ));
            attempt!(buffer.append(b" if\n"));
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 1))
            ));
            attempt!(buffer.append(b" i32.wrap_i64\n"));
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 2))
            ));
            attempt!(buffer.append(b" call $copy i64.extend_i32_u\n"));
            attempt!(super::super::abi::set(
                buffer,
                attempt!(super::slot(value, 1))
            ));
            buffer.append(b" end\n")
        }
        noble_contracts::component::Type::Boolean
        | noble_contracts::component::Type::S64
        | noble_contracts::component::Type::Own(_)
        | noble_contracts::component::Type::Borrow(_) => Err(crate::Diagnostic::Defective),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; canonical aggregate storage emits exact discriminant and payload offsets through the non-const bounded sink; checked lane lookup and explicit scalar rejection propagate diagnostics rather than asserting on plans."
)]
fn store(
    ty: noble_contracts::component::Type,
    area: u32,
    value: &super::Value,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    attempt!(super::super::abi::get(buffer, area));
    attempt!(super::super::abi::get(
        buffer,
        attempt!(super::slot(value, 0))
    ));
    attempt!(buffer.append(b" i32.store\n"));
    match ty {
        noble_contracts::component::Type::String | noble_contracts::component::Type::Bytes => {
            attempt!(super::super::abi::get(buffer, area));
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 1))
            ));
            buffer.append(b" i32.store offset=4\n")
        }
        noble_contracts::component::Type::ResultS64String => {
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 0))
            ));
            attempt!(buffer.append(b" i32.eqz if\n"));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 1))
            ));
            attempt!(buffer.append(b" i64.store offset=8\n else\n"));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 1))
            ));
            attempt!(buffer.append(b" i32.wrap_i64 i32.store offset=8\n"));
            attempt!(super::super::abi::get(buffer, area));
            attempt!(super::super::abi::get(
                buffer,
                attempt!(super::slot(value, 2))
            ));
            buffer.append(b" i32.store offset=12\n end\n")
        }
        noble_contracts::component::Type::Boolean
        | noble_contracts::component::Type::S64
        | noble_contracts::component::Type::Own(_)
        | noble_contracts::component::Type::Borrow(_) => Err(crate::Diagnostic::Defective),
    }
}
