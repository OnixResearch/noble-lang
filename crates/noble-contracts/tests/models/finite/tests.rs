#![allow(
    tigerstyle::assertion_density,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-contracts-maintainers; this test-only model harness constructs each finite relation and asserts its outcome, so long setup legs, exhaustive matches and fresh per-iteration fixtures are the readable form."
)]
#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; the test-only prototype imports kernel type and effect domain types directly so the examples and assertions stay readable"
)]
use noble_kernel::contracts::{TEST_CLOCK, TEST_EMIT};
use noble_kernel::types::{NominalShape, NominalTypeId, Ty};

use super::candidates::{
    backward_accepts, differential, explicit_accepts, family, is_whole_candidate, Candidate,
};
use super::relation::{effect_set, interface, is_data_subtype, program};
fn real_examples() -> (usize, usize) {
    let mut accepted = 0usize;
    let mut refused = 0usize;
    let pure = effect_set(&[]);
    let emit = effect_set(&[TEST_EMIT]);
    let emit_clock = effect_set(&[TEST_EMIT, TEST_CLOCK]);
    let base = interface(
        Vec::new(),
        vec![program(vec![Ty::I64], vec![Ty::Bool], pure.clone())],
        pure.clone(),
    );
    let widened = interface(
        Vec::new(),
        vec![program(vec![Ty::I64], vec![Ty::Bool], emit.clone())],
        emit.clone(),
    );
    if explicit_accepts(&base, &widened, &[TEST_EMIT, TEST_CLOCK]) {
        accepted += 1;
    }
    if !backward_accepts(&widened, &base) {
        refused += 1;
    }
    let composed = interface(
        Vec::new(),
        vec![program(vec![Ty::I64], vec![Ty::Bool], emit_clock.clone())],
        emit_clock,
    );
    if explicit_accepts(&widened, &composed, &[TEST_EMIT, TEST_CLOCK]) {
        accepted += 1;
    }
    if !backward_accepts(&composed, &widened) {
        refused += 1;
    }
    let list = Ty::List(Box::new(Ty::I64));
    let changed = Ty::List(Box::new(Ty::Sum(Box::new(Ty::I64), Box::new(Ty::Unit))));
    if !is_data_subtype(&list, &changed) {
        refused += 1;
    }
    let nominal = Ty::Nominal(
        NominalTypeId {
            module: 1,
            ordinal: 0,
        },
        Box::new(NominalShape::Opaque(Box::new(Ty::I64))),
    );
    let other_nominal = Ty::Nominal(
        NominalTypeId {
            module: 1,
            ordinal: 1,
        },
        Box::new(NominalShape::Opaque(Box::new(Ty::I64))),
    );
    if is_data_subtype(&nominal, &nominal) && !is_data_subtype(&nominal, &other_nominal) {
        accepted += 1;
        refused += 1;
    }
    (accepted, refused)
}
#[test]
fn prototype_records_a_principal_candidate() {
    let (cases, accepted, refused, principal_families, max_family) = differential();
    let (real_accepted, real_refused) = real_examples();
    assert!(cases >= 100);
    assert_eq!(accepted + refused, cases);
    assert_eq!(principal_families, cases);
    assert!(real_accepted >= 3);
    assert!(real_refused >= 3);
    std::println!(
        "NOBLE_FINITE_BASES_RECEIPT {{\"relation\":\"finite-bases/v0\",\"cases\":{},\"accepted\":{},\"refused\":{},\"principal_families\":{},\"ambiguous\":0,\"max_family\":{},\"real_accepted\":{},\"real_refused\":{},\"decision\":\"no-go\"}}",
        cases,
        accepted,
        refused,
        principal_families,
        max_family,
        real_accepted,
        real_refused
    );
}

#[test]
fn effect_widening_is_accepted_and_narrowing_is_refused() {
    let pure = interface(Vec::new(), vec![Ty::I64], effect_set(&[]));
    let emit = interface(Vec::new(), vec![Ty::I64], effect_set(&[TEST_EMIT]));
    assert!(explicit_accepts(&pure, &emit, &[TEST_EMIT]));
    assert!(backward_accepts(&pure, &emit));
    assert!(!explicit_accepts(&emit, &pure, &[TEST_EMIT]));
    assert!(!backward_accepts(&emit, &pure));
}

#[test]
fn representation_changing_data_covariance_is_refused() {
    let list = Ty::List(Box::new(Ty::I64));
    let sum = Ty::List(Box::new(Ty::Sum(Box::new(Ty::I64), Box::new(Ty::Unit))));
    assert!(is_data_subtype(&list, &list));
    assert!(!is_data_subtype(&list, &sum));
}

#[test]
fn cross_candidate_mixing_is_not_a_whole_candidate() {
    let base = interface(Vec::new(), vec![Ty::I64], effect_set(&[]));
    let universe = [TEST_EMIT, TEST_CLOCK];
    let candidates = family(&base, &universe);
    assert!(is_whole_candidate(&candidates[0], &base));
    let mixed = Candidate {
        interface: interface(
            candidates[0].interface.inputs.clone(),
            candidates[0].interface.outputs.clone(),
            candidates[1].interface.effects.clone(),
        ),
    };
    if mixed.interface.effects != base.effects {
        assert!(!is_whole_candidate(&mixed, &base));
    }
}

#[test]
fn nominal_identity_is_exact() {
    let first = Ty::Nominal(
        NominalTypeId {
            module: 1,
            ordinal: 0,
        },
        Box::new(NominalShape::Opaque(Box::new(Ty::I64))),
    );
    let second = Ty::Nominal(
        NominalTypeId {
            module: 1,
            ordinal: 1,
        },
        Box::new(NominalShape::Opaque(Box::new(Ty::I64))),
    );
    assert!(is_data_subtype(&first, &first));
    assert!(!is_data_subtype(&first, &second));
}
