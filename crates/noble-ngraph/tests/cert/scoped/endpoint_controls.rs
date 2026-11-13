use noble_ngraph::cert_scoped::{
    self, Derivation, Outcome, Refusal, Relation, Rule, Scope, Step, Unsupported,
};
use noble_ngraph::ir::{FactRecord, Literal, NodeId, Op, Provenance, ScalarType};

use super::TestIds;
use super::{admitted, builder, check, exact, guarded_fold, preflight_fold, propose};

#[test]
fn dropping_a_guard_assumption_cannot_leave_an_equality_certificate() {
    let graph = guarded_fold();
    let (mut derivation, result) = propose(&graph);
    let mut without_assumption = graph.clone();
    without_assumption.nodes[2].op = Op::Const(Literal::I64(0));
    derivation.left = without_assumption.logical_id(&TestIds);
    assert_eq!(
        check(&without_assumption, &result, &derivation),
        Outcome::Refused(Refusal::ScopeUnavailable)
    );
}

#[test]
fn a_changed_input_domain_is_refused() {
    let graph = preflight_fold();
    let (mut derivation, result) = propose(&graph);
    let mut narrowed = result.clone();
    narrowed.facts.push(FactRecord {
        node: NodeId(0),
        fact: exact(4),
        provenance: Provenance::InputValidated,
    });
    derivation.right = narrowed.logical_id(&TestIds);
    assert_eq!(
        check(&graph, &narrowed, &derivation),
        Outcome::Refused(Refusal::InterfaceChanged)
    );
}

#[test]
fn a_substituted_result_is_refused() {
    let graph = guarded_fold();
    let (mut derivation, result) = propose(&graph);
    let mut substituted = result.clone();
    substituted.nodes[3].op = Op::Const(Literal::I64(8));
    derivation.right = substituted.logical_id(&TestIds);
    assert_eq!(
        check(&graph, &substituted, &derivation),
        Outcome::Refused(Refusal::ResultMismatch)
    );
}

#[test]
fn a_weaker_relation_is_unsupported() {
    let graph = guarded_fold();
    let (derivation, result) = propose(&graph);
    for relation in [Relation::ScopedEquality, Relation::DirectedRefinement] {
        let mut weaker = derivation.clone();
        weaker.relation = relation;
        assert_eq!(
            check(&graph, &result, &weaker),
            Outcome::Unsupported(Unsupported::Relation(relation))
        );
    }
    let mut reversed = derivation;
    reversed.relation = Relation::DirectedRefinement;
    reversed.left = result.logical_id(&TestIds);
    reversed.right = graph.logical_id(&TestIds);
    assert_eq!(
        check(&result, &graph, &reversed),
        Outcome::Unsupported(Unsupported::Relation(Relation::DirectedRefinement))
    );
}

#[test]
fn an_out_of_slice_operation_is_unsupported() {
    let mut builder = builder("scoped-out-of-slice");
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let equal = builder.op(Op::I64Eq(x, y));
    builder.output(equal);
    let graph = builder.finish();
    admitted(&graph);
    let derivation = Derivation {
        revision: String::from(cert_scoped::SCOPED_REVISION),
        relation: Relation::Equality,
        steps: vec![Step {
            rule: Rule::Preflight,
            node: NodeId(2),
            operands: [NodeId(0), NodeId(1)],
            scopes: [Scope::Constant, Scope::Constant],
            operand_values: [0, 0],
            claimed: 0,
        }],
        left: graph.logical_id(&TestIds),
        right: graph.logical_id(&TestIds),
    };
    assert_eq!(
        check(&graph, &graph, &derivation),
        Outcome::Unsupported(Unsupported::Operation("i64.eq"))
    );
}
