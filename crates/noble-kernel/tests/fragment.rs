//! Fragment type, effect-set, and scheme primitive controls.
// Names the fragment contract this crate implements.
// r[impl VT-M2-01]

#[test]
fn data_is_recursive_over_resource_payloads() {
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    let nested = noble_kernel::types::Ty::List(Box::new(noble_kernel::types::Ty::Pair(
        Box::new(noble_kernel::types::Ty::Text),
        Box::new(noble_kernel::types::Ty::Unit),
    )));
    assert!(nested.is_data());
    assert!(!resource.is_data());
    assert!(!noble_kernel::types::Ty::Sum(
        Box::new(resource.clone()),
        Box::new(noble_kernel::types::Ty::I64)
    )
    .is_data());
    assert!(!noble_kernel::types::Ty::List(Box::new(resource)).is_data());
    let program = noble_kernel::types::Ty::program(
        vec![noble_kernel::types::Ty::I64],
        vec![noble_kernel::types::Ty::Text],
        noble_kernel::types::EffSet::empty(),
    );
    assert!(program.is_data());
    // Sum(1) + I64(1) + List(1) + Unit(1)
    assert_eq!(
        noble_kernel::types::Ty::Sum(
            Box::new(noble_kernel::types::Ty::I64),
            Box::new(noble_kernel::types::Ty::List(Box::new(
                noble_kernel::types::Ty::Unit
            )))
        )
        .size(),
        Some(4)
    );
}

#[test]
fn effect_sets_union_sorted_deduplicated_and_ordered() {
    let left = ids(&[5, 1, 3]);
    let right = ids(&[3, 2]);
    assert_eq!(left.union(&right).as_slice(), ids(&[1, 2, 3, 5]).as_slice());
    assert_eq!(left.union(&right), right.union(&left));
    assert!(ids(&[1, 3]).is_subset_of(&left));
    assert!(!ids(&[1, 4]).is_subset_of(&left));
    assert!(noble_kernel::types::EffSet::empty().is_subset_of(&left));
    assert!(!left.is_subset_of(&noble_kernel::types::EffSet::empty()));
}

fn ids(list: &[u32]) -> noble_kernel::types::EffSet {
    let effects: Vec<noble_kernel::types::EffId> = list
        .iter()
        .map(|id| noble_kernel::types::EffId(*id))
        .collect();
    noble_kernel::types::EffSet::from_ids(&effects)
}

#[test]
fn schemes_instantiate_and_reject_mismatches() -> Result<(), String> {
    let mut env = match noble_kernel::contracts::environment() {
        Ok(env) => env,
        Err(defect) => return Err(format!("environment defect: {defect:?}")),
    };
    // The first definition is `dup : S a -- S a a`.
    let scheme = match env.defs.first().cloned() {
        Some(scheme) => scheme,
        None => return Err("missing dup contract".to_string()),
    };
    assert!(scheme.validate().is_ok());
    let good = noble_kernel::words::Inst {
        bindings: vec![
            noble_kernel::words::Binding::Stack(vec![noble_kernel::types::Ty::Bool]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
        ],
    };
    assert!(scheme.check_inst(&good, 64, 256, 16).is_ok());
    let out = match scheme.subst_stack(&scheme.stack_out, &good) {
        Ok(out) => out,
        Err(error) => return Err(format!("substitution failed: {error:?}")),
    };
    assert_eq!(
        out,
        vec![
            noble_kernel::types::Ty::Bool,
            noble_kernel::types::Ty::I64,
            noble_kernel::types::Ty::I64
        ]
    );
    let wrong_kind = noble_kernel::words::Inst {
        bindings: vec![
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
        ],
    };
    assert!(matches!(
        scheme.check_inst(&wrong_kind, 64, 256, 16),
        Err(noble_kernel::words::InstError::KindMismatch)
    ));
    let arity = noble_kernel::words::Inst {
        bindings: vec![noble_kernel::words::Binding::Value(
            noble_kernel::types::Ty::I64,
        )],
    };
    assert!(matches!(
        scheme.check_inst(&arity, 64, 256, 16),
        Err(noble_kernel::words::InstError::ArityMismatch)
    ));
    let big = noble_kernel::words::Inst {
        bindings: vec![
            noble_kernel::words::Binding::Stack(vec![noble_kernel::types::Ty::I64; 4]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
        ],
    };
    assert!(matches!(
        scheme.check_inst(&big, 3, 256, 16),
        Err(noble_kernel::words::InstError::OversizedStack)
    ));
    env.defs.clear();
    Ok(())
}

// Regression control for the substitution walk's constructor completion,
// found by the bounded property harness (DX-PROPERTY-01) at generated cases
// 2 and 11: the work-stack walk queued pair and sum children so the finish
// step popped them swapped (contradicting `inr : S b -- S Sum<a,b>` and the
// reference model), and the completion step popped a phantom second segment
// for list patterns, so every list-bearing witness rejected.
// r[verify VT-M2-01]
#[test]
fn constructor_patterns_substitute_in_documented_order() -> Result<(), String> {
    let env = match noble_kernel::contracts::environment() {
        Ok(env) => env,
        Err(defect) => return Err(format!("environment defect: {defect:?}")),
    };
    // `pair : S a b -- S Pair<a,b>` is definition 12; `inr : S b -- S Sum<a,b>`
    // is definition 15; `nil : S -- S List<a>` is definition 18. Distinct
    // types on each side make any swap observable.
    for (def, bindings, expected) in [
        (
            noble_kernel::contracts::Definition(12),
            vec![
                noble_kernel::words::Binding::Stack(vec![]),
                noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
                noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Text),
            ],
            noble_kernel::types::Ty::Pair(
                Box::new(noble_kernel::types::Ty::Bool),
                Box::new(noble_kernel::types::Ty::Text),
            ),
        ),
        (
            noble_kernel::contracts::Definition(15),
            vec![
                noble_kernel::words::Binding::Stack(vec![]),
                noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Unit),
                noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
            ],
            noble_kernel::types::Ty::Sum(
                Box::new(noble_kernel::types::Ty::Unit),
                Box::new(noble_kernel::types::Ty::Bool),
            ),
        ),
        (
            noble_kernel::contracts::Definition(18),
            vec![
                noble_kernel::words::Binding::Stack(vec![]),
                noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
            ],
            noble_kernel::types::Ty::List(Box::new(noble_kernel::types::Ty::Bool)),
        ),
    ] {
        let scheme = match env.scheme(def) {
            Some(scheme) => scheme.clone(),
            None => return Err("missing contract".to_string()),
        };
        let inst = noble_kernel::words::Inst { bindings };
        let out = match scheme.subst_stack(&scheme.stack_out, &inst) {
            Ok(out) => out,
            Err(error) => return Err(format!("substitution failed: {error:?}")),
        };
        assert_eq!(out, vec![expected]);
    }
    Ok(())
}
