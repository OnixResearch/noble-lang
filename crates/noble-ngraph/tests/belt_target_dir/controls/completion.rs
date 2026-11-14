use noble_ngraph::belt::target::{self, Issue, Machine, Operation, OutputRead, Refusal};
use noble_ngraph::ir::{GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::sem::Value;

fn issue(cycle: u32, lane: u8, unit: u8, operation: Operation) -> Issue {
    Issue {
        cycle,
        lane,
        unit,
        operation,
    }
}

fn graph() -> noble_ngraph::CheckedGraph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(
            &super::super::common::IDS,
            "target-belt-114-completion-priority",
        ),
    );
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let z = builder.input(ScalarType::I64);
    let w = builder.input(ScalarType::I64);
    let left = builder.op(Op::I64Add(x, y));
    let right = builder.op(Op::I64Sub(z, w));
    let result = builder.op(Op::I64Add(left, right));
    builder.output(result);
    super::checked(builder)
}

fn completion_events() -> [Issue; 7] {
    [
        issue(0, 0, 3, Operation::Input { node: 0, slot: 0 }),
        issue(0, 1, 4, Operation::Input { node: 1, slot: 1 }),
        issue(1, 0, 3, Operation::Input { node: 2, slot: 2 }),
        issue(1, 1, 4, Operation::Input { node: 3, slot: 3 }),
        issue(
            2,
            0,
            0,
            Operation::Add {
                node: 4,
                left_age: 3,
                right_age: 2,
            },
        ),
        issue(
            2,
            1,
            1,
            Operation::Sub {
                node: 5,
                left_age: 1,
                right_age: 0,
            },
        ),
        issue(
            3,
            0,
            0,
            Operation::Add {
                node: 6,
                left_age: 1,
                right_age: 0,
            },
        ),
    ]
}

fn expect_refusal(
    checked: &noble_ngraph::CheckedGraph,
    events: &[Issue],
    outputs: &[OutputRead],
    expected: Refusal,
) {
    let bytes = super::encode_candidate(checked, Machine::T2, events, outputs);
    assert!(matches!(
        target::check(&super::super::common::IDS, checked, Machine::T2, &bytes),
        Err(actual) if actual == expected,
    ));
}

#[test]
fn completion_priority_controls_operand_ages_and_lane_unit_conflicts_refuse() {
    let accepted_graph = graph();
    let mut events = completion_events();
    let outputs = [OutputRead { node: 6 }];
    let bytes = super::encode_candidate(&accepted_graph, Machine::T2, &events, &outputs);
    assert_eq!(
        target::execute(
            &super::super::common::IDS,
            &accepted_graph,
            Machine::T2,
            &bytes,
            &[Value::I64(5), Value::I64(3), Value::I64(9), Value::I64(1)],
        ),
        Ok(vec![Value::I64(16)]),
    );

    events[4].unit = 1;
    events[5].unit = 0;
    expect_refusal(&accepted_graph, &events, &outputs, Refusal::OperandMismatch);

    events = completion_events();
    events[5].lane = 0;
    expect_refusal(&accepted_graph, &events, &outputs, Refusal::IssueConflict);

    events = completion_events();
    events[5].unit = 0;
    expect_refusal(&accepted_graph, &events, &outputs, Refusal::UnitConflict);
}
