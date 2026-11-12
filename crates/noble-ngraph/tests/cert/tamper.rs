//! Corruption controls for the multi-step certificate (issue #42).
//!
//! The fixture records two folds and one `x * 1` identity. Every recorded field of a step
//! is part of its premise, so each control below corrupts exactly one of them and the
//! checker must refuse it. A checker that validated only the leading step, or that compared
//! an identity step's operands but not its recorded bits and claim, would accept some.

/// The fixture's original graph, its emitted derivation and the emitted endpoint.
fn endpoints() -> (
    noble_ngraph::ir::Graph,
    noble_ngraph::cert::Derivation,
    noble_ngraph::ir::Graph,
) {
    let before = super::differential::independent_folds();
    let (derivation, after) = super::differential::fold(&before);
    (before, derivation, after)
}

/// The checker's verdict on `derivation` with `corrupt` applied.
fn verdict(
    before: &noble_ngraph::ir::Graph,
    after: &noble_ngraph::ir::Graph,
    derivation: &noble_ngraph::cert::Derivation,
    corrupt: &dyn Fn(&mut noble_ngraph::cert::Derivation),
) -> Result<(), noble_ngraph::cert::Refusal> {
    let mut corrupted = derivation.clone();
    corrupt(&mut corrupted);
    noble_ngraph::cert::check(&super::differential::TestIds, before, after, &corrupted)
}

/// Both folds of the fixture are bound: the second fold's claim and declared constant.
fn folds_are_bound(
    before: &noble_ngraph::ir::Graph,
    after: &noble_ngraph::ir::Graph,
    derivation: &noble_ngraph::cert::Derivation,
) {
    assert_eq!(
        verdict(before, after, derivation, &|corrupted| {
            corrupted.steps[1].claimed ^= 1;
        }),
        Err(noble_ngraph::cert::Refusal::ClaimMismatch),
        "the second fold's claim"
    );
    assert_eq!(
        verdict(before, after, derivation, &|corrupted| {
            corrupted.steps[1].second_bits ^= 1;
        }),
        Err(noble_ngraph::cert::Refusal::ConstantMismatch),
        "the second fold's declared constant"
    );
}

/// The recorded `x * 1` identity step is the canonical premise.
fn identity_premise_is_canonical(derivation: &noble_ngraph::cert::Derivation) {
    let Some(identity) = derivation.steps.get(2) else {
        panic!("the fixture records an identity step after its two folds");
    };
    assert_eq!(identity.rule, noble_ngraph::cert::Rule::NeutralMulRight);
    assert_eq!(
        (identity.first_bits, identity.second_bits, identity.claimed),
        (1, 1, 0),
        "the emitted identity premise is the canonical one"
    );
}

/// Every recorded field of the identity step is part of its premise.
fn identity_fields_are_bound(
    before: &noble_ngraph::ir::Graph,
    after: &noble_ngraph::ir::Graph,
    derivation: &noble_ngraph::cert::Derivation,
) {
    assert_eq!(
        verdict(before, after, derivation, &|corrupted| {
            corrupted.steps[2].claimed ^= 1;
        }),
        Err(noble_ngraph::cert::Refusal::Malformed),
        "the identity step's claim"
    );
    assert_eq!(
        verdict(before, after, derivation, &|corrupted| {
            corrupted.steps[2].claimed = u64::MAX;
        }),
        Err(noble_ngraph::cert::Refusal::Malformed),
        "the identity step's claim at the field's extreme"
    );
    assert_eq!(
        verdict(before, after, derivation, &|corrupted| {
            corrupted.steps[2].first_bits ^= 1;
        }),
        Err(noble_ngraph::cert::Refusal::Malformed),
        "the identity step's first recorded constant"
    );
    assert_eq!(
        verdict(before, after, derivation, &|corrupted| {
            corrupted.steps[2].second_bits ^= 1;
        }),
        Err(noble_ngraph::cert::Refusal::Malformed),
        "the identity step's second recorded constant"
    );
}

/// Node order, a fold applied without its step, and the uncorrupted certificate.
fn structure_is_bound(
    before: &noble_ngraph::ir::Graph,
    after: &noble_ngraph::ir::Graph,
    derivation: &noble_ngraph::cert::Derivation,
) {
    assert_eq!(
        verdict(before, after, derivation, &|corrupted| {
            corrupted.steps.swap(1, 2);
        }),
        Err(noble_ngraph::cert::Refusal::Order),
        "steps out of node order"
    );
    assert_eq!(
        verdict(before, after, derivation, &|corrupted| {
            corrupted.steps.remove(1);
        }),
        Err(noble_ngraph::cert::Refusal::ResultMismatch),
        "a fold applied in the result but missing from the derivation"
    );
    assert_eq!(
        verdict(before, after, derivation, &|_| {}),
        Ok(()),
        "the uncorrupted certificate is accepted"
    );
}

pub(super) fn later_steps_are_refused() {
    let (before, derivation, after) = endpoints();
    folds_are_bound(&before, &after, &derivation);
    identity_premise_is_canonical(&derivation);
    identity_fields_are_bound(&before, &after, &derivation);
    structure_is_bound(&before, &after, &derivation);
}
