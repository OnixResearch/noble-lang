#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; exercise independently checks the second premise and submits one concrete composition; the bounded mismatch, conclusion, projection and premise helpers assert all resulting Core behavior."
)]
pub(super) fn exercise(state: &mut super::Harness) {
    // Serialized replay recomputes +2 and retains a concrete conclusion descriptor.
    let (_, second, _) = super::checker::checked(
        &mut state.core,
        &mut state.checker,
        super::INCREMENT,
        super::PROOF,
    );
    let right = second.evidence.expect("second proof");
    let composed = noble_contracts::companion::observe(
        &[(1, 1), (2, 4), (1, 1), (2, 4)],
        noble_contracts::companion::InterfaceSignatures {
            input: state.observed.input_signature,
            output: state.observed.output_signature,
        },
    );
    let mut replay = super::derivation(
        noble_contracts::companion::RuleId::ComposeV1,
        &state.accepted,
        &composed,
        vec![state.evidence, right],
    );
    replay.statement = noble_contracts::companion::ClaimTemplate::IncrementBy(2).digest();
    mismatched_replays(&mut state.core, &replay);
    let composition = state
        .core
        .replay(&replay)
        .expect("exact serialized composition");
    conclusion(state, &composed, composition);
    projections(state, &composed);
    premise_multiplicity(state);
}

fn mismatched_replays(
    core: &mut noble_contracts::companion::Core,
    replay: &noble_contracts::companion::Derivation,
) {
    let mut false_claim = replay.clone();
    false_claim.statement = noble_contracts::companion::ClaimTemplate::IncrementBy(99).digest();
    assert_eq!(
        core.replay(&false_claim),
        Err(noble_contracts::companion::Refusal::MismatchedClaim)
    );
    let mut unrelated = replay.clone();
    unrelated.subject = super::increment_subject(99);
    assert_eq!(
        core.replay(&unrelated),
        Err(noble_contracts::companion::Refusal::MismatchedSubject)
    );
}

fn conclusion(
    state: &mut super::Harness,
    composed: &noble_contracts::companion::Subject,
    composition: noble_contracts::companion::EvidenceId,
) {
    assert_eq!(
        state.core.claim(composition),
        Ok(noble_contracts::companion::ClaimTemplate::IncrementBy(2))
    );
    assert_eq!(
        state.core.bind(composition, &super::increment_subject(99)),
        Err(noble_contracts::companion::Refusal::MismatchedSubject)
    );
    assert_eq!(
        state.core.evidence_guard_templates(composition),
        Err(noble_contracts::companion::Refusal::UnsupportedGuardTemplate)
    );
    let released = state
        .core
        .release(composition)
        .expect("derived claim release");
    assert_ne!(
        released.contract,
        state.accepted.contract.expect("original descriptor")
    );
    assert_eq!(
        state.core.contract_program(released.contract),
        Ok(composed.events.as_slice())
    );
    let triple = noble_contracts::companion::observe(
        &[(1, 1), (2, 4), (1, 1), (2, 4), (1, 1), (2, 4)],
        noble_contracts::companion::InterfaceSignatures {
            input: state.observed.input_signature,
            output: state.observed.output_signature,
        },
    );
    let third = state
        .core
        .derive_compose(composition, state.evidence, &triple)
        .expect("transitive composition uses derived program, not original");
    assert_eq!(
        state.core.claim(third.evidence),
        Ok(noble_contracts::companion::ClaimTemplate::IncrementBy(3))
    );
}

fn projections(state: &mut super::Harness, composed: &noble_contracts::companion::Subject) {
    let rules = [
        noble_contracts::companion::RuleId::AdmitLeanV1,
        noble_contracts::companion::RuleId::ProjectV1,
        noble_contracts::companion::RuleId::InvokeV1,
    ];
    let mut index = 0;
    while index < rules.len() {
        let theft = super::derivation(
            rules[index],
            &state.accepted,
            &super::increment_subject(99),
            vec![state.evidence],
        );
        assert_eq!(
            state.core.replay(&theft),
            Err(noble_contracts::companion::Refusal::MismatchedSubject)
        );
        index += 1;
    }
    let mut forged_identity = composed.clone();
    forged_identity.identity = state.bound.identity;
    assert_eq!(
        state.core.bind(state.evidence, &forged_identity),
        Err(noble_contracts::companion::Refusal::MismatchedSubject)
    );
    let mut forged_fields = state.bound.clone();
    forged_fields.output_signature ^= 1;
    assert_eq!(
        state.core.bind(state.evidence, &forged_fields),
        Err(noble_contracts::companion::Refusal::MismatchedSubject)
    );
}

fn premise_multiplicity(state: &mut super::Harness) {
    let mut duplicate = super::derivation(
        noble_contracts::companion::RuleId::ProjectV1,
        &state.accepted,
        &state.bound,
        vec![state.evidence, state.evidence],
    );
    assert_eq!(
        state.core.replay(&duplicate),
        Err(noble_contracts::companion::Refusal::DuplicatePremise)
    );
    duplicate.premises.clear();
    assert_eq!(
        state.core.replay(&duplicate),
        Err(noble_contracts::companion::Refusal::MissingPremise)
    );
}
