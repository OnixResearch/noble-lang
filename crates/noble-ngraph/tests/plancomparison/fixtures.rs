use noble_ngraph::ir::{GraphBuilder, Op, ScalarType, SubProfile, Subject};

fn checked(builder: GraphBuilder, label: &str) -> noble_ngraph::CheckedGraph {
    match noble_ngraph::check::check(&super::common::IDS, builder.finish()) {
        Ok(checked) => checked,
        Err(error) => panic!("{label} graph rejected: {error}"),
    }
}

pub(super) fn chain_graph() -> noble_ngraph::CheckedGraph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "net-plan-chain"),
    );
    let a = builder.input(ScalarType::I64);
    let b = builder.input(ScalarType::I64);
    let c = builder.input(ScalarType::I64);
    let d = builder.input(ScalarType::I64);
    let ab = builder.op(Op::I64Add(a, b));
    let abc = builder.op(Op::I64Mul(ab, c));
    let result = builder.op(Op::I64Sub(abc, d));
    builder.output(result);
    checked(builder, "chain")
}

pub(super) fn fanout_graph() -> noble_ngraph::CheckedGraph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "net-plan-fanout"),
    );
    let a = builder.input(ScalarType::I64);
    let b = builder.input(ScalarType::I64);
    let difference = builder.op(Op::I64Sub(a, b));
    let square = builder.op(Op::I64Mul(difference, difference));
    builder.output(difference);
    builder.output(square);
    checked(builder, "fanout")
}

pub(super) fn erasure_graph() -> noble_ngraph::CheckedGraph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "net-plan-erasure"),
    );
    let _erased = builder.input(ScalarType::I64);
    let observed = builder.input(ScalarType::I64);
    builder.output(observed);
    checked(builder, "erasure")
}

pub(super) fn subtraction_graph() -> noble_ngraph::CheckedGraph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "net-plan-subtraction"),
    );
    let a = builder.input(ScalarType::I64);
    let b = builder.input(ScalarType::I64);
    let difference = builder.op(Op::I64Sub(a, b));
    builder.output(difference);
    checked(builder, "subtraction")
}
