//! Pure preflight and native-copy decisions for the synchronous bounded-region host.
//! The shell alone owns the table, allocation, copy, completion and report effects.

pub const MAX_REGION_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Representation,
    NoOwner,
    Identity,
    Bounds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub representation: u32,
    pub claim: crate::resources::Handle,
    pub retained: Option<crate::resources::Handle>,
    pub offset_bytes: i64,
    pub length_bytes: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub start: usize,
    pub finish: usize,
}

/// Refuse before a borrow or native work. Validate the claim against retained
/// table state, not merely against the caller's representation or owner copy.
#[expect(
    tigerstyle::usize_in_public_api,
    reason = "Owner: noble-maintainers; the range and storage length are adapter-local platform indices; signed guest arguments are converted only after checked addition."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every attacker-controlled preflight branch explicitly refuses before native work; redundant panic assertions are not an authority check."
)]
pub fn preflight(
    table: &crate::resources::Table,
    required: crate::resources::Requirement,
    request: Request,
    region_len: usize,
) -> Result<Range, Refusal> {
    if request.representation != 1 {
        return Err(Refusal::Representation);
    }
    let Some(actual) = request.retained else {
        return Err(Refusal::NoOwner);
    };
    if table.validate(request.claim, required).is_err() || request.claim != actual {
        return Err(Refusal::Identity);
    }
    let Some(end) = request.offset_bytes.checked_add(request.length_bytes) else {
        return Err(Refusal::Bounds);
    };
    let Ok(start) = usize::try_from(request.offset_bytes) else {
        return Err(Refusal::Bounds);
    };
    let Ok(count) = usize::try_from(request.length_bytes) else {
        return Err(Refusal::Bounds);
    };
    let Ok(finish) = usize::try_from(end) else {
        return Err(Refusal::Bounds);
    };
    if start > region_len || finish > region_len || count > MAX_REGION_BYTES {
        return Err(Refusal::Bounds);
    }
    Ok(Range { start, finish })
}

/// The copied interval must be the exact preflight interval of the admitted
/// owner. `native_access` is the table's separate authorization, not a report.
#[expect(
    tigerstyle::usize_in_public_api,
    reason = "Owner: noble-maintainers; these are adapter-local slice indices, bounded and checked by preflight before the host enters native work."
)]
pub fn authorize_copy(
    access: Result<crate::resources::Snapshot, crate::resources::Error>,
    claim: crate::resources::Handle,
    scope: crate::resources::Scope,
    range: Range,
    region_len: usize,
) -> bool {
    match access {
        Ok(snapshot) => {
            snapshot.handle == claim
                && scope.handle == claim
                && snapshot.state == crate::resources::State::Busy(scope.serial)
                && snapshot.last_scope == Some(scope.serial)
                && range.start <= range.finish
                && range.finish <= region_len
        }
        Err(_) => false,
    }
}
