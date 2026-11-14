use crate::differential::TestIds;
use noble_ngraph::cert::{self, Proposer};
use noble_ngraph::ir::{Fact, GraphBuilder, Literal, Op, ScalarType, SubProfile, Subject};

#[test]
fn pruning_keeps_a_guard_outside_output_ancestry() {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&TestIds, "cert-prune-guard-root"),
    );
    let input = builder.input(ScalarType::I64);
    let zero = builder.i64_const(0);
    let sum = builder.op(Op::I64Add(zero, zero));
    builder.guard(
        Fact::Range {
            lo: Literal::I64(0),
            hi: Literal::I64(0),
        },
        input,
    );
    builder.output(sum);
    let graph = builder.finish();

    let (derivation, result) =
        cert::propose_chained(&TestIds, Proposer::FixedPoint, &graph).expect("proposal accepted");
    assert_eq!(derivation.steps.len(), 2, "rewrite then final prune");
    assert_eq!(result.nodes.len(), 3, "the dead constant operand is pruned");
    assert!(
        result
            .nodes
            .iter()
            .any(|node| matches!(node.op, Op::Guard { .. })),
        "a guard remains a liveness root even when it is not output ancestry"
    );
    noble_ngraph::check(&TestIds, result.clone()).expect("guard-preserving candidate admitted");
    assert_eq!(
        cert::check_chained(&TestIds, &graph, &result, &derivation),
        Ok(())
    );
}
