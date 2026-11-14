use super::{assert_refusal, target, Machine, Refusal};
use noble_ngraph::belt::target::Operation;
use noble_ngraph::ir::{GraphBuilder, Op, ScalarType, SubProfile, Subject};

fn checker_exhaustion_events() -> Vec<target::Issue> {
    let mut events = Vec::with_capacity(50_003);
    events.push(target::Issue {
        cycle: 0,
        lane: 0,
        unit: 3,
        operation: Operation::Input { node: 0, slot: 0 },
    });
    events.push(target::Issue {
        cycle: 0,
        lane: 1,
        unit: 4,
        operation: Operation::Input { node: 1, slot: 1 },
    });
    events.push(target::Issue {
        cycle: 1,
        lane: 0,
        unit: 0,
        operation: Operation::Add {
            node: 2,
            left_age: 1,
            right_age: 0,
        },
    });
    for cycle in 2..50_002 {
        events.push(target::Issue {
            cycle,
            lane: 0,
            unit: 3,
            operation: Operation::Save {
                node: 0,
                age: 2,
                slot: 0,
            },
        });
    }
    assert_eq!(events.len(), 50_003);
    events
}

#[test]
fn producer_work_exhaustion_refuses() {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(
            &super::super::common::IDS,
            "target-belt-114-producer-exhaustion",
        ),
    );
    let mut values = Vec::with_capacity(20);
    for value in 0..20 {
        values.push(builder.i64_const(i64::from(value)));
    }
    let mut result = values[0];
    for value in values.into_iter().skip(1) {
        result = builder.op(Op::I64Add(result, value));
    }
    builder.output(result);
    let accepted_graph = super::super::belt_target_tests::checked(builder.finish());
    assert!(matches!(
        target::lower(&super::super::common::IDS, &accepted_graph, Machine::T1),
        Err(Refusal::SchedulingExhausted)
    ));
}

#[test]
fn checker_work_exhaustion_refuses_after_bounded_prefix() {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(
            &super::super::common::IDS,
            "target-belt-114-checker-exhaustion",
        ),
    );
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let sum = builder.op(Op::I64Add(x, y));
    builder.output(sum);
    let accepted_graph = super::super::belt_target_tests::checked(builder.finish());
    let events = checker_exhaustion_events();
    assert_refusal(
        &accepted_graph,
        Machine::T2,
        &events,
        &[target::OutputRead { node: 2 }],
        Refusal::CheckingExhausted,
    );
}
