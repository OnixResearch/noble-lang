use noble_ngraph::belt::target::{self, Machine};
use noble_ngraph::ir::{Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::sem::Value;

pub(crate) fn graph() -> Graph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "target-belt-114-basic"),
    );
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let z = builder.i64_const(-3);
    let sum = builder.op(Op::I64Add(x, y));
    let difference = builder.op(Op::I64Sub(y, x));
    let product = builder.op(Op::I64Mul(sum, z));
    for node in [sum, difference, product] { builder.output(node); }
    builder.finish()
}

pub(crate) fn checked(graph: Graph) -> noble_ngraph::CheckedGraph {
    noble_ngraph::check(&super::common::IDS, graph).expect("fixed test graph checks")
}

#[test]
fn fixed_profiles_produce_bound_checked_and_interpreted_schedules() {
    let accepted_graph = checked(graph());
    let expected = [Value::I64(8), Value::I64(-2), Value::I64(-24)];
    for machine in [Machine::T1, Machine::T2] {
        let descriptor = machine.descriptor();
        assert_eq!(descriptor.initiation_interval, 1);
        let lowered = target::lower(&super::common::IDS, &accepted_graph, machine)
            .expect("the bounded test graph schedules");
        let schedule = target::check(&super::common::IDS, &accepted_graph, machine, &lowered.bytes)
            .expect("the independent checker accepts the producer's artifact");
        assert_eq!(schedule.machine(), machine);
        assert_eq!(schedule.event_count(), lowered.event_count);
        assert_eq!(
            target::execute_checked(
                &super::common::IDS,
                &accepted_graph,
                schedule,
                &[Value::I64(5), Value::I64(3)],
            ),
            Ok(expected.to_vec()),
        );
    }
}
