use noble_ngraph::cert_scoped::{Outcome, Relation, Rule, Scope};
use noble_ngraph::eval::Obs;
use noble_ngraph::ir::NodeId;

use super::{
    admitted, assert_observations_agree, both, check, exact, guarded_fold,
    guarded_fold_without_guard, inputs, preflight_fold, preflight_fold_without_fact, propose,
    two_guards,
};

#[test]
fn the_guarded_pilot_is_accepted_and_the_guard_scope_is_recorded() {
    let graph = guarded_fold();
    admitted(&graph);
    let (derivation, result) = propose(&graph);
    assert_eq!(check(&graph, &result, &derivation), Outcome::Accepted);
    assert_eq!(derivation.relation, Relation::Equality);
    let first = derivation.steps[0];
    assert_eq!(first.node, NodeId(3));
    assert_eq!(first.rule, Rule::Scoped);
    assert_eq!(first.scopes[0], Scope::Guard(NodeId(2)));
    assert_eq!(first.scopes[1], Scope::Constant);
    assert_eq!(first.operand_values, [0, 7]);
    assert_eq!(first.claimed, 7);
    admitted(&result);
}

#[test]
fn the_preflight_pilot_is_accepted_and_the_input_fact_is_recorded() {
    let graph = preflight_fold();
    admitted(&graph);
    let (derivation, result) = propose(&graph);
    assert_eq!(check(&graph, &result, &derivation), Outcome::Accepted);
    let first = derivation.steps[0];
    assert_eq!(first.node, NodeId(2));
    assert_eq!(first.rule, Rule::Preflight);
    assert_eq!(first.scopes[0], Scope::Constant);
    assert_eq!(first.scopes[1], Scope::InputFact);
    assert_eq!(first.operand_values, [9, 3]);
    assert_eq!(first.claimed, 6);
    admitted(&result);
}

#[test]
fn the_guarded_pilot_preserves_the_observation_of_every_input() {
    let graph = guarded_fold();
    let (derivation, result) = propose(&graph);
    assert_eq!(check(&graph, &result, &derivation), Outcome::Accepted);
    // 0 satisfies the guard; 5 and -1 violate it; i64::MIN/MAX are the wrapping corners.
    for &value in &[0, 5, -1, i64::MIN, i64::MAX, 7] {
        assert_observations_agree(&graph, &result, &[value], "guarded pilot");
    }
}

#[test]
fn the_preflight_pilot_preserves_the_observation_of_every_input() {
    let graph = preflight_fold();
    let (derivation, result) = propose(&graph);
    assert_eq!(check(&graph, &result, &derivation), Outcome::Accepted);
    // 3 is the admitted domain; 4 and -1 are refused by input validation in both graphs.
    for &value in &[3, 4, -1, i64::MIN, i64::MAX] {
        assert_observations_agree(&graph, &result, &[value], "preflight pilot");
    }
}

#[test]
fn the_guard_is_load_bearing_for_the_guarded_pilot() {
    let scoped = guarded_fold();
    let (derivation, folded) = propose(&scoped);
    assert_eq!(check(&scoped, &folded, &derivation), Outcome::Accepted);
    let unscoped = guarded_fold_without_guard();
    let unscoped_inputs = inputs(&[5]);
    let (unscoped_reference, _) = both(&unscoped, &unscoped_inputs);
    let (folded_reference, _) = both(&folded, &unscoped_inputs);
    assert!(
        !unscoped_reference.obs_eq(&folded_reference),
        "dropping the guard must change the observation"
    );
    assert_observations_agree(&scoped, &folded, &[5], "guarded pilot");
}

#[test]
fn the_input_fact_is_load_bearing_for_the_preflight_pilot() {
    let scoped = preflight_fold();
    let (derivation, folded) = propose(&scoped);
    assert_eq!(check(&scoped, &folded, &derivation), Outcome::Accepted);
    let unscoped = preflight_fold_without_fact();
    let unscoped_inputs = inputs(&[4]);
    let (unscoped_reference, _) = both(&unscoped, &unscoped_inputs);
    let (folded_reference, _) = both(&folded, &unscoped_inputs);
    assert!(
        !unscoped_reference.obs_eq(&folded_reference),
        "dropping the input fact must change the observation"
    );
    assert_observations_agree(&scoped, &folded, &[4], "preflight pilot");
}

#[test]
fn the_two_guard_fixture_preserves_the_first_failure() {
    let graph = two_guards();
    admitted(&graph);
    let (derivation, result) = propose(&graph);
    assert_eq!(check(&graph, &result, &derivation), Outcome::Accepted);
    admitted(&result);
    assert_eq!(derivation.steps.len(), 2);
    assert_eq!(
        derivation.steps[0].scopes,
        [Scope::Guard(NodeId(2)), Scope::Guard(NodeId(3))]
    );
    // No predicate violated, one violated, the other violated, both violated, and shared
    // values on both sides.
    for &(x, y) in &[
        (0, 7),
        (3, 7),
        (0, 9),
        (3, 9),
        (0, 0),
        (7, 7),
        (-1, 7),
        (0, -7),
        (i64::MIN, i64::MAX),
    ] {
        assert_observations_agree(&graph, &result, &[x, y], "two guards");
    }
    // And the first failing guard is the one the reference interpreter reports, in both
    // endpoints, not just a failing guard.
    let (reference, compiled) = both(&graph, &inputs(&[3, 9]));
    assert_eq!(
        reference,
        Obs::GuardFailed {
            node: NodeId(2),
            fact: exact(0),
        }
    );
    assert!(reference.obs_eq(&compiled));
}
