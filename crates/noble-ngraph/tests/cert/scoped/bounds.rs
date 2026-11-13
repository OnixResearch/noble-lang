use noble_ngraph::cert_scoped::{self, Derivation, Exhausted, Outcome, Relation};

use super::TestIds;
use super::{builder, check, guarded_fold, propose, two_guards};

#[test]
fn limits_are_explicit_outcomes() {
    let graph = guarded_fold();
    let (derivation, result) = propose(&graph);
    let mut oversized = derivation.clone();
    let step = derivation.steps[0];
    oversized.steps = vec![step; cert_scoped::MAX_STEPS.saturating_add(1)];
    assert_eq!(
        check(&graph, &result, &oversized),
        Outcome::Exhausted(Exhausted::Steps)
    );
    let mut builder = builder("scoped-wide");
    for _ in 0..=cert_scoped::MAX_WITNESS_NODES {
        builder.i64_const(0);
    }
    let wide = builder.finish();
    let wide_derivation = Derivation {
        revision: String::from(cert_scoped::SCOPED_REVISION),
        relation: Relation::Equality,
        steps: Vec::new(),
        left: wide.logical_id(&TestIds),
        right: wide.logical_id(&TestIds),
    };
    assert_eq!(
        cert_scoped::check(&TestIds, &wide, &wide, &wide_derivation),
        Outcome::Exhausted(Exhausted::WitnessNodes)
    );
}

#[test]
fn cancellation_is_a_distinct_non_accepting_outcome() {
    let graph = two_guards();
    let (derivation, result) = propose(&graph);
    assert_eq!(derivation.steps.len(), 2);
    assert_eq!(
        cert_scoped::check_with_cancel(&TestIds, &graph, &result, &derivation, || true),
        Outcome::Cancelled
    );
    let mut polls = 0;
    assert_eq!(
        cert_scoped::check_with_cancel(&TestIds, &graph, &result, &derivation, || {
            polls += 1;
            polls == 3
        }),
        Outcome::Cancelled
    );
    assert_eq!(polls, 3, "cancellation is polled between derivation steps");
    assert_eq!(Outcome::Cancelled.label(), "cancelled");
}
