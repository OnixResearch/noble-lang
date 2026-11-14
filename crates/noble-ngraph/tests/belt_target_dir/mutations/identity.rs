use super::{fixture_graph, identity_graph, target, Machine, Refusal};

#[test]
fn all_fixed_descriptor_bytes_and_artifact_identity_are_checked() {
    let accepted_graph = fixture_graph();
    let lowered = target::lower(&super::super::common::IDS, &accepted_graph, Machine::T1)
        .expect("T1 fixture lowers");
    let descriptor_start =
        4 + 1 + target::FORMAT_REVISION.len() + 1 + 1 + target::MACHINE_REVISION.len();
    for field in 0..10 {
        let mut changed = lowered.bytes.clone();
        changed[descriptor_start + field] ^= 1;
        assert!(
            matches!(
                target::decode(&super::super::common::IDS, &changed),
                Err(Refusal::DescriptorMismatch)
            ),
            "descriptor field {field}"
        );
    }
    assert!(matches!(
        target::check(
            &super::super::common::IDS,
            &accepted_graph,
            Machine::T2,
            &lowered.bytes
        ),
        Err(Refusal::MachineMismatch)
    ));
    let different_graph = super::super::belt_target_tests::checked(identity_graph());
    assert!(matches!(
        target::check(
            &super::super::common::IDS,
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
        target::decode(&super::super::common::IDS, &changed_body),
        Err(Refusal::DigestMismatch)
    ));
}
