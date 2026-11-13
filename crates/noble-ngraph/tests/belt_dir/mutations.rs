#![allow(
    clippy::unwrap_used,
    tigerstyle::assertion_density,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    reason = "Owner: noble-maintainers; mutation tests deliberately build forged wire artifacts and pin the checker's fail-closed refusal for each forged condition."
)]

use noble_ngraph::belt::{self, Refusal};
use noble_ngraph::ir::{Fact, GraphBuilder, Op, ScalarType, SubProfile, Subject};

fn asymmetric() -> noble_ngraph::check::CheckedGraph {
    let ids = super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&ids, "belt-113-asymmetric"),
    );
    let x = builder.input(ScalarType::I64);
    let y = builder.input(ScalarType::I64);
    let result = builder.op(Op::I64Sub(y, x));
    builder.output(result);
    super::belt_fixtures::checked(builder.finish())
}

fn stream(operands: [u8; 2], operation: u8, output: bool) -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 0, 0, 0, 1, 0, 0, 0, operation];
    bytes.extend_from_slice(&operands);
    if output {
        bytes.extend_from_slice(&[8, 0]);
    }
    bytes
}

fn check_stream(
    checked: &noble_ngraph::check::CheckedGraph,
    capacity: u8,
    slots: u8,
    bytes: &[u8],
) -> Result<(), Refusal> {
    let artifact = belt::encode(
        &super::common::IDS,
        checked,
        capacity,
        slots,
        bytes,
    )?;
    belt::check(&super::common::IDS, checked, &artifact).map(|_| ())
}

#[test]
fn rejects_instruction_semantic_order_and_completion_mutations() {
    let checked = asymmetric();
    assert!(check_stream(&checked, 2, 0, &stream([0, 1], 3, true)).is_ok());
    assert_eq!(
        check_stream(&checked, 2, 0, &stream([1, 0], 3, true)).unwrap_err(),
        Refusal::OperandMismatch
    );
    assert_eq!(
        check_stream(&checked, 2, 0, &stream([0, 1], 2, true)).unwrap_err(),
        Refusal::BodyMismatch
    );
    assert_eq!(
        check_stream(&checked, 2, 0, &stream([2, 1], 3, true)).unwrap_err(),
        Refusal::StaleAge
    );
    assert_eq!(
        check_stream(&checked, 2, 0, &stream([0, 1], 3, false)).unwrap_err(),
        Refusal::OutputMismatch
    );

    let mut extra_output = stream([0, 1], 3, true);
    extra_output.extend_from_slice(&[8, 0]);
    assert_eq!(
        check_stream(&checked, 2, 0, &extra_output).unwrap_err(),
        Refusal::OutputMismatch
    );

    let mut reordered_definitions = stream([0, 1], 3, true);
    let (first_input, rest) = reordered_definitions.split_at_mut(5);
    first_input.swap_with_slice(&mut rest[..5]);
    assert_eq!(
        check_stream(&checked, 2, 0, &reordered_definitions).unwrap_err(),
        Refusal::BodyMismatch
    );

    let ids = super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&ids, "belt-113-output-order"),
    );
    let first = builder.input(ScalarType::I64);
    let second = builder.input(ScalarType::I64);
    builder.output(first);
    builder.output(second);
    let two_outputs = super::belt_fixtures::checked(builder.finish());
    let outputs = [
        0, 0, 0, 0, 0, // input 0
        0, 1, 0, 0, 0, // input 1
        8, 1, 8, 0, // graph output order
    ];
    assert!(check_stream(&two_outputs, 2, 0, &outputs).is_ok());
    let reversed_outputs = [
        0, 0, 0, 0, 0, // input 0
        0, 1, 0, 0, 0, // input 1
        8, 0, 8, 1, // wrong graph output order
    ];
    assert_eq!(
        check_stream(&two_outputs, 2, 0, &reversed_outputs).unwrap_err(),
        Refusal::OutputMismatch
    );
}

#[test]
fn rejects_invalid_machine_bounds_and_wire_data() {
    let checked = asymmetric();
    let valid = stream([0, 1], 3, true);
    assert_eq!(
        belt::encode(&super::common::IDS, &checked, 1, 0, &valid),
        Err(Refusal::Capacity)
    );
    assert_eq!(
        belt::encode(&super::common::IDS, &checked, 65, 0, &valid),
        Err(Refusal::Capacity)
    );
    assert_eq!(
        belt::encode(&super::common::IDS, &checked, 2, 9, &valid),
        Err(Refusal::SaveSlots)
    );
    let wide = belt::encode(&super::common::IDS, &checked, 64, 8, &valid)
        .expect("the largest declared belt and save table are admitted");
    assert!(belt::check(&super::common::IDS, &checked, &wide).is_ok());
    assert_eq!(
        belt::encode(&super::common::IDS, &checked, 2, 0, &[9]),
        Err(Refusal::Malformed)
    );

    let mut artifact = belt::encode(&super::common::IDS, &checked, 2, 0, &valid)
        .expect("the baseline stream encodes");
    let code_start = artifact.len() - valid.len();
    let mut changed_code = artifact.clone();
    changed_code[code_start] ^= 1;
    assert_eq!(
        belt::check(&super::common::IDS, &checked, &changed_code).unwrap_err(),
        Refusal::InstructionDigest
    );

    let capacity_offset = 4
        + 1
        + belt::FORMAT_REVISION.len()
        + 1
        + belt::LOWERER_REVISION.len()
        + 1
        + belt::CHECKER_REVISION.len()
        + 32;
    let mut invalid_capacity = artifact.clone();
    invalid_capacity[capacity_offset] = 1;
    assert_eq!(
        belt::check(&super::common::IDS, &checked, &invalid_capacity).unwrap_err(),
        Refusal::Capacity
    );
    artifact[capacity_offset] = 3;
    assert_eq!(
        belt::check(&super::common::IDS, &checked, &artifact).unwrap_err(),
        Refusal::ArtifactBinding
    );
}

