//! Tests for the fragment type, effect-set, and scheme core.
// Names the fragment contract this crate implements; acceptance stays separate.
// r[impl VT-M2-01]

use noble_kernel::scheme::{
    Binding, EffSlot, Inst, InstError, PItem, PSig, PTy, Scheme, SchemeError, VarId, VarKind,
};
use noble_kernel::types::{stack_is_data, EffId, EffSet, ResourceKind, Ty};

fn ids(list: &[u32]) -> EffSet {
    EffSet::from_ids(&list.iter().map(|id| EffId(*id)).collect::<Vec<_>>())
}

#[test]
fn data_is_recursive_over_resource_payloads() {
    let resource = Ty::Resource(ResourceKind(1));
    assert!(Ty::I64.is_data());
    assert!(Ty::List(Box::new(Ty::Pair(Box::new(Ty::Text), Box::new(Ty::Unit)))).is_data());
    assert!(!resource.is_data());
    assert!(!Ty::Sum(Box::new(resource.clone()), Box::new(Ty::I64)).is_data());
    assert!(!Ty::List(Box::new(resource)).is_data());
    let program = Ty::program(vec![Ty::I64], vec![Ty::Text], EffSet::empty());
    assert!(program.is_data());
    assert!(stack_is_data(&[Ty::Bool, program]));
    assert!(!stack_is_data(&[Ty::Bool, Ty::Resource(ResourceKind(2))]));
    // Sum(1) + I64(1) + List(1) + Unit(1)
    assert_eq!(
        Ty::Sum(Box::new(Ty::I64), Box::new(Ty::List(Box::new(Ty::Unit)))).size(),
        4
    );
}

#[test]
fn effect_union_is_sorted_deduplicated_and_ordered() {
    let a = ids(&[5, 1, 3]);
    let b = ids(&[3, 2]);
    assert_eq!(a.union(&b).as_slice(), ids(&[1, 2, 3, 5]).as_slice());
    assert_eq!(a.union(&b), b.union(&a));
    assert!(ids(&[1, 3]).is_subset_of(&a));
    assert!(!ids(&[1, 4]).is_subset_of(&a));
    assert!(EffSet::empty().is_subset_of(&a));
    assert!(!a.is_subset_of(&EffSet::empty()));
}

#[test]
fn dup_scheme_instantiates_and_rejects_kind_mismatch() {
    let dup = Scheme {
        var_kinds: vec![VarKind::Stack, VarKind::Value],
        stack_in: vec![PItem::Stack(VarId(0)), PItem::Ty(PTy::Var(VarId(1)))],
        stack_out: vec![
            PItem::Stack(VarId(0)),
            PItem::Ty(PTy::Var(VarId(1))),
            PItem::Ty(PTy::Var(VarId(1))),
        ],
        effects: vec![],
    };
    dup.validate().expect("dup scheme is well formed");
    let inst = Inst {
        bindings: vec![Binding::Stack(vec![Ty::Bool]), Binding::Value(Ty::I64)],
    };
    dup.check_inst(&inst, 64, 256, 16).expect("fits limits");
    assert_eq!(
        dup.subst_stack(&dup.stack_in, &inst).unwrap(),
        vec![Ty::Bool, Ty::I64]
    );
    assert_eq!(
        dup.subst_stack(&dup.stack_out, &inst).unwrap(),
        vec![Ty::Bool, Ty::I64, Ty::I64]
    );
    let wrong_kind = Inst {
        bindings: vec![Binding::Value(Ty::Bool), Binding::Value(Ty::I64)],
    };
    assert_eq!(
        dup.check_inst(&wrong_kind, 64, 256, 16),
        Err(InstError::KindMismatch)
    );
    let arity = Inst {
        bindings: vec![Binding::Value(Ty::I64)],
    };
    assert_eq!(
        dup.check_inst(&arity, 64, 256, 16),
        Err(InstError::ArityMismatch)
    );
}

#[test]
fn run_scheme_carries_program_pattern_and_effect_variable() {
    let run = Scheme {
        var_kinds: vec![VarKind::Stack, VarKind::Stack, VarKind::Effect],
        stack_in: vec![
            PItem::Stack(VarId(0)),
            PItem::Ty(PTy::Program(Box::new(PSig {
                stack_in: vec![PItem::Stack(VarId(0))],
                stack_out: vec![PItem::Stack(VarId(1))],
                effects: vec![EffSlot::Var(VarId(2))],
            }))),
        ],
        stack_out: vec![PItem::Stack(VarId(1))],
        effects: vec![EffSlot::Var(VarId(2))],
    };
    run.validate().expect("run scheme is well formed");
    let inst = Inst {
        bindings: vec![
            Binding::Stack(vec![Ty::I64]),
            Binding::Stack(vec![Ty::I64]),
            Binding::Effect(ids(&[7])),
        ],
    };
    assert_eq!(
        run.subst_stack(&run.stack_in, &inst).unwrap(),
        vec![
            Ty::I64,
            Ty::program(vec![Ty::I64], vec![Ty::I64], ids(&[7]))
        ]
    );
    assert_eq!(run.subst_effects(&run.effects, &inst).unwrap(), ids(&[7]));
}

#[test]
fn scheme_validation_rejects_kind_misuse_and_missing_variables() {
    let wrong_use = Scheme {
        var_kinds: vec![VarKind::Value],
        stack_in: vec![PItem::Stack(VarId(0))],
        stack_out: vec![],
        effects: vec![],
    };
    assert_eq!(wrong_use.validate(), Err(SchemeError::KindMismatch));
    let unknown = Scheme {
        var_kinds: vec![VarKind::Stack],
        stack_in: vec![PItem::Ty(PTy::Var(VarId(9)))],
        stack_out: vec![],
        effects: vec![],
    };
    assert_eq!(unknown.validate(), Err(SchemeError::UnknownVariable));
}

#[test]
fn oversized_instantiations_reject_before_use() {
    let swap = Scheme {
        var_kinds: vec![VarKind::Stack, VarKind::Value, VarKind::Value],
        stack_in: vec![
            PItem::Stack(VarId(0)),
            PItem::Ty(PTy::Var(VarId(1))),
            PItem::Ty(PTy::Var(VarId(2))),
        ],
        stack_out: vec![
            PItem::Stack(VarId(0)),
            PItem::Ty(PTy::Var(VarId(2))),
            PItem::Ty(PTy::Var(VarId(1))),
        ],
        effects: vec![],
    };
    swap.validate().expect("swap scheme is well formed");
    let big_stack = Inst {
        bindings: vec![
            Binding::Stack(vec![Ty::I64; 4]),
            Binding::Value(Ty::Bool),
            Binding::Value(Ty::Text),
        ],
    };
    assert_eq!(
        swap.check_inst(&big_stack, 3, 256, 16),
        Err(InstError::OversizedStack)
    );
    assert!(swap.check_inst(&big_stack, 4, 256, 16).is_ok());
}
