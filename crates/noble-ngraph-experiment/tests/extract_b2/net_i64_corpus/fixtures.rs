use noble_ngraph::check;
use noble_ngraph::eval::{self, Control};
use noble_ngraph::ir::{Graph, GraphBuilder, ScalarType, SubProfile, Subject};
use noble_ngraph::net::{self, InteractionGraph};
use noble_ngraph::sem::Value;
use noble_ngraph_experiment::scheme::BLAKE3;

#[derive(Clone, Debug)]
pub(super) struct Fixture {
    pub(super) name: &'static str,
    pub(super) graph: Graph,
    pub(super) inputs: Vec<Value>,
}

fn full_fixture() -> Fixture {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&BLAKE3, "net-i64-corpus/fanout-overflow"),
    );
    let left = builder.input(ScalarType::I64);
    let right = builder.input(ScalarType::I64);
    let _discarded = builder.input(ScalarType::I64);
    let three = builder.i64_const(3);
    let sum = builder.op(noble_ngraph::ir::Op::I64Add(left, three));
    let difference = builder.op(noble_ngraph::ir::Op::I64Sub(right, left));
    let product = builder.op(noble_ngraph::ir::Op::I64Mul(sum, difference));
    let equal = builder.op(noble_ngraph::ir::Op::I64Eq(left, right));
    builder.output(product);
    builder.output(equal);
    builder.output(product);
    Fixture {
        name: "fanout_overflow",
        graph: builder.finish(),
        inputs: vec![Value::I64(i64::MAX), Value::I64(-1), Value::I64(17)],
    }
}

fn discarded_fixture() -> Fixture {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&BLAKE3, "net-i64-corpus/discarded-unit"),
    );
    let _discarded = builder.input(ScalarType::I64);
    Fixture {
        name: "discarded_unit",
        graph: builder.finish(),
        inputs: vec![Value::I64(i64::MIN)],
    }
}

fn empty_fixture() -> Fixture {
    let builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&BLAKE3, "net-i64-corpus/empty"),
    );
    Fixture {
        name: "empty",
        graph: builder.finish(),
        inputs: Vec::new(),
    }
}

pub(super) fn build_fixtures() -> [Fixture; 3] {
    [full_fixture(), discarded_fixture(), empty_fixture()]
}

pub(super) fn checked_and_net(fixture: &Fixture) -> (noble_ngraph::CheckedGraph, InteractionGraph) {
    let checked = check::check(&BLAKE3, fixture.graph.clone()).expect("corpus graph checks");
    let limits = net::Limits::default();
    let candidate = net::lower(&BLAKE3, &checked, &fixture.inputs, &limits)
        .expect("checked corpus graph lowers");
    assert_eq!(
        net::validate(&BLAKE3, &checked, &candidate, &fixture.inputs, &limits),
        Ok(()),
        "independent validator admits {}",
        fixture.name
    );
    (checked, candidate)
}

pub(super) fn rust_result(
    fixture: &Fixture,
    checked: &noble_ngraph::CheckedGraph,
    candidate: &InteractionGraph,
) -> (Vec<Value>, u64) {
    let mut control = Control::default();
    let reference = eval::reference(checked, &fixture.inputs, None, &mut control);
    let reference_outputs = match reference.obs {
        eval::Obs::Normal(outputs) => outputs,
        other => panic!("{} reference refused: {other:?}", fixture.name),
    };
    let limits = net::Limits::default();
    let plan = net::Plan {
        schedule: Some(net::Schedule::Fifo),
        ..net::Plan::default()
    };
    let report = net::reduce(
        &BLAKE3,
        net::ReduceRequest {
            checked,
            candidate,
            inputs: &fixture.inputs,
            limits: &limits,
            plan,
        },
    );
    let net_outputs = match report.obs {
        net::Obs::Normal(outputs) => outputs,
        other => panic!("{} net refused: {other:?}", fixture.name),
    };
    assert_eq!(
        net_outputs, reference_outputs,
        "{} Rust differential",
        fixture.name
    );
    let expected = candidate
        .expected_interactions()
        .expect("interaction count fits the contract counter");
    assert_eq!(report.interactions, expected, "{} Rust count", fixture.name);
    (net_outputs, report.interactions)
}
