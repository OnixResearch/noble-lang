//! Host-registered nominal resources and borrowed live-slot signatures.

use super::{Env, NominalDecl, NominalError};
use crate::types::{NominalShape, ResourceKind, Ty};

fn resource_kind(decl: &NominalDecl) -> Option<ResourceKind> {
    let NominalShape::Opaque(representation) = &decl.shape else {
        return None;
    };
    let Ty::Resource(kind) = &**representation else {
        return None;
    };
    Some(*kind)
}

pub(super) fn register_resource(mut env: Env, decl: NominalDecl) -> Result<Env, NominalError> {
    let kind = resource_kind(&decl).ok_or(NominalError::InvalidRepresentation)?;
    if !env.live_slots
        || !decl.exported
        || decl.public != [false, false]
        || kind == super::FIXTURE_RESOURCE
        || env.resource_kinds.contains(&kind)
    {
        return Err(NominalError::InvalidRepresentation);
    }
    if env.nominal(decl.id).is_some() || env.generic_variant(decl.id).is_some() {
        return Err(NominalError::DuplicateIdentity);
    }
    env.resource_kinds.push(kind);
    env.live_resource_nominals.push(decl.id);
    env.nominals.push(decl);
    Ok(env)
}

/// Check the catalog itself, not just its registration path: `Env` fields
/// are public and may have been altered after host registration.
pub(super) fn catalog_valid(env: &Env) -> bool {
    if !env.live_slots {
        return env.live_resource_nominals.is_empty();
    }
    if env.resource_kinds.len() != env.live_resource_nominals.len().saturating_add(1)
        || env.resource_kinds.first() != Some(&super::FIXTURE_RESOURCE)
    {
        return false;
    }
    let mut index = 0;
    while index < env.live_resource_nominals.len() {
        let Some(decl) = env.nominal(env.live_resource_nominals[index]) else {
            return false;
        };
        if !decl.exported
            || decl.public != [false, false]
            || resource_kind(decl) != Some(env.resource_kinds[index + 1])
        {
            return false;
        }
        let mut prior = 0;
        while prior < index {
            if env.resource_kinds[prior + 1] == env.resource_kinds[index + 1]
                || env.live_resource_nominals[prior] == env.live_resource_nominals[index]
            {
                return false;
            }
            prior += 1;
        }
        index += 1;
    }
    true
}

pub(super) fn is_live_kind(env: &Env, kind: ResourceKind) -> bool {
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

/// Only root-positioned borrowed references may be used as authority. Their
/// signatures are finite concrete trees; a borrowed input may itself be a
/// root-provided reference, but no result or value child may contain one.
pub(super) fn valid_ref(env: &Env, ty: &Ty) -> bool {
    if !env.live_slots {
        return false;
    }
    let mut pending: alloc::vec::Vec<&Ty> = alloc::vec::Vec::new();
    let mut next = Some(ty);
    let mut visited = 0usize;
    while let Some(current) = next {
        if visited >= 512 {
            return false;
        }
        visited += 1;
        let Ty::LiveRef(input, output, effects) = current else {
            return false;
        };
        if input.len().saturating_add(output.len()) > 512 {
            return false;
        }
        let mut effect_index = 0;
        while effect_index < effects.as_slice().len() {
            if !env.knows_effect(effects.as_slice()[effect_index]) {
                return false;
            }
            effect_index += 1;
        }
        let mut output_index = 0;
        while output_index < output.len() {
            if !env.valid_type(&output[output_index], 512) {
                return false;
            }
            output_index += 1;
        }
        let mut input_index = 0;
        while input_index < input.len() {
            let value = &input[input_index];
            if matches!(value, Ty::LiveRef(_, _, _)) {
                if pending.len() >= 512 {
                    return false;
                }
                pending.push(value);
            } else if !env.valid_type(value, 512) {
                return false;
            }
            input_index += 1;
        }
        next = pending.pop();
    }
    true
}
