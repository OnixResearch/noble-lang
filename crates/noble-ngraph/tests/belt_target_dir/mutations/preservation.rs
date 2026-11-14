use super::{assert_refusal, pressure_graph, target, Machine, Refusal};
use noble_ngraph::belt::target::Operation;
use noble_ngraph::ir::{GraphBuilder, SubProfile, Subject};
use noble_ngraph::sem::Value;

#[test]
fn save_and_load_mutations_refuse() {
    let accepted_graph = super::super::belt_target_tests::checked(pressure_graph());
    let lowered = target::lower(&super::super::common::IDS, &accepted_graph, Machine::T1)
        .expect("the pressure fixture lowers");
    assert!(lowered.preservation_saves > 0);
    assert!(lowered.preservation_loads > 0);
    assert_eq!(
        target::execute(
            &super::super::common::IDS,
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
        target::decode(&super::super::common::IDS, &lowered.bytes).expect("fresh artifact decodes");
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
    assert_refusal(
        &accepted_graph,
        Machine::T1,
        &bad_save,
        &outputs,
        Refusal::SaveSlotBound,
    );

    let load = events
        .iter()
        .position(|event| matches!(event.operation, Operation::Load { .. }))
        .expect("pressure fixture reloads a preserved value");
    let mut bad_load = events;
    if let Operation::Load { node, .. } = bad_load[load].operation {
        bad_load[load].operation = Operation::Load { node, slot: 7 };
    }
    assert_refusal(
        &accepted_graph,
        Machine::T1,
        &bad_load,
        &outputs,
        Refusal::UninitializedSave,
    );
}

#[test]
fn output_capacity_is_checked_per_machine_profile() {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(
            &super::super::common::IDS,
            "target-belt-114-too-many-outputs",
        ),
    );
    for value in [2, 3, 4, 5, 6] {
        let node = builder.i64_const(value);
        builder.output(node);
    }
    let too_many_outputs = super::super::belt_target_tests::checked(builder.finish());
    assert!(matches!(
        target::lower(&super::super::common::IDS, &too_many_outputs, Machine::T1),
        Err(Refusal::OutputMismatch)
    ));
    let t2 = target::lower(&super::super::common::IDS, &too_many_outputs, Machine::T2)
        .expect("five outputs fit T2");
    assert_eq!(
        target::execute(
            &super::super::common::IDS,
            &too_many_outputs,
            Machine::T2,
            &t2.bytes,
            &[]
        ),
        Ok([2, 3, 4, 5, 6].map(Value::I64).to_vec())
    );
}
