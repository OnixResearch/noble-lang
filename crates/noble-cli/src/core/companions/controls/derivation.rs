pub(super) fn family(state: &mut super::Harness) {
    let (_, family, baseline) = super::checker::checked(
        &mut state.core,
        &mut state.checker,
        super::FAMILY,
        super::FAMILY_PROOF,
    );
    captures(state, &family, &baseline);
    rejected_instances(
        &mut state.core,
        family.evidence.expect("family evidence"),
        &state.observed,
    );
    shifted_family(state);
}

fn captures(
    state: &mut super::Harness,
    family: &noble_contracts::companion::Admission,
    baseline: &crate::workflow::encoding::Json,
) {
    let family_id = family.evidence.expect("family evidence");
    let seven = state
        .core
        .instantiate(family_id, 7, &super::increment_subject(7))
        .expect("real capture instantiates");
    let negative = state
        .core
        .instantiate(family_id, -1, &super::increment_subject(-1))
        .expect("negative capture instantiates");
    assert_eq!(
        state.core.claim(seven.evidence),
        Ok(noble_contracts::companion::ClaimTemplate::IncrementBy(7))
    );
    assert_eq!(
        state.core.claim(negative.evidence),
        Ok(noble_contracts::companion::ClaimTemplate::IncrementBy(-1))
    );
    assert_ne!(seven.contract, family.contract.expect("family descriptor"));
    super::control(
        &mut state.controls,
        ("CONTRACT-02", "captured-value"),
        crate::workflow::encoding::object([
            (
                "original_capture",
                crate::workflow::encoding::Json::Number(7),
            ),
            (
                "changed_capture",
                crate::workflow::encoding::Json::Number(8),
            ),
            (
                "original_instance_evidence",
                crate::workflow::encoding::Json::Number(u64::from(seven.evidence.0)),
            ),
        ]),
        state
            .core
            .bind(seven.evidence, &super::increment_subject(8))
            .map(|_| ()),
        noble_contracts::companion::Refusal::MismatchedSubject,
        baseline,
    );
    let eight = state
        .core
        .instantiate(family_id, 8, &super::increment_subject(8))
        .expect("changed capture explicitly revalidated");
    super::annotate_last(
        &mut state.controls,
        "revalidated",
        crate::workflow::encoding::Json::Bool(
            state
                .core
                .bind(eight.evidence, &super::increment_subject(8))
                .is_ok(),
        ),
    );
}

fn rejected_instances(
    core: &mut noble_contracts::companion::Core,
    family: noble_contracts::companion::EvidenceId,
    observed: &noble_contracts::companion::Subject,
) {
    let wrong = [
        noble_contracts::companion::observe(
            &[(1, 7), (2, 5)],
            noble_contracts::companion::InterfaceSignatures {
                input: observed.input_signature,
                output: observed.output_signature,
            },
        ),
        noble_contracts::companion::observe(
            &[(1, 7), (2, 4), (1, 9)],
            noble_contracts::companion::InterfaceSignatures {
                input: observed.input_signature,
                output: observed.output_signature,
            },
        ),
    ];
    let mut index = 0;
    while index < wrong.len() {
        assert_eq!(
            core.instantiate(family, 7, &wrong[index]),
            Err(noble_contracts::companion::Refusal::WrongInstantiation)
        );
        index += 1;
    }
    let mut forged_capture = super::increment_subject(7);
    forged_capture.captures[0].value = 8;
    assert_eq!(
        core.instantiate(family, 8, &forged_capture),
        Err(noble_contracts::companion::Refusal::WrongInstantiation)
    );
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; shifted_family checks a real altered family theorem through checked and asserts the single distinct WrongInstantiation outcome; acceptance and bindability assertions remain in the independent checker helper."
)]
fn shifted_family(state: &mut super::Harness) {
    // A theorem about x+1 is not the exact supported forall-x family template.
    let shifted_family = std::str::from_utf8(super::FAMILY).expect("source").replace(
        "(param x) (add (param x) (in n))",
        "(add (param x) 1) (add (add (param x) 1) (in n))",
    );
    let shifted_proof = std::str::from_utf8(super::FAMILY_PROOF)
        .expect("proof")
        .replace(
            "(.param 0) (.add (.param 0) (.input 0))",
            "(.add (.param 0) (.i64 1)) (.add (.add (.param 0) (.i64 1)) (.input 0))",
        );
    let (_, shifted, _) = super::checker::checked(
        &mut state.core,
        &mut state.checker,
        shifted_family.as_bytes(),
        shifted_proof.as_bytes(),
    );
    assert_eq!(
        state.core.instantiate(
            shifted.evidence.expect("shifted theorem"),
            7,
            &super::increment_subject(7)
        ),
        Err(noble_contracts::companion::Refusal::WrongInstantiation)
    );
}
