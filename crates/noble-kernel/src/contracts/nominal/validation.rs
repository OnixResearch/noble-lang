//! Independently checked module definitions, bootstrap rows, and bound adapters.

impl super::super::Env {
    pub(crate) fn validate_contracts(&self) -> bool {
        if self.kinds.len() != self.defs.len() || self.bound_adapters.len() > self.defs.len() {
            return false;
        }
        if (!self.nominals.is_empty() || !self.bound_adapters.is_empty()) && !self.declared_modules
        {
            return false;
        }
        bootstrap_matches(self)
            && (!self.declared_modules || fixed_definitions_match(self))
            && counts_match(self)
            && definitions_match(self)
            && bound_adapters_unique(self)
    }

    fn matching_bound_rows(&self, def: super::super::Definition, slot: u32) -> u8 {
        let mut matches = 0u8;
        let mut row_index = 0;
        while row_index < self.bound_adapters.len() {
            if bound_row_matches(self, row_index, def, slot) {
                matches = matches.saturating_add(1);
            }
            row_index += 1;
        }
        matches
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; both pinned Rust compilers reject indexing bound_adapters Vec (E0277/E0658) and its runtime BoundAdapter equality checks; reassess when those reads and comparisons become const-capable."
)]
fn bound_row_matches(
    env: &super::super::Env,
    index: usize,
    def: super::super::Definition,
    slot: u32,
) -> bool {
    let row = &env.bound_adapters[index];
    if row.definition != def {
        return false;
    }
    if row.adapter_slot != slot {
        return false;
    }
    bound_row_contract(row)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; both pinned Rust compilers reject input Vec indexing (E0277/E0658) and runtime EffSet::contains used to validate this adapter; reassess when these reads become const-capable."
)]
fn bound_row_contract(row: &super::super::BoundAdapter) -> bool {
    if row.adapter_identity.is_empty() {
        return false;
    }
    if row.input.len() != 1 {
        return false;
    }
    if row.input[0] != crate::types::Ty::Text {
        return false;
    }
    if !row.output.is_empty() {
        return false;
    }
    if row.effects.as_slice().len() != 1 {
        return false;
    }
    row.effects.contains(super::super::TEST_EMIT)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; both pinned Rust compilers reject indexing the resource_kinds and effects Vecs (E0277/E0658) to validate fixture identities; reassess when indexed Vec reads become const-capable."
)]
fn bootstrap_matches(env: &super::super::Env) -> bool {
    if !env.declared_modules {
        return true;
    }
    env.resource_kinds.len() == 1
        && env.resource_kinds[0].0 == super::super::FIXTURE_RESOURCE.0
        && env.effects.len() == 1
        && env.effects[0].0 == super::super::TEST_EMIT.0
        && env.definition_owners.len() == env.defs.len()
        && env.deps.len() == env.defs.len()
}

fn fixed_definitions_match(env: &super::super::Env) -> bool {
    let fixed = super::super::bootstrap::data::table();
    if env.defs.len() < fixed.len() {
        return false;
    }
    let mut index = 0;
    let mut is_matching = true;
    while index < fixed.len() && is_matching {
        let (behavior, scheme) = &fixed[index];
        is_matching =
            env.kinds[index] == *behavior && super::schemes::same_scheme(&env.defs[index], scheme);
        index += 1;
    }
    is_matching
}

fn counts_match(env: &super::super::Env) -> bool {
    let mut index = 0;
    let mut is_matching = true;
    while index < env.nominals.len() && is_matching {
        is_matching = declaration_counts_match(env, &env.nominals[index]);
        index += 1;
    }
    is_matching
}

fn declaration_counts_match(env: &super::super::Env, decl: &super::super::NominalDecl) -> bool {
    let mut actual = [0u8; 5];
    let mut index = 0;
    while index < env.kinds.len() {
        if let Some(which) = operation_slot(env.kinds[index], decl.id) {
            actual[which] = actual[which].saturating_add(1);
        }
        index += 1;
    }
    let expected = if matches!(decl.shape, crate::types::NominalShape::Opaque(_)) {
        [1, 1, 0, 0, 0]
    } else if matches!(decl.shape, crate::types::NominalShape::Variant(_, _)) {
        [0, 0, 1, 1, 1]
    } else {
        return false;
    };
    actual == expected
}

