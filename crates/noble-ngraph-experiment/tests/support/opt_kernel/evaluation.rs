use noble_ngraph_experiment::opt::{
    classify_bits, Candidate, NonFinite, RefusalClass, StepDescriptor, TransitionInput,
};

#[test]
fn nonfinite_classification_preserves_finite_bit_patterns() {
    assert_eq!(classify_bits(0x7f80_0000), Some(NonFinite::Infinity));
    assert_eq!(classify_bits(0xff80_0000), Some(NonFinite::Infinity));
    assert_eq!(classify_bits(0x7fc0_0000), Some(NonFinite::NaN));
    assert_eq!(classify_bits(0x8000_0000), None);
    assert_eq!(classify_bits(0x0000_0001), None);
}

#[test]
fn runtime_shape_and_state_controls_fail_closed() {
    let candidate =
        Candidate::parse(super::SGD_ENVELOPE.as_bytes()).expect("fixed candidate admits");

    let short = TransitionInput {
        parameters: &[0x3f80_0000],
        gradients: &[0x3f80_0000, 0x3f80_0000],
        eta: 0x3f00_0000,
        step: StepDescriptor::new(1, 0, 0).expect("descriptor"),
        state: &[],
    };
    let refusal = candidate
        .evaluate(&short)
        .expect_err("shape mismatch refuses");
    assert_eq!(refusal.class, RefusalClass::Unsupported);
    assert_eq!(refusal.location, "$.inputs.p");

    let state = TransitionInput {
        parameters: &[0x3f80_0000, 0x4000_0000],
        gradients: &[0x3f80_0000, 0x3f80_0000],
        eta: 0x3f00_0000,
        step: StepDescriptor::new(1, 0, 0).expect("descriptor"),
        state: &[1],
    };
    let refusal = candidate
        .evaluate(&state)
        .expect_err("nonempty state refuses");
    assert_eq!(refusal.class, RefusalClass::Exhausted);
    assert_eq!(refusal.location, "$.state");
}

#[test]
fn step_descriptor_version_is_fixed() {
    let refusal = StepDescriptor::new(2, 0, 0).expect_err("descriptor version refuses");
    assert_eq!(refusal.class, RefusalClass::Malformed);
    assert_eq!(refusal.location, "$.inputs.step.version");
}

#[test]
fn step_descriptor_values_do_not_change_the_candidate_transition() {
    let candidate =
        Candidate::parse(super::SGD_ENVELOPE.as_bytes()).expect("fixed candidate admits");
    let parameters = [0x3f80_0000, 0x4000_0000];
    let gradients = [0x3f80_0000, 0xc000_0000];
    let first = TransitionInput {
        parameters: &parameters,
        gradients: &gradients,
        eta: 0x3f00_0000,
        step: StepDescriptor::new(1, 0, 0).expect("first descriptor"),
        state: &[],
    };
    let later = TransitionInput {
        parameters: &parameters,
        gradients: &gradients,
        eta: 0x3f00_0000,
        step: StepDescriptor::new(1, 17, 9).expect("later descriptor"),
        state: &[],
    };
    assert_ne!(first.step, later.step, "the host metadata actually changes");
    assert_eq!(candidate.evaluate(&first), candidate.evaluate(&later));
}
