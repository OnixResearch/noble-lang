#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; ABI emission writes only the fresh compiler-owned bounded byte sink; borrowed WIT types and caller input remain unchanged and no partial output escapes compilation."
)]

/// The closed integer-lane vocabulary of the canonical32 profile.
#[derive(Clone, Copy)]
#[octet::sealed_enum]
pub(super) enum Lane {
    I32,
    I64,
}
impl Lane {
    pub(super) fn write(self, buffer: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
        match self {
            Self::I32 => buffer.append(b"i32"),
            Self::I64 => buffer.append(b"i64"),
        }
    }
}

pub(super) const fn lane_count(ty: noble_contracts::component::Type) -> usize {
    match ty {
        noble_contracts::component::Type::Boolean
        | noble_contracts::component::Type::S64
        | noble_contracts::component::Type::Own(_)
        | noble_contracts::component::Type::Borrow(_) => 1,
        noble_contracts::component::Type::String | noble_contracts::component::Type::Bytes => 2,
        noble_contracts::component::Type::ResultS64String => 3,
    }
}

pub(super) const fn lane(
    ty: noble_contracts::component::Type,
    at: usize,
) -> Result<Lane, crate::Diagnostic> {
    if at >= lane_count(ty) {
        return Err(crate::Diagnostic::Defective);
    }
    match ty {
        noble_contracts::component::Type::S64 => Ok(Lane::I64),
        noble_contracts::component::Type::ResultS64String if at == 1 => Ok(Lane::I64),
        noble_contracts::component::Type::Boolean
        | noble_contracts::component::Type::String
        | noble_contracts::component::Type::Bytes
        | noble_contracts::component::Type::ResultS64String
        | noble_contracts::component::Type::Own(_)
        | noble_contracts::component::Type::Borrow(_) => Ok(Lane::I32),
    }
}

pub(super) fn value_type(
    ty: &noble_kernel::types::Ty,
) -> Result<Option<noble_contracts::component::Type>, crate::Diagnostic> {
    match ty {
        noble_kernel::types::Ty::Unit => Ok(None),
        noble_kernel::types::Ty::Bool => Ok(Some(noble_contracts::component::Type::Boolean)),
        noble_kernel::types::Ty::I64 => Ok(Some(noble_contracts::component::Type::S64)),
        noble_kernel::types::Ty::Text => Ok(Some(noble_contracts::component::Type::String)),
        noble_kernel::types::Ty::Resource(kind) => {
            Ok(Some(noble_contracts::component::Type::Own(*kind)))
        }
        noble_kernel::types::Ty::List(item) => {
            if **item != noble_kernel::types::Ty::I64 {
                return Err(crate::Diagnostic::Unsupported);
            }
            Ok(Some(noble_contracts::component::Type::Bytes))
        }
        noble_kernel::types::Ty::Sum(ok, error) => {
            if **ok != noble_kernel::types::Ty::I64 {
                return Err(crate::Diagnostic::Unsupported);
            }
            if **error != noble_kernel::types::Ty::Text {
                return Err(crate::Diagnostic::Unsupported);
            }
            Ok(Some(noble_contracts::component::Type::ResultS64String))
        }
        noble_kernel::types::Ty::Syntax
        | noble_kernel::types::Ty::Contract
        | noble_kernel::types::Ty::Evidence
        | noble_kernel::types::Ty::Certified
        | noble_kernel::types::Ty::Pair(_, _)
        | noble_kernel::types::Ty::Program(_, _, _) => Err(crate::Diagnostic::Unsupported),
    }
}

pub(super) const fn memory_layout(ty: noble_contracts::component::Type) -> (u32, u32) {
    match ty {
        noble_contracts::component::Type::Boolean => (1, 1),
        noble_contracts::component::Type::Own(_) | noble_contracts::component::Type::Borrow(_) => {
            (4, 4)
        }
        noble_contracts::component::Type::S64 => (8, 8),
        noble_contracts::component::Type::String | noble_contracts::component::Type::Bytes => {
            (4, 8)
        }
        noble_contracts::component::Type::ResultS64String => (8, 16),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; bounded canonical flattening propagates output-buffer failures and rejects excess lanes through diagnostics rather than assertions over a caller signature."
)]
pub(super) fn parameters(
    types: &[noble_contracts::component::Type],
    buffer: &mut crate::output::Buffer,
) -> Result<usize, crate::Diagnostic> {
    let mut count = 0usize;
    let mut at = 0usize;
    let mut failure = None;
    let end = types.len();
    while at < end && failure.is_none() {
        let ty = types[at];
        match parameter_type(ty, buffer) {
            Ok(lanes) => count = count.saturating_add(lanes),
            Err(error) => failure = Some(error),
        }
        at = at.saturating_add(1);
    }
    if let Some(error) = failure {
        return Err(error);
    }
    if count > super::FLAT_PARAMETER_LIMIT {
        return Err(crate::Diagnostic::Unsupported);
    }
    Ok(count)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; each canonical lane is checked and written through a fallible bounded sink; unsupported layouts and exhausted output are diagnostics, not assertion preconditions."
)]
fn parameter_type(
    ty: noble_contracts::component::Type,
    buffer: &mut crate::output::Buffer,
) -> Result<usize, crate::Diagnostic> {
    let count = lane_count(ty);
    let mut at = 0usize;
    let mut failure = None;
    while at < count && failure.is_none() {
        match lane(ty, at) {
            Ok(kind) => {
                if let Err(error) = parameter(kind, buffer) {
                    failure = Some(error);
                }
            }
            Err(error) => failure = Some(error),
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(count),
    }
}

fn parameter(lane: Lane, buffer: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
    attempt!(buffer.append(b" (param "));
    attempt!(lane.write(buffer));
    buffer.append(b")")
}

pub(super) fn result(
    types: &[noble_contracts::component::Type],
) -> Result<Option<noble_contracts::component::Type>, crate::Diagnostic> {
    if types.len() > 1 {
        return Err(crate::Diagnostic::Unsupported);
    }
    Ok(types.first().copied())
}

pub(super) fn get(buffer: &mut crate::output::Buffer, local: u32) -> Result<(), crate::Diagnostic> {
    attempt!(buffer.append(b" local.get "));
    attempt!(buffer.number(u64::from(local)));
    buffer.append(b"\n")
}
pub(super) fn set(buffer: &mut crate::output::Buffer, local: u32) -> Result<(), crate::Diagnostic> {
    attempt!(buffer.append(b" local.set "));
    attempt!(buffer.number(u64::from(local)));
    buffer.append(b"\n")
}
