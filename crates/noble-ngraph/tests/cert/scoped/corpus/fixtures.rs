use noble_ngraph::ir::{
    Fact, Graph, GraphBuilder, Literal, Op, Provenance, ScalarType, SubProfile, Subject,
};

use super::super::super::differential::TestIds;

fn exact(value: i64) -> Fact {
    Fact::Range {
        lo: Literal::I64(value),
        hi: Literal::I64(value),
    }
}

pub(super) fn guarded_fixture() -> Graph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&TestIds, "scoped-lean-guarded"),
    );
    let input = builder.input(ScalarType::I64);
    let zero = builder.i64_const(0);
    builder.guard(exact(0), zero);
    builder.guard(exact(0), input);
    let sum = builder.op(Op::I64Add(input, zero));
    builder.output(sum);
    builder.finish()
}

pub(super) fn preflight_fixture() -> Graph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&TestIds, "scoped-lean-preflight"),
    );
    let input = builder.input(ScalarType::I64);
    let nine = builder.i64_const(9);
    builder.guard(exact(9), nine);
    let difference = builder.op(Op::I64Sub(nine, input));
    builder.fact(input, exact(3), Provenance::InputValidated);
    builder.output(difference);
    builder.finish()
}
