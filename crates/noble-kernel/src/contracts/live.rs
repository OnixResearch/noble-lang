//! Host-registered nominal resources and borrowed live-slot signatures.

fn resource_kind(decl: &super::NominalDecl) -> Option<crate::types::ResourceKind> {
    let crate::types::NominalShape::Opaque(representation) = &decl.shape else {
        return None;
    };
    let crate::types::Ty::Resource(kind) = &**representation else {
        return None;
    };
    Some(*kind)
}

pub(super) fn register_resource(
    mut env: super::Env,
    decl: super::NominalDecl,
) -> Result<super::Env, super::NominalError> {
    let kind = resource_kind(&decl).ok_or(super::NominalError::InvalidRepresentation)?;
    let is_live = env.live_slots;
    let is_exported = decl.exported;
    let is_unpublished = decl.public == [false, false];
    let is_fixture_kind = kind == super::FIXTURE_RESOURCE;
    let is_registered = env.resource_kinds.contains(&kind);
    let is_registrable = is_live && is_exported && is_unpublished;
    if !is_registrable || is_fixture_kind || is_registered {
        return Err(super::NominalError::InvalidRepresentation);
    }
    if env.nominal(decl.id).is_some() || env.generic_variant(decl.id).is_some() {
        return Err(super::NominalError::DuplicateIdentity);
    }
    env.resource_kinds.push(kind);
    env.live_resource_nominals.push(decl.id);
    env.nominals.push(decl);
    Ok(env)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-kernel; the invariants of this linear dispatcher are enforced by its callers and the kernel test suite"
)]
/// Check the catalog itself, not just its registration path: `super::Env` fields
/// are public and may have been altered after host registration.
pub(super) fn catalog_valid(env: &super::Env) -> bool {
    if !env.live_slots {
        return env.live_resource_nominals.is_empty();
    }
    if env.resource_kinds.len() != env.live_resource_nominals.len().saturating_add(1)
        || env.resource_kinds.first() != Some(&super::FIXTURE_RESOURCE)
    {
        return false;
    }
    let mut index = 0;
    let mut valid = true;
    while index < env.live_resource_nominals.len() && valid {
        valid = catalog_row_valid(env, index);
        index += 1;
    }
    valid
}

/// Validate one resource-catalog row against its declaration, its kind and every
/// earlier row. Loop-free so its early returns and the duplicate scan stay
/// extractable (Aeneas rejects returns/continue inside nested loops).
fn catalog_row_valid(env: &super::Env, index: usize) -> bool {
    let Some(decl) = env.nominal(env.live_resource_nominals[index]) else {
        return false;
    };
    if !decl.exported
        || decl.public != [false, false]
        || resource_kind(decl) != Some(env.resource_kinds[index.saturating_add(1)])
    {
        return false;
    }
    let mut prior = 0;
    let mut unique = true;
    while prior < index && unique {
        if env.resource_kinds[prior.saturating_add(1)]
            == env.resource_kinds[index.saturating_add(1)]
            || env.live_resource_nominals[prior] == env.live_resource_nominals[index]
        {
            unique = false;
        }
        prior += 1;
    }
    unique
}

pub(super) fn kind_is_slot_resource(env: &super::Env, kind: crate::types::ResourceKind) -> bool {
    if !env.live_slots {
        return false;
    }
    let mut index = 1;
    while index < env.resource_kinds.len() {
        if env.resource_kinds[index] == kind {
            return true;
        }
        index += 1;
    }
    false
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-kernel; the invariants of this linear dispatcher are enforced by its callers and the kernel test suite"
)]
/// Only root-positioned borrowed references may be used as authority. Their
/// signatures are finite concrete trees; a borrowed input may itself be a
/// root-provided reference, but no result or value child may contain one.
///
/// Return the known-effect flag for one ordered effect list without returning
/// from inside a loop.
pub(super) fn effects_known(env: &super::Env, effects: &[crate::types::EffId]) -> bool {
    let mut index = 0;
    let mut known = true;
    while index < effects.len() && known {
        if !env.knows_effect(effects[index]) {
            known = false;
        }
        index += 1;
    }
    known
}

/// Validate one ordered type list at the fixed 512 nesting budget, accumulating
/// the verdict instead of returning from inside the loop.
pub(super) fn types_valid(env: &super::Env, types: &[crate::types::Ty]) -> bool {
    let mut index = 0;
    let mut valid = true;
    while index < types.len() && valid {
        valid = env.valid_type(&types[index], 512);
        index += 1;
    }
    valid
}

/// Validate one reachable `LiveRef` node and queue its forwarded references.
/// Loop-free apart from accumulating scans, so Aeneas can extract it.
pub(super) fn ref_node_valid<'a>(
    env: &super::Env,
    current: &'a crate::types::Ty,
    pending: &mut alloc::vec::Vec<&'a crate::types::Ty>,
) -> bool {
    let crate::types::Ty::LiveRef(input, output, effects) = current else {
        return false;
    };
    if input.len().saturating_add(output.len()) > 512 {
        return false;
    }
    if !effects_known(env, effects.as_slice()) {
        return false;
    }
    if !types_valid(env, output) {
        return false;
    }
    let mut index = 0;
    let mut queued = true;
    while index < input.len() && queued {
        let value = &input[index];
        if matches!(value, crate::types::Ty::LiveRef(_, _, _)) {
            if pending.len() >= 512 {
                queued = false;
            } else {
                pending.push(value);
            }
        } else if !env.valid_type(value, 512) {
            queued = false;
        }
        index += 1;
    }
    queued
}

pub(super) fn valid_ref(env: &super::Env, ty: &crate::types::Ty) -> bool {
    if !env.live_slots {
        return false;
    }
    let mut pending: alloc::vec::Vec<&crate::types::Ty> = alloc::vec::Vec::new();
    let mut next = Some(ty);
    let mut visited = 0usize;
    let mut valid = true;
    while let Some(current) = next {
        if visited >= 512 || !ref_node_valid(env, current, &mut pending) {
            valid = false;
            next = None;
        } else {
            visited += 1;
            next = pending.pop();
        }
    }
    valid
}
