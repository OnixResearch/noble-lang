use noble_ngraph::cert_scoped::{self, Derivation, Outcome, Refusal, Relation, Rule, Scope, Step};
use noble_ngraph::ir::{Literal, NodeId, Op};

use super::TestIds;
use super::{
    admitted, assert_observations_agree, both, check, derived_fact_fold, future_guard,
    future_guard_folded, guarded_fold, inputs, preflight_fold, propose,
};

#[test]
fn a_future_guard_cannot_justify_an_earlier_rewrite() {
    let graph = future_guard();
    admitted(&graph);
    // The proposer must not emit the step: no guard below node 2 pins `x`.
    let (empty, _) = propose(&graph);
    assert!(
        empty.steps.is_empty(),
        "a future guard must not be proposed"
    );
    // A hand-written witness that names the future guard is refused.
    let folded = future_guard_folded();
    admitted(&folded);
    let derivation = Derivation {
        revision: String::from(cert_scoped::SCOPED_REVISION),
        relation: Relation::Equality,
        steps: vec![Step {
            rule: Rule::Scoped,
            node: NodeId(2),
            operands: [NodeId(0), NodeId(1)],
            scopes: [Scope::Guard(NodeId(4)), Scope::Constant],
            operand_values: [0, 1],
            claimed: 1,
        }],
        left: graph.logical_id(&TestIds),
        right: folded.logical_id(&TestIds),
    };
    assert_eq!(
        check(&graph, &folded, &derivation),
        Outcome::Refused(Refusal::ScopeUnavailable)
    );
    // The refusal is load-bearing: the same rewrite changes which guard fails.
    let (scoped_reference, _) = both(&graph, &inputs(&[5]));
    let (folded_reference, _) = both(&folded, &inputs(&[5]));
    assert!(
        !scoped_reference.obs_eq(&folded_reference),
        "the future guard must matter for an input outside its scope"
    );
    // A value-only comparison cannot see it: on the in-scope input both outputs agree.
    assert_observations_agree(&graph, &folded, &[0], "future guard, in scope");
}

#[test]
fn a_wrong_guarded_value_is_refused() {
    let graph = guarded_fold();
    let (mut derivation, result) = propose(&graph);
    derivation.steps[0].operand_values[0] = 1;
    assert_eq!(
        check(&graph, &result, &derivation),
        Outcome::Refused(Refusal::ScopeValueMismatch)
    );
    let (mut derivation, result) = propose(&graph);
    derivation.steps[0].claimed = 8;
    assert_eq!(
        check(&graph, &result, &derivation),
        Outcome::Refused(Refusal::ClaimMismatch)
    );
}

#[test]
fn every_step_witness_field_is_bound() {
    let graph = guarded_fold();
    let (derivation, result) = propose(&graph);

    let mut changed = derivation.clone();
    changed.steps[0].rule = Rule::Preflight;
    assert_eq!(
        check(&graph, &result, &changed),
        Outcome::Refused(Refusal::ScopeKindMismatch)
    );

    let mut changed = derivation.clone();
    changed.steps[0].node = NodeId(4);
    assert_eq!(
        check(&graph, &result, &changed),
        Outcome::Refused(Refusal::Malformed)
    );

    let mut changed = derivation.clone();
    changed.steps[0].operands.swap(0, 1);
    assert_eq!(
        check(&graph, &result, &changed),
        Outcome::Refused(Refusal::Malformed)
    );

    let mut changed = derivation.clone();
    changed.steps[0].scopes[0] = Scope::Constant;
    assert_eq!(
        check(&graph, &result, &changed),
        Outcome::Refused(Refusal::ScopeUnavailable)
    );

    let mut changed = derivation.clone();
    changed.revision = String::from("ngraph-cert-scoped/unknown");
    assert_eq!(
        check(&graph, &result, &changed),
        Outcome::Refused(Refusal::StaleRevision)
    );

    let mut changed = derivation.clone();
    changed.left = result.logical_id(&TestIds);
    assert_eq!(
        check(&graph, &result, &changed),
        Outcome::Refused(Refusal::EndpointMismatch)
    );

    let mut changed = derivation;
    changed.right = graph.logical_id(&TestIds);
    assert_eq!(
        check(&graph, &result, &changed),
        Outcome::Refused(Refusal::EndpointMismatch)
    );
}

#[test]
fn a_stale_fact_cannot_stand_in_for_an_admitted_input_domain() {
    // A derived fact really does pin the subtraction to 6 in this admitted graph, but a
    // derivation may only use an admitted input fact, so the witness is refused.
    let graph = derived_fact_fold();
    admitted(&graph);
    let mut result = graph.clone();
    result.nodes[3].op = Op::Const(Literal::I64(15));
    let derivation = Derivation {
        revision: String::from(cert_scoped::SCOPED_REVISION),
        relation: Relation::Equality,
        steps: vec![Step {
            rule: Rule::Preflight,
            node: NodeId(3),
            operands: [NodeId(2), NodeId(1)],
            scopes: [Scope::InputFact, Scope::Constant],
            operand_values: [6, 9],
            claimed: 15,
        }],
        left: graph.logical_id(&TestIds),
        right: result.logical_id(&TestIds),
    };
    assert_eq!(
        check(&graph, &result, &derivation),
        Outcome::Refused(Refusal::ScopeUnavailable)
    );
    // Dropping the input fact from the resulting graph is the stale-fact case where the
    // input domain silently widens, and it is refused as an interface change.
    let graph = preflight_fold();
    let (mut derivation, result) = propose(&graph);
    let mut widened = result.clone();
    widened.facts.clear();
    derivation.right = widened.logical_id(&TestIds);
    assert_eq!(
        check(&graph, &widened, &derivation),
        Outcome::Refused(Refusal::InterfaceChanged)
    );
}

#[test]
fn a_removed_guard_is_refused() {
    let graph = guarded_fold();
    let (mut derivation, result) = propose(&graph);
    let mut without_guard = result.clone();
    without_guard.nodes[2].op = Op::Const(Literal::I64(0));
    derivation.right = without_guard.logical_id(&TestIds);
    assert_eq!(
        check(&graph, &without_guard, &derivation),
        Outcome::Refused(Refusal::GuardChanged)
    );
}
