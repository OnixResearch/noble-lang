use noble_ngraph::belt::target::{self, Machine, Operation, OutputRead, Refusal};
use noble_ngraph::ir::{Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::sem::Value;

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

#[test]
fn all_fixed_descriptor_bytes_and_artifact_identity_are_checked() {
    let accepted_graph = super::belt_target_tests::checked(super::belt_target_tests::graph());
    let lowered = target::lower(&super::common::IDS, &accepted_graph, Machine::T1)
        .expect("T1 fixture lowers");
    let descriptor_start =
        4 + 1 + target::FORMAT_REVISION.len() + 1 + 1 + target::MACHINE_REVISION.len();
    for field in 0..10 {
        let mut changed = lowered.bytes.clone();
        changed[descriptor_start + field] ^= 1;
        assert!(
            matches!(
                target::decode(&super::common::IDS, &changed),
                Err(Refusal::DescriptorMismatch)
            ),
            "descriptor field {field}"
        );
    }
    assert!(matches!(
        target::check(
            &super::common::IDS,
            &accepted_graph,
            Machine::T2,
            &lowered.bytes
        ),
        Err(Refusal::MachineMismatch)
    ));
    let different_graph = super::belt_target_tests::checked(identity_graph());
    assert!(matches!(
        target::check(
            &super::common::IDS,
            &different_graph,
            Machine::T1,
            &lowered.bytes
        ),
        Err(Refusal::GraphIdentity)
    ));

    let mut changed_body = lowered.bytes.clone();
    let last = changed_body.len() - 1;
    changed_body[last] ^= 1;
    assert!(matches!(
        target::decode(&super::common::IDS, &changed_body),
        Err(Refusal::DigestMismatch)
    ));
}

#[test]
fn reencoded_legality_mutations_refuse_at_the_changed_premise() {
    let accepted_graph = super::belt_target_tests::checked(super::belt_target_tests::graph());
    let lowered = target::lower(&super::common::IDS, &accepted_graph, Machine::T1)
        .expect("T1 fixture lowers");
    let decoded =
        target::decode(&super::common::IDS, &lowered.bytes).expect("fresh artifact decodes");
    let events = decoded.events().to_vec();
    let outputs = decoded.outputs().to_vec();

    let mut wrong_class = events.clone();
    wrong_class[0].unit = 0;
    assert!(matches!(
        target::check(
            &super::common::IDS,
            &accepted_graph,
            Machine::T1,
            &encode(&accepted_graph, Machine::T1, &wrong_class, &outputs)
        ),
        Err(Refusal::UnitClass)
    ));

    let mut wrong_lane = events.clone();
    wrong_lane[0].lane = 1;
    assert!(matches!(
        target::check(
            &super::common::IDS,
            &accepted_graph,
            Machine::T1,
            &encode(&accepted_graph, Machine::T1, &wrong_lane, &outputs)
        ),
        Err(Refusal::IssueLane)
    ));

    let mut wrong_node = events.clone();
    wrong_node[0].operation = Operation::Input { node: 1, slot: 0 };
    assert!(matches!(
        target::check(
            &super::common::IDS,
            &accepted_graph,
            Machine::T1,
            &encode(&accepted_graph, Machine::T1, &wrong_node, &outputs)
        ),
        Err(Refusal::NodeMismatch)
    ));

    let add = events
        .iter()
        .position(|event| matches!(event.operation, Operation::Add { .. }))
        .expect("the fixture has an addition");
    let mut wrong_age = events.clone();
    if let Operation::Add {
        node,
        left_age,
        right_age,
    } = wrong_age[add].operation
    {
        wrong_age[add].operation = Operation::Add {
            node,
            left_age: right_age,
            right_age: left_age,
        };
    }
    assert!(matches!(
        target::check(
            &super::common::IDS,
            &accepted_graph,
            Machine::T1,
            &encode(&accepted_graph, Machine::T1, &wrong_age, &outputs)
        ),
        Err(Refusal::OperandMismatch)
    ));

    let mut duplicate = events.clone();
    duplicate[1].operation = Operation::Input { node: 0, slot: 0 };
    assert!(matches!(
        target::check(
            &super::common::IDS,
            &accepted_graph,
            Machine::T1,
            &encode(&accepted_graph, Machine::T1, &duplicate, &outputs)
        ),
        Err(Refusal::DuplicateNode)
    ));

    let mut missing = events.clone();
    let multiply = missing
        .iter()
        .position(|event| matches!(event.operation, Operation::Multiply { .. }))
        .expect("the fixture has a multiplication");
    missing.remove(multiply);
    assert!(matches!(
        target::check(
            &super::common::IDS,
            &accepted_graph,
            Machine::T1,
            &encode(&accepted_graph, Machine::T1, &missing, &outputs)
        ),
        Err(Refusal::MissingNode)
    ));

    let mut reordered_outputs = outputs.clone();
    reordered_outputs.reverse();
    assert!(matches!(
        target::check(
            &super::common::IDS,
            &accepted_graph,
            Machine::T1,
            &encode(&accepted_graph, Machine::T1, &events, &reordered_outputs)
        ),
        Err(Refusal::OutputMismatch)
    ));

    let mut outside_horizon = events;
    outside_horizon[0].cycle = target::MAX_HORIZON;
    assert!(matches!(
        target::check(
            &super::common::IDS,
            &accepted_graph,
            Machine::T1,
            &encode(&accepted_graph, Machine::T1, &outside_horizon, &outputs)
        ),
        Err(Refusal::HorizonBound)
    ));
}

#[test]
fn save_load_and_output_capacity_mutations_are_rejected() {
    let accepted_graph = super::belt_target_tests::checked(pressure_graph());
    let lowered = target::lower(&super::common::IDS, &accepted_graph, Machine::T1)
        .expect("the pressure fixture lowers");
    assert!(lowered.preservation_saves > 0);
    assert!(lowered.preservation_loads > 0);
    assert_eq!(
        target::execute(
            &super::common::IDS,
            &accepted_graph,
            Machine::T1,
            &lowered.bytes,
            &[Value::I64(17), Value::I64(25)]
        ),
        Ok(vec![
            Value::I64(2),
            Value::I64(3),
            Value::I64(4),
            Value::I64(47)
        ])
    );
    let decoded =
        target::decode(&super::common::IDS, &lowered.bytes).expect("fresh artifact decodes");
    let outputs = decoded.outputs().to_vec();
    let events = decoded.events().to_vec();

    let save = events
        .iter()
        .position(|event| matches!(event.operation, Operation::Save { .. }))
        .expect("pressure fixture saves a live value");
    let mut bad_save = events.clone();
    if let Operation::Save { node, age, .. } = bad_save[save].operation {
        bad_save[save].operation = Operation::Save { node, age, slot: 8 };
    }
    assert!(matches!(
        target::check(
            &super::common::IDS,
            &accepted_graph,
            Machine::T1,
            &encode(&accepted_graph, Machine::T1, &bad_save, &outputs)
        ),
        Err(Refusal::SaveSlotBound)
    ));

    let load = events
        .iter()
        .position(|event| matches!(event.operation, Operation::Load { .. }))
        .expect("pressure fixture reloads a preserved value");
    let mut bad_load = events;
    if let Operation::Load { node, .. } = bad_load[load].operation {
        bad_load[load].operation = Operation::Load { node, slot: 7 };
    }
    assert!(matches!(
        target::check(
            &super::common::IDS,
            &accepted_graph,
            Machine::T1,
            &encode(&accepted_graph, Machine::T1, &bad_load, &outputs)
        ),
        Err(Refusal::UninitializedSave)
    ));

    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "target-belt-114-too-many-outputs"),
    );
    for value in [2, 3, 4, 5, 6] {
        let node = builder.i64_const(value);
        builder.output(node);
    }
    let too_many_outputs = super::belt_target_tests::checked(builder.finish());
    assert!(matches!(
        target::lower(&super::common::IDS, &too_many_outputs, Machine::T1),
        Err(Refusal::OutputMismatch)
    ));
    let t2 = target::lower(&super::common::IDS, &too_many_outputs, Machine::T2)
        .expect("five outputs fit T2");
    assert_eq!(
        target::execute(
            &super::common::IDS,
            &too_many_outputs,
            Machine::T2,
            &t2.bytes,
            &[]
        ),
        Ok([2, 3, 4, 5, 6].map(Value::I64).to_vec())
    );
}

#[test]
fn producer_and_checker_exhaustion_are_distinct_from_semantic_refusal() {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "target-belt-114-producer-exhaustion"),
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
    let accepted_graph = super::belt_target_tests::checked(builder.finish());
    assert!(matches!(
        target::lower(&super::common::IDS, &accepted_graph, Machine::T1),
        Err(Refusal::SchedulingExhausted)
    ));

    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "target-belt-114-checker-exhaustion"),
    );
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let sum = builder.op(Op::I64Add(x, y));
    builder.output(sum);
    let checked = super::belt_target_tests::checked(builder.finish());
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
    let bytes = encode(&checked, Machine::T2, &events, &[OutputRead { node: 2 }]);
    assert!(matches!(
        target::check(&super::common::IDS, &checked, Machine::T2, &bytes),
        Err(Refusal::CheckingExhausted)
    ));
}
