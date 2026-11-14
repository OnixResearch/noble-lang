use noble_ngraph::belt::target::{self, Machine, OutputRead, Refusal};
use noble_ngraph::ir::{Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};

fn pressure_graph() -> Graph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "target-belt-114-preserve"),
    );
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let c1 = builder.i64_const(2);
    let c2 = builder.i64_const(3);
    let c3 = builder.i64_const(4);
    let c4 = builder.i64_const(5);
    let sum = builder.op(Op::I64Add(x, y));
    let result = builder.op(Op::I64Add(sum, c4));
    for node in [c1, c2, c3, result] {
        builder.output(node);
    }
    builder.finish()
}

fn identity_graph() -> Graph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "target-belt-114-different-identity"),
    );
    let input = builder.input(ScalarType::I64);
    builder.output(input);
    builder.finish()
}

fn encode(
    checked: &noble_ngraph::CheckedGraph,
    machine: Machine,
    events: &[target::Issue],
    outputs: &[OutputRead],
) -> Vec<u8> {
    target::encode(&super::common::IDS, checked, machine, events, outputs)
        .expect("a structurally valid candidate encodes")
}

fn fixture_graph() -> noble_ngraph::CheckedGraph {
    super::belt_target_tests::checked(super::belt_target_tests::graph())
}

fn assert_refusal(
    checked: &noble_ngraph::CheckedGraph,
    machine: Machine,
    events: &[target::Issue],
    outputs: &[OutputRead],
    expected: Refusal,
) {
    let bytes = encode(checked, machine, events, outputs);
    assert!(matches!(
        target::check(&super::common::IDS, checked, machine, &bytes),
        Err(actual) if actual == expected,
    ));
}

#[path = "mutations/exhaustion.rs"]
mod exhaustion;
#[path = "mutations/identity.rs"]
mod identity;
#[path = "mutations/legality.rs"]
mod legality;
#[path = "mutations/preservation.rs"]
mod preservation;