#[test]
fn rejects_wrong_graph_identity_and_live_value_loss() {
    let checked = asymmetric();
    let other = {
        let ids = super::common::IDS;
        let mut builder = GraphBuilder::new(
            SubProfile::I64,
            Subject::authored(&ids, "belt-113-different-identity"),
        );
        let x = builder.input(ScalarType::I64);
        let y = builder.input(ScalarType::I64);
        let result = builder.op(Op::I64Sub(y, x));
        builder.output(result);
        super::belt_fixtures::checked(builder.finish())
    };
    let artifact = belt::encode(
        &super::common::IDS,
        &checked,
        2,
        0,
        &stream([0, 1], 3, true),
    )
    .expect("the artifact binds its source graph");
    assert_eq!(
        belt::check(&super::common::IDS, &other, &artifact).unwrap_err(),
        Refusal::GraphIdentity
    );

    let ids = super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&ids, "belt-113-live-eviction"),
    );
    let x = builder.input(ScalarType::I64);
    let _y = builder.input(ScalarType::I64);
    let z = builder.input(ScalarType::I64);
    let result = builder.op(Op::I64Add(x, z));
    builder.output(result);
    let live = super::belt_fixtures::checked(builder.finish());
    assert_eq!(
        check_stream(&live, 2, 0, &[0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 2, 0, 0, 0])
            .unwrap_err(),
        Refusal::LiveValueLost
    );
}

#[test]
fn rejects_uninitialized_and_live_overwritten_slots() {
    let checked = asymmetric();
    assert_eq!(
        check_stream(&checked, 2, 1, &[0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 7, 0])
            .unwrap_err(),
        Refusal::UninitializedSlot
    );
    assert_eq!(
        check_stream(&checked, 2, 0, &[0, 0, 0, 0, 0, 6, 0, 1])
            .unwrap_err(),
        Refusal::SlotBound
    );

    let ids = super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&ids, "belt-113-slot-overwrite"),
    );
    let x = builder.input(ScalarType::I64);
    let _y = builder.input(ScalarType::I64);
    let z = builder.input(ScalarType::I64);
    let result = builder.op(Op::I64Add(x, z));
    builder.output(result);
    let live = super::belt_fixtures::checked(builder.finish());
    let overwritten = [
        0, 0, 0, 0, 0, // input 0
        0, 1, 0, 0, 0, // input 1
        6, 1, 0, // save x to slot 0
        0, 2, 0, 0, 0, // input 2 evicts x, which is saved
        6, 0, 0, // overwrite x with z
    ];
    assert_eq!(
        check_stream(&live, 2, 1, &overwritten).unwrap_err(),
        Refusal::LiveSlotOverwrite
    );
}

#[test]
fn rejects_a_rebound_constant_mutation() {
    let checked = super::belt_fixtures::checked(super::belt_fixtures::graph());
    let lowered = belt::lower(&super::common::IDS, &checked, 2, 8)
        .expect("the exact-constant fixture lowers");
    let view = belt::decode(&super::common::IDS, &lowered.bytes)
        .expect("fresh artifact decodes");
    let mut body = view.instruction_bytes().to_vec();
    let minimum = i64::MIN.to_le_bytes();
    let constant_low_byte = body
        .windows(minimum.len())
        .position(|bytes| bytes == minimum)
        .expect("the lowered artifact contains the minimum constant payload");
    body[constant_low_byte] ^= 1;
    let rebound = belt::encode(
        &super::common::IDS,
        &checked,
        view.capacity(),
        view.save_slots(),
        &body,
    )
    .expect("the semantically forged stream still has a valid structural encoding");
    assert_eq!(
        belt::check(&super::common::IDS, &checked, &rebound).unwrap_err(),
        Refusal::BodyMismatch
    );
}

#[test]
fn refuses_facts_f32_and_parameters_before_lowering() {
    let ids = super::common::IDS;
    let mut guarded_builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&ids, "belt-113-guarded"),
    );
    let input = guarded_builder.input(ScalarType::I64);
    let three = guarded_builder.i64_const(3);
    let guarded_value = guarded_builder.op(Op::I64Add(input, three));
    guarded_builder.guard(Fact::Bit, guarded_value);
    guarded_builder.output(guarded_value);
    let guarded = super::belt_fixtures::checked(guarded_builder.finish());
    assert_eq!(
        belt::lower(&ids, &guarded, 8, 8),
        Err(Refusal::UnsupportedGraph)
    );

    let mut f32_builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&ids, "belt-113-f32"),
    );
    let f32_value = f32_builder.param();
    f32_builder.output(f32_value);
    let f32_graph = super::belt_fixtures::checked(f32_builder.finish());
    assert_eq!(
        belt::lower(&ids, &f32_graph, 8, 8),
        Err(Refusal::UnsupportedGraph)
    );
}
