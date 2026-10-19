#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; the test-only relation imports kernel type and effect domain types directly so the structural rules stay readable"
)]
use noble_kernel::types::{EffId, EffSet, Ty};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Interface {
    pub(super) inputs: Vec<Ty>,
    pub(super) outputs: Vec<Ty>,
    pub(super) effects: EffSet,
}

pub(super) fn interface(inputs: Vec<Ty>, outputs: Vec<Ty>, effects: EffSet) -> Interface {
    Interface {
        inputs,
        outputs,
        effects,
    }
}

pub(super) fn effect_set(ids: &[EffId]) -> EffSet {
    EffSet::from_ids(ids)
}

#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-maintainers; the test-only relation walks a finite kernel type tree; the recursive form states the structural rule directly and is bounded by the input type depth"
)]
pub(super) fn is_data_subtype(left: &Ty, right: &Ty) -> bool {
    match (left, right) {
        (Ty::Unit, Ty::Unit) | (Ty::Bool, Ty::Bool) | (Ty::I64, Ty::I64) | (Ty::Text, Ty::Text) => {
            true
        }
        (Ty::Pair(left_a, left_b), Ty::Pair(right_a, right_b))
        | (Ty::Sum(left_a, left_b), Ty::Sum(right_a, right_b)) => {
            is_data_subtype(left_a, right_a) && is_data_subtype(left_b, right_b)
        }
        (Ty::List(left_item), Ty::List(right_item)) => is_data_subtype(left_item, right_item),
        (Ty::Nominal(left_id, left_shape), Ty::Nominal(right_id, right_shape)) => {
            left_id == right_id && left_shape == right_shape
        }
        (
            Ty::GenericNominal(left_id, left_args, left_shape),
            Ty::GenericNominal(right_id, right_args, right_shape),
        ) => left_id == right_id && left_args == right_args && left_shape == right_shape,
        _ => false,
    }
}

pub(super) fn is_stack_subtype(sub: &[Ty], sup: &[Ty]) -> bool {
    if sub.len() != sup.len() {
        return false;
    }
    let mut at = 0usize;
    while at < sub.len() {
        if !is_type_subtype(&sub[at], &sup[at]) {
            return false;
        }
        at += 1;
    }
    true
}

pub(super) fn is_type_subtype(left: &Ty, right: &Ty) -> bool {
    match (left, right) {
        (
            Ty::Program(left_in, left_out, left_effects),
            Ty::Program(right_in, right_out, right_effects),
        ) => {
            is_stack_subtype(right_in.as_slice(), left_in.as_slice())
                && is_stack_subtype(left_out.as_slice(), right_out.as_slice())
                && left_effects.is_subset_of(right_effects)
        }
        _ => is_data_subtype(left, right),
    }
}

pub(super) fn is_interface_subtype(base: &Interface, target: &Interface) -> bool {
    is_stack_subtype(&target.inputs, &base.inputs)
        && is_stack_subtype(&base.outputs, &target.outputs)
        && base.effects.is_subset_of(&target.effects)
}

pub(super) fn program(inputs: Vec<Ty>, outputs: Vec<Ty>, effects: EffSet) -> Ty {
    Ty::program(inputs, outputs, effects)
}
