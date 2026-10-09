use alloc::vec;
use alloc::vec::Vec;
use super::{CheckedScheme, Dependency, SchemeError, SymbolicCandidate, SymbolicNode};
use super::project::charge;
use crate::contracts::{Behavior, Definition, Env, GenericVariantDecl, NominalDecl};
use crate::types::{EffId, NominalShape, NominalTypeId};
use crate::words::Scheme;

/// Only the owner and definitions actually invoked by this checked body are
/// retained. Whole growing environments are never copied per definition.
#[derive(Clone, Debug)]
pub(super) struct EnvSnapshot {
    rows: Vec<DefinitionRow>,
    nominals: Vec<NominalDecl>,
    generic_variants: Vec<GenericVariantDecl>,
    effects: Vec<EffId>,
    live_slots: bool,
    live_test_hosts: bool,
    declared_modules: bool,
    text_cursor: bool,
}

#[derive(Clone, Debug)]
struct DefinitionRow {
    def: Definition,
    scheme: Scheme,
    kind: Behavior,
    module: Option<u64>,
    deps: Vec<Definition>,
}

pub(super) fn capture(
    env: &Env, owner: Definition, candidate: &SymbolicCandidate, work: &mut u32,
) -> Result<EnvSnapshot, SchemeError> {
    let mut identities = vec![owner];
    for node in &candidate.nodes {
        charge(work,1)?;
        if let SymbolicNode::Invocation {def,..} | SymbolicNode::SignedSelf {def} = node {
            if !identities.contains(def) { identities.push(*def); }
        }
    }
    let mut rows = Vec::with_capacity(identities.len());
    let mut nominals = Vec::<NominalDecl>::new();
    let mut generic_variants = Vec::<GenericVariantDecl>::new();
    for def in identities {
        let index = usize::try_from(def.0).map_err(|_| SchemeError::StaleEnvironment)?;
        let scheme = env.scheme(def).ok_or(SchemeError::StaleEnvironment)?;
        let kind = env.kind(def).ok_or(SchemeError::StaleEnvironment)?;
        let module = *env.definition_owners.get(index).ok_or(SchemeError::StaleEnvironment)?;
        let deps = env.deps.get(index).ok_or(SchemeError::StaleEnvironment)?;
        charge(work,scheme_cost(scheme).saturating_add(deps.len()))?;
        if let Some((id,false)) = access_descriptor(kind) {
            if !nominals.iter().any(|item| item.id == id) {
                charge(work,env.nominals.len().saturating_add(1))?;
                let desc = env.nominal(id).ok_or(SchemeError::StaleEnvironment)?;
                charge(work,nominal_size(desc)?)?;
                nominals.push(desc.clone());
            }
        }
        if let Some((id,true)) = access_descriptor(kind) {
            if !generic_variants.iter().any(|item| item.id == id) {
                charge(work,env.generic_variants.len().saturating_add(1))?;
                generic_variants.push(env.generic_variant(id)
                    .ok_or(SchemeError::StaleEnvironment)?.clone());
            }
        }
        rows.push(DefinitionRow {def,scheme:scheme.clone(),kind,module,deps:deps.clone()});
    }
    charge(work,env.effects.len())?;
    Ok(EnvSnapshot {
        rows,nominals,generic_variants,effects:env.effects.clone(),live_slots:env.live_slots,
        live_test_hosts:env.live_test_hosts,declared_modules:env.declared_modules,
        text_cursor:env.text_cursor,
    })
}

fn access_descriptor(kind: Behavior) -> Option<(NominalTypeId,bool)> {
    match kind {
        Behavior::NominalNew(id) | Behavior::NominalInto(id)
        | Behavior::NominalLeft(id) | Behavior::NominalRight(id)
        | Behavior::NominalMatch(id) => Some((id,false)),
        Behavior::GenericLeft(id) | Behavior::GenericRight(id)
        | Behavior::GenericMatch(id) => Some((id,true)),
        _ => None,
    }
}

fn scheme_cost(scheme: &Scheme) -> usize {
    let mut nodes = scheme.var_kinds.len()
        .saturating_add(scheme.effects.len()).saturating_add(1);
    for pattern in scheme.stack_in.iter().chain(&scheme.stack_out) {
        nodes = nodes.saturating_add(super::project::pattern_size(pattern));
    }
    nodes
}

