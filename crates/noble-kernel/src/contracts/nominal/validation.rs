//! Independently checked module definitions, bootstrap rows, and bound adapters.

impl super::super::Env {
    pub(crate) fn valid_generic_declarations(&self) -> bool {
        let mut index = 0;
        let mut valid = true;
        while index < self.generic_variants.len() && valid {
            let decl = &self.generic_variants[index];
            valid = matches!(decl.payload_params, [0, 1] | [1, 0])
                && self.nominal(decl.id).is_none();
            let mut prior = 0;
            while prior < index && valid {
                valid = self.generic_variants[prior].id != decl.id;
                prior += 1;
            }
            index += 1;
        }
        valid
    }

    pub(crate) fn validate_contracts(&self) -> bool {
        if self.kinds.len() != self.defs.len() || self.bound_adapters.len() > self.defs.len() {
            return false;
        }
        if (!self.nominals.is_empty()
            || !self.generic_variants.is_empty()
            || !self.bound_adapters.is_empty())
            && !self.declared_modules
        {
            return false;
        }
        bootstrap_matches(self)
            && super::super::live::catalog_valid(self)
            && (!self.declared_modules || fixed_definitions_match(self))
            && counts_match(self)
            && generic_counts_match(self)
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

fn generic_counts_match(env: &super::super::Env) -> bool {
    if !env.valid_generic_declarations() {
        return false;
    }
    let mut index = 0;
    while index < env.generic_variants.len() {
        let decl = &env.generic_variants[index];
        let mut count = [0u8; 3];
        let mut kind_index = 0;
        while kind_index < env.kinds.len() {
            match env.kinds[kind_index] {
                super::super::Behavior::GenericLeft(id) if id == decl.id => {
                    count[0] = count[0].saturating_add(1);
                }
                super::super::Behavior::GenericRight(id) if id == decl.id => {
                    count[1] = count[1].saturating_add(1);
                }
                super::super::Behavior::GenericMatch(id) if id == decl.id => {
                    count[2] = count[2].saturating_add(1);
                }
                _ => {}
            }
            kind_index += 1;
        }
        if count != [1, 1, 1] {
            return false;
        }
        index += 1;
    }
    true
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
    (row.input.as_slice() == [crate::types::Ty::Text]
        && row.output.is_empty()
        && row.effects.as_slice() == [super::super::TEST_EMIT])
        || (row.input.is_empty()
            && row.output.as_slice() == [crate::types::Ty::I64]
            && row.effects.as_slice() == [super::super::TEST_CLOCK])
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; both pinned Rust compilers reject indexing the resource_kinds and effects Vecs (E0277/E0658) to validate fixture identities; reassess when indexed Vec reads become const-capable."
)]
fn bootstrap_matches(env: &super::super::Env) -> bool {
    if !env.declared_modules {
        return true;
    }
    let has_clock = env.kinds.iter().any(|kind| matches!(kind, super::super::Behavior::BoundClock(_)));
    env.resource_kinds.len() == 1 + env.live_resource_nominals.len()
        && env.resource_kinds[0].0 == super::super::FIXTURE_RESOURCE.0
        && env.effects.len() == 1 + usize::from(has_clock) + usize::from(env.live_slots)
        && env.effects[0].0 == super::super::TEST_EMIT.0
        && env.effects.contains(&super::super::TEST_CLOCK) == has_clock
        && env.effects.contains(&super::super::LIVE_DISPATCH) == env.live_slots
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
    let expected = if env.live_resource_nominals.contains(&decl.id) {
        [0, 0, 0, 0, 0]
    } else if matches!(decl.shape, crate::types::NominalShape::Opaque(_)) {
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
            bound_definition_matches(env, index, slot, false)
        } else if let super::super::Behavior::BoundClock(slot) = kind {
            bound_definition_matches(env, index, slot, true)
        } else if let Some(id) = generic_operation_id(kind) {
            generic_definition_matches(env, index, kind, id)
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

const fn generic_operation_id(kind: super::super::Behavior) -> Option<crate::types::NominalTypeId> {
    match kind {
        super::super::Behavior::GenericLeft(id)
        | super::super::Behavior::GenericRight(id)
        | super::super::Behavior::GenericMatch(id) => Some(id),
        _ => None,
    }
}

fn generic_definition_matches(
    env: &super::super::Env,
    index: usize,
    kind: super::super::Behavior,
    id: crate::types::NominalTypeId,
) -> bool {
    if env.definition_owners.get(index) != Some(&Some(id.module)) {
        return false;
    }
    let Some(decl) = env.generic_variant(id) else {
        return false;
    };
    let Some(expected) = super::schemes::expected_generic_scheme(decl, kind) else {
        return false;
    };
    super::schemes::same_scheme(&env.defs[index], &expected)
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
    let Some(scheme) = super::schemes::expected_scheme(env, decl, kind) else {
        return false;
    };
    super::schemes::same_scheme(&env.defs[index], &scheme)
}

fn bound_definition_matches(
    env: &super::super::Env,
    index: usize,
    slot: u32,
    clock: bool,
) -> bool {
    if !(if clock {
        super::schemes::valid_clock_scheme(&env.defs[index])
    } else {
        super::schemes::valid_emit_scheme(&env.defs[index])
    }) {
        return false;
    }
    let Ok(def) = u32::try_from(index) else {
        return false;
    };
    let Some(row) = env.bound_adapters.iter().find(|row| {
        row.definition == super::super::Definition(def) && row.adapter_slot == slot
    }) else {
        return false;
    };
    if clock != (row.effects.as_slice() == [super::super::TEST_CLOCK]) {
        return false;
    }
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
        is_unique = matches!(
            env.kind(definition),
            Some(super::super::Behavior::BoundEmit(actual)
                | super::super::Behavior::BoundClock(actual)) if actual == slot
        );
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
