//! Corruption controls for the multi-step certificate (issue #42).
//!
//! The fixture records two folds and one `x * 1` identity. Every recorded field of a step
//! is part of its premise, so each control below corrupts exactly one of them and the
//! checker must refuse it. A checker that validated only the leading step, or that compared
//! an identity step's operands but not its recorded bits and claim, would accept some.

use noble_ngraph::cert::{Derivation, Refusal, Rule};

use super::differential;

pub(super) fn later_steps_are_refused() {
    let before = differential::independent_folds();
    let (derivation, after) = differential::fold(&before);
    let scheme = differential::TestIds;
    let refused = |corrupt: &dyn Fn(&mut Derivation)| {
        let mut corrupted = derivation.clone();
        corrupt(&mut corrupted);
        noble_ngraph::cert::check(&scheme, &before, &after, &corrupted)
    };
    let identity = derivation
        .steps
        .get(2)
        .expect("the fixture records an identity step after its two folds");
    assert_eq!(identity.rule, Rule::NeutralMulRight);
    assert_eq!(
        (identity.first_bits, identity.second_bits, identity.claimed),
        (1, 1, 0),
        "the emitted identity premise is the canonical one"
    );
    // Each corruption touches a step after the first, so a checker that validated only the
    // leading step would accept all of them.
    assert_eq!(
        refused(&|corrupted| corrupted.steps[1].claimed ^= 1),
        Err(Refusal::ClaimMismatch),
        "the second fold's claim"
    );
    assert_eq!(
        refused(&|corrupted| corrupted.steps[1].second_bits ^= 1),
        Err(Refusal::ConstantMismatch),
        "the second fold's declared constant"
    );
    assert_eq!(
        refused(&|corrupted| corrupted.steps[2].operands.swap(0, 1)),
        Err(Refusal::Malformed),
        "the identity step's operand order"
    );
    assert_eq!(
        refused(&|corrupted| corrupted.steps[2].claimed ^= 1),
        Err(Refusal::Malformed),
        "the identity step's claim"
    );
    assert_eq!(
        refused(&|corrupted| corrupted.steps[2].claimed = u64::MAX),
        Err(Refusal::Malformed),
        "the identity step's claim at the field's extreme"
    );
    assert_eq!(
        refused(&|corrupted| corrupted.steps[2].first_bits ^= 1),
        Err(Refusal::Malformed),
        "the identity step's first recorded constant"
    );
    assert_eq!(
        refused(&|corrupted| corrupted.steps[2].second_bits ^= 1),
        Err(Refusal::Malformed),
        "the identity step's second recorded constant"
    );
    assert_eq!(
        refused(&|corrupted| corrupted.steps.swap(1, 2)),
        Err(Refusal::Order),
        "steps out of node order"
    );
    assert_eq!(
        refused(&|corrupted| {
            corrupted.steps.remove(1);
        }),
        Err(Refusal::ResultMismatch),
        "a fold applied in the result but missing from the derivation"
    );
    assert_eq!(
        refused(&|_| {}),
        Ok(()),
        "the uncorrupted certificate is accepted"
    );
}
