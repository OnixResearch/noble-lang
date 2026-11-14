use noble_ngraph::belt::target::{self, Issue, Machine, Operation, OutputRead, Refusal};
use noble_ngraph::ir::{GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::sem::Value;

fn checked(builder: GraphBuilder) -> noble_ngraph::CheckedGraph {
    noble_ngraph::check(&super::common::IDS, builder.finish()).expect("control graph checks")
}

#[test]
fn independent_operations_reorder_and_sparse_cycles_are_idle() {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "target-belt-114-reorder"),
    );
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let sum = builder.op(Op::I64Add(x, y));
    let difference = builder.op(Op::I64Sub(y, x));
    builder.output(sum);
    builder.output(difference);
    let accepted_graph = checked(builder);
    let events = [
        Issue { cycle: 0, lane: 0, unit: 2, operation: Operation::Input { node: 0, slot: 0 } },
        Issue { cycle: 2, lane: 0, unit: 2, operation: Operation::Input { node: 1, slot: 1 } },
        Issue { cycle: 4, lane: 0, unit: 0, operation: Operation::Sub { node: 3, left_age: 0, right_age: 1 } },
        Issue { cycle: 5, lane: 0, unit: 0, operation: Operation::Add { node: 2, left_age: 2, right_age: 1 } },
    ];
    let outputs = [OutputRead { node: 2 }, OutputRead { node: 3 }];
    let bytes = target::encode(&super::common::IDS, &accepted_graph, Machine::T1, &events, &outputs)
        .expect("well-framed candidate encodes");
    assert!(target::check(&super::common::IDS, &accepted_graph, Machine::T1, &bytes).is_ok());
    assert_eq!(
        target::execute(&super::common::IDS, &accepted_graph, Machine::T1, &bytes, &[Value::I64(5), Value::I64(3)]),
        Ok(vec![Value::I64(8), Value::I64(-2)]),
    );
}

#[test]
fn same_cycle_issue_reads_only_pre_cycle_values() {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "target-belt-114-no-forward"),
    );
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let sum = builder.op(Op::I64Add(x, y));
    let dependent = builder.op(Op::I64Add(sum, x));
    builder.output(dependent);
    let accepted_graph = checked(builder);
    let events = [
        Issue { cycle: 0, lane: 0, unit: 3, operation: Operation::Input { node: 0, slot: 0 } },
        Issue { cycle: 0, lane: 1, unit: 4, operation: Operation::Input { node: 1, slot: 1 } },
        Issue { cycle: 1, lane: 0, unit: 0, operation: Operation::Add { node: 2, left_age: 1, right_age: 0 } },
        Issue { cycle: 1, lane: 1, unit: 1, operation: Operation::Add { node: 3, left_age: 0, right_age: 1 } },
    ];
    let bytes = target::encode(&super::common::IDS, &accepted_graph, Machine::T2, &events, &[OutputRead { node: 3 }])
        .expect("candidate encodes");
    assert!(matches!(
        target::check(&super::common::IDS, &accepted_graph, Machine::T2, &bytes),
        Err(Refusal::OperandMismatch)
    ));
}

#[test]
fn latency_skew_allows_out_of_order_completion() {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "target-belt-114-out-of-order"),
    );
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let product = builder.op(Op::I64Mul(x, y));
    let sum = builder.op(Op::I64Add(x, y));
    builder.output(product);
    builder.output(sum);
    let accepted_graph = checked(builder);
    let events = [
        Issue { cycle: 0, lane: 0, unit: 3, operation: Operation::Input { node: 0, slot: 0 } },
        Issue { cycle: 0, lane: 1, unit: 4, operation: Operation::Input { node: 1, slot: 1 } },
        Issue { cycle: 1, lane: 0, unit: 2, operation: Operation::Multiply { node: 2, left_age: 1, right_age: 0 } },
        Issue { cycle: 1, lane: 1, unit: 0, operation: Operation::Add { node: 3, left_age: 1, right_age: 0 } },
    ];
    let outputs = [OutputRead { node: 2 }, OutputRead { node: 3 }];
    let bytes = target::encode(&super::common::IDS, &accepted_graph, Machine::T2, &events, &outputs)
        .expect("variable-latency schedule encodes");
    assert_eq!(
        target::execute(&super::common::IDS, &accepted_graph, Machine::T2, &bytes, &[Value::I64(5), Value::I64(3)]),
        Ok(vec![Value::I64(15), Value::I64(8)]),
    );
}

#[test]
fn completion_priority_controls_operand_ages_and_lane_unit_conflicts_refuse() {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "target-belt-114-completion-priority"),
    );
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let z = builder.input(ScalarType::I64);
    let w = builder.input(ScalarType::I64);
    let left = builder.op(Op::I64Add(x, y));
    let right = builder.op(Op::I64Sub(z, w));
    let result = builder.op(Op::I64Add(left, right));
    builder.output(result);
    let accepted_graph = checked(builder);
    let events = [
        Issue { cycle: 0, lane: 0, unit: 3, operation: Operation::Input { node: 0, slot: 0 } },
        Issue { cycle: 0, lane: 1, unit: 4, operation: Operation::Input { node: 1, slot: 1 } },
        Issue { cycle: 1, lane: 0, unit: 3, operation: Operation::Input { node: 2, slot: 2 } },
        Issue { cycle: 1, lane: 1, unit: 4, operation: Operation::Input { node: 3, slot: 3 } },
        Issue { cycle: 2, lane: 0, unit: 0, operation: Operation::Add { node: 4, left_age: 3, right_age: 2 } },
        Issue { cycle: 2, lane: 1, unit: 1, operation: Operation::Sub { node: 5, left_age: 1, right_age: 0 } },
        Issue { cycle: 3, lane: 0, unit: 0, operation: Operation::Add { node: 6, left_age: 1, right_age: 0 } },
    ];
    let outputs = [OutputRead { node: 6 }];
    let bytes = target::encode(&super::common::IDS, &accepted_graph, Machine::T2, &events, &outputs)
        .expect("ordered completion schedule encodes");
    assert_eq!(
        target::execute(&super::common::IDS, &accepted_graph, Machine::T2, &bytes,
            &[Value::I64(5), Value::I64(3), Value::I64(9), Value::I64(1)]),
        Ok(vec![Value::I64(16)]),
    );

    let mut reordered = events;
    reordered[4].unit = 1;
    reordered[5].unit = 0;
    let reordered_bytes = target::encode(&super::common::IDS, &accepted_graph, Machine::T2, &reordered, &outputs)
        .expect("a changed completion order is still structurally encoded");
    assert!(matches!(
        target::check(&super::common::IDS, &accepted_graph, Machine::T2, &reordered_bytes),
        Err(Refusal::OperandMismatch)
    ));

    let mut lane_conflict = events;
    lane_conflict[5].lane = 0;
    let lane_bytes = target::encode(&super::common::IDS, &accepted_graph, Machine::T2, &lane_conflict, &outputs)
        .expect("a lane conflict is a legality failure rather than a framing error");
    assert!(matches!(
        target::check(&super::common::IDS, &accepted_graph, Machine::T2, &lane_bytes),
        Err(Refusal::IssueConflict)
    ));

    let mut unit_conflict = events;
    unit_conflict[5].unit = 0;
    let unit_bytes = target::encode(&super::common::IDS, &accepted_graph, Machine::T2, &unit_conflict, &outputs)
        .expect("a unit conflict is a legality failure rather than a framing error");
    assert!(matches!(
        target::check(&super::common::IDS, &accepted_graph, Machine::T2, &unit_bytes),
        Err(Refusal::UnitConflict)
    ));
}