fn nominal_size(decl: &NominalDecl) -> Result<usize,SchemeError> {
    let size = match &decl.shape {
        NominalShape::Opaque(ty) => ty.size(),
        NominalShape::Variant(left,right) => left.size()
            .and_then(|count| count.checked_add(right.size()?)),
    }.ok_or(SchemeError::TypeSizeExhausted)?;
    Ok(size as usize)
}

/// Exact retained definition rows and effect identities are checked even if
/// unrelated later module registrations append definitions or nominals.
fn current_snapshot(seal: &CheckedScheme, env: &Env, work: &mut u32) -> Result<(), SchemeError> {
    let old = &seal.environment;
    charge(work,old.rows.len().saturating_add(old.effects.len())
        .saturating_add(old.nominals.len()).saturating_add(old.generic_variants.len()))?;
    if env.effects.get(..old.effects.len()) != Some(old.effects.as_slice())
        || env.live_slots != old.live_slots
        || env.live_test_hosts != old.live_test_hosts
        || env.declared_modules != old.declared_modules || env.text_cursor != old.text_cursor
    { return Err(SchemeError::StaleEnvironment); }
    for row in &old.rows {
        charge(work,scheme_cost(&row.scheme).saturating_add(row.deps.len()))?;
        let index = usize::try_from(row.def.0).map_err(|_| SchemeError::StaleEnvironment)?;
        if env.scheme(row.def) != Some(&row.scheme)
            || env.kind(row.def) != Some(row.kind)
            || env.definition_owners.get(index) != Some(&row.module)
            || env.deps.get(index) != Some(&row.deps)
        { return Err(SchemeError::StaleEnvironment); }
    }
    for nominal in &old.nominals {
        charge(work,env.nominals.len().saturating_add(nominal_size(nominal)?))?;
        if env.nominal(nominal.id) != Some(nominal) {
            return Err(SchemeError::StaleEnvironment);
        }
    }
    for generic in &old.generic_variants {
        charge(work,env.generic_variants.len())?;
        if env.generic_variant(generic.id) != Some(generic) {
            return Err(SchemeError::StaleEnvironment);
        }
    }
    Ok(())
}

fn find_unique<'a>(def: Definition, deps: &'a [Dependency<'a>])
    -> Result<&'a Dependency<'a>, SchemeError>
{
    let mut found = None;
    for dep in deps {
        if dep.seal.owner == def {
            if found.is_some() { return Err(SchemeError::InvalidDependency(def)); }
            found = Some(dep);
        }
    }
    found.ok_or(SchemeError::InvalidDependency(def))
}

/// Check a direct Named dependency and its entire previously sealed closure;
/// a caller-supplied source tag must be validated by the source ingress.
pub(super) fn dependency<'a>(
    def: Definition, env: &Env, deps: &'a [Dependency<'a>], work: &mut u32,
) -> Result<&'a CheckedScheme, SchemeError> {
    let root = find_unique(def,deps)?;
    let mut pending = vec![root.seal];
    let mut visited = Vec::<Definition>::new();
    while let Some(seal) = pending.pop() {
        charge(work,1)?;
        if visited.contains(&seal.owner) { continue; }
        visited.push(seal.owner);
        let supplied = find_unique(seal.owner,deps)?;
        if supplied.current_stamp != seal.stamp { return Err(SchemeError::StaleSource); }
        current_snapshot(seal,env,work)?;
        for (required,stamp) in &seal.dependencies {
            charge(work,1)?;
            let next = find_unique(*required,deps)?;
            if next.current_stamp != *stamp { return Err(SchemeError::StaleSource); }
            pending.push(next.seal);
        }
    }
    Ok(root.seal)
}

pub(super) fn instantiate_context<'a>(
    seal: &CheckedScheme, env: &Env, deps: &'a [Dependency<'a>], work: &mut u32,
) -> Result<(), SchemeError> {
    current_snapshot(seal,env,work)?;
    let owner = usize::try_from(seal.owner.0).map_err(|_| SchemeError::InvalidOwner)?;
    if env.caller_module.is_none()
        || env.definition_owners.get(owner) != Some(&env.caller_module)
    { return Err(SchemeError::InvalidOwner); }
    for (def,stamp) in &seal.dependencies {
        let dep = dependency(*def,env,deps,work)?;
        if dep.stamp != *stamp { return Err(SchemeError::StaleSource); }
    }
    Ok(())
}
