use super::{assert_refusal, fixture_graph, target, Machine, OutputRead, Refusal};
use noble_ngraph::belt::target::Operation;

struct Fixture {
    checked: noble_ngraph::CheckedGraph,
    events: Vec<target::Issue>,
    outputs: Vec<OutputRead>,
}

fn fixture() -> Fixture {
    let checked = fixture_graph();
    let lowered = target::lower(&super::super::common::IDS, &checked, Machine::T1)
        .expect("T1 fixture lowers");
    let decoded =
        target::decode(&super::super::common::IDS, &lowered.bytes).expect("fresh artifact decodes");
    Fixture {
        checked,
        events: decoded.events().to_vec(),
        outputs: decoded.outputs().to_vec(),
    }
}

#[test]
fn issue_class_lane_node_age_and_uniqueness_mutations_refuse() {
    let fixture = fixture();

    let mut wrong_class = fixture.events.clone();
    wrong_class[0].unit = 0;
    assert_refusal(
        &fixture.checked,
        Machine::T1,
        &wrong_class,
        &fixture.outputs,
        Refusal::UnitClass,
    );

    let mut wrong_lane = fixture.events.clone();
    wrong_lane[0].lane = 1;
    assert_refusal(
        &fixture.checked,
        Machine::T1,
        &wrong_lane,
        &fixture.outputs,
        Refusal::IssueLane,
    );

    let mut wrong_node = fixture.events.clone();
    wrong_node[0].operation = Operation::Input { node: 1, slot: 0 };
    assert_refusal(
        &fixture.checked,
        Machine::T1,
        &wrong_node,
        &fixture.outputs,
        Refusal::NodeMismatch,
    );

    let add = fixture
        .events
        .iter()
        .position(|event| matches!(event.operation, Operation::Add { .. }))
        .expect("the fixture has an addition");
    let mut wrong_age = fixture.events.clone();
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
    assert_refusal(
        &fixture.checked,
        Machine::T1,
        &wrong_age,
        &fixture.outputs,
        Refusal::OperandMismatch,
    );

    let mut duplicate = fixture.events.clone();
    duplicate[1].operation = Operation::Input { node: 0, slot: 0 };
    assert_refusal(
        &fixture.checked,
        Machine::T1,
        &duplicate,
        &fixture.outputs,
        Refusal::DuplicateNode,
    );
}

#[test]
fn missing_node_output_order_and_horizon_mutations_refuse() {
    let fixture = fixture();

    let mut missing = fixture.events.clone();
    let multiply = missing
        .iter()
        .position(|event| matches!(event.operation, Operation::Multiply { .. }))
        .expect("the fixture has a multiplication");
    missing.remove(multiply);
    assert_refusal(
        &fixture.checked,
        Machine::T1,
        &missing,
        &fixture.outputs,
        Refusal::MissingNode,
    );

    let mut reordered_outputs = fixture.outputs.clone();
    reordered_outputs.reverse();
    assert_refusal(
        &fixture.checked,
        Machine::T1,
        &fixture.events,
        &reordered_outputs,
        Refusal::OutputMismatch,
    );

    let mut outside_horizon = fixture.events;
    outside_horizon[0].cycle = target::MAX_HORIZON;
    assert_refusal(
        &fixture.checked,
        Machine::T1,
        &outside_horizon,
        &fixture.outputs,
        Refusal::HorizonBound,
    );
}