const fn operation_slot(
    kind: super::super::Behavior,
    id: crate::types::NominalTypeId,
) -> Option<usize> {
    let Some((found, slot)) = operation_identity_slot(kind) else {
        return None;
    };
    if found.module == id.module && found.ordinal == id.ordinal {
        Some(slot)
    } else {
        None
    }
}

const fn operation_identity_slot(
    kind: super::super::Behavior,
) -> Option<(crate::types::NominalTypeId, usize)> {
    if let super::super::Behavior::NominalNew(id) = kind {
        return Some((id, 0));
    }
    if let super::super::Behavior::NominalInto(id) = kind {
        return Some((id, 1));
    }
    if let super::super::Behavior::NominalLeft(id) = kind {
        return Some((id, 2));
    }
    if let super::super::Behavior::NominalRight(id) = kind {
        return Some((id, 3));
    }
    if let super::super::Behavior::NominalMatch(id) = kind {
        return Some((id, 4));
    }
    None
}

fn definitions_match(env: &super::super::Env) -> bool {
    let mut index = 0;
    let mut is_matching = true;
    while index < env.kinds.len() && is_matching {
        let kind = env.kinds[index];
        let is_matched = if let super::super::Behavior::BoundEmit(slot) = kind {
            bound_definition_matches(env, index, slot)
        } else if let Some((id, _)) = operation_identity_slot(kind) {
            declared_definition_matches(env, index, kind, id)
        } else {
            true
        };
        is_matching = is_matched;
        index += 1;
    }
    is_matching
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; both pinned Rust compilers reject owner Vec::get/index (E0277/E0658), Env::nominal lookup, and allocated expected_scheme construction (E0015); reassess when those checks become const-capable."
)]
fn declared_definition_matches(
    env: &super::super::Env,
    index: usize,
    kind: super::super::Behavior,
    id: crate::types::NominalTypeId,
) -> bool {
    if env.definition_owners.get(index) != Some(&Some(id.module)) {
        return false;
    }
    let Some(decl) = env.nominal(id) else {
        return false;
    };
    let Some(scheme) = super::schemes::expected_scheme(decl, kind) else {
        return false;
    };
    super::schemes::same_scheme(&env.defs[index], &scheme)
}

fn bound_definition_matches(env: &super::super::Env, index: usize, slot: u32) -> bool {
    if !super::schemes::valid_emit_scheme(&env.defs[index]) {
        return false;
    }
    let Ok(def) = u32::try_from(index) else {
        return false;
    };
    matches!(
        env.matching_bound_rows(super::super::Definition(def), slot),
        1
    ) && matches!(env.definition_owners.get(index), Some(Some(_)))
}

fn bound_adapters_unique(env: &super::super::Env) -> bool {
    let mut index = 0;
    let mut is_unique = true;
    while index < env.bound_adapters.len() && is_unique {
        let definition = env.bound_adapters[index].definition;
        let slot = env.bound_adapters[index].adapter_slot;
        is_unique = env.kind(definition) == Some(super::super::Behavior::BoundEmit(slot));
        let mut prior = 0;
        while prior < index && is_unique {
            let prev = &env.bound_adapters[prior];
            is_unique = prev.adapter_slot != slot && prev.definition != definition;
            prior += 1;
        }
        index += 1;
    }
    is_unique
}

pub(super) fn valid_program_components(
    env: &super::super::Env,
    stack_in: &[crate::types::Ty],
    stack_out: &[crate::types::Ty],
    effects: &crate::types::EffSet,
    max_nodes: u32,
) -> bool {
    let ids = effects.as_slice();
    let mut index = 0;
    let mut is_effect_set_known = true;
    while index < ids.len() && is_effect_set_known {
        is_effect_set_known = env.knows_effect(ids[index]);
        index += 1;
    }
    is_effect_set_known
        && u64::try_from(stack_in.len().saturating_add(stack_out.len()))
            .is_ok_and(|count| count <= u64::from(max_nodes))
}
