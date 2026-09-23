pub(super) fn forgery(state: &mut super::Harness) {
    serialized_status(state);
    unknown_tags(state);
    premises(state);
    wrong_family(state);
    replay_budget(state);
}

fn serialized_status(state: &mut super::Harness) {
    let forged = format!(
        "-- noble-evidence v1 statement {}\n{{\"certified\":true,\"construction\":\"Certified\"}}",
        state.accepted.statement_digest
    );
    let result = state
        .core
        .admit(&state.increment, super::offer(forged.as_bytes()));
    super::control(
        &mut state.controls,
        ("CONTRACT-06", "serialized-certified-true"),
        crate::workflow::encoding::object([(
            "payload",
            crate::workflow::encoding::string(&forged),
        )]),
        super::admission_result(&result),
        noble_contracts::companion::Refusal::ForgedStatus,
        &state.baseline,
    );
    let checked_forgery = state.checker.admit(
        &mut state.core,
        &state.increment,
        super::offer(forged.as_bytes()),
    );
    assert_eq!(checked_forgery.evidence, None);
    assert_eq!(
        checked_forgery.refusals,
        [noble_contracts::companion::Refusal::ForgedStatus]
    );
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; unknown_tags decodes two fixed invalid tags and delegates exact UnsupportedEvidenceClass and UnknownRule assertions to control; repeating them here would not test another transition."
)]
fn unknown_tags(state: &mut super::Harness) {
    super::control(
        &mut state.controls,
        ("CONTRACT-06", "forged-constructor-tag"),
        crate::workflow::encoding::object([(
            "evidence_class_tag",
            crate::workflow::encoding::Json::Number(u64::from(u32::MAX)),
        )]),
        noble_contracts::companion::EvidenceClass::decode(u32::MAX).map(|_| ()),
        noble_contracts::companion::Refusal::UnsupportedEvidenceClass,
        &state.baseline,
    );
    super::control(
        &mut state.controls,
        ("CONTRACT-06", "unknown-rule"),
        crate::workflow::encoding::object([
            ("ruleset", crate::workflow::encoding::Json::Number(1)),
            ("rule", crate::workflow::encoding::Json::Number(99)),
        ]),
        noble_contracts::companion::RuleId::decode(noble_contracts::companion::EncodedRule {
            ruleset: 1,
            code: 99,
        })
        .map(|_| ()),
        noble_contracts::companion::Refusal::UnknownRule,
        &state.baseline,
    );
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; premises constructs two bounded adversarial references with checked index arithmetic, and control asserts each real replay refusal against CyclicDerivation or MissingPremise."
)]
fn premises(state: &mut super::Harness) {
    let self_id = noble_contracts::companion::EvidenceId(
        state.evidence.0.checked_add(1).expect("next evidence id"),
    );
    let cyclic = super::derivation(
        noble_contracts::companion::RuleId::AdmitLeanV1,
        &state.accepted,
        &state.bound,
        vec![self_id],
    );
    super::control(
        &mut state.controls,
        ("CONTRACT-06", "cyclic-derivation"),
        crate::workflow::encoding::object([
            (
                "new_evidence_id",
                crate::workflow::encoding::Json::Number(u64::from(self_id.0)),
            ),
            ("premises", super::premises_json(&[self_id])),
        ]),
        state.core.replay(&cyclic).map(|_| ()),
        noble_contracts::companion::Refusal::CyclicDerivation,
        &state.baseline,
    );
    let missing = noble_contracts::companion::EvidenceId(
        state
            .evidence
            .0
            .checked_add(50)
            .expect("absent evidence id"),
    );
    let absent = super::derivation(
        noble_contracts::companion::RuleId::ComposeV1,
        &state.accepted,
        &state.bound,
        vec![missing],
    );
    super::control(
        &mut state.controls,
        ("CONTRACT-06", "missing-premise"),
        crate::workflow::encoding::object([("premises", super::premises_json(&[missing]))]),
        state.core.replay(&absent).map(|_| ()),
        noble_contracts::companion::Refusal::MissingPremise,
        &state.baseline,
    );
}

fn wrong_family(state: &mut super::Harness) {
    super::control(
        &mut state.controls,
        ("CONTRACT-06", "wrong-family-instantiation"),
        crate::workflow::encoding::object([
            (
                "family_evidence",
                crate::workflow::encoding::Json::Number(u64::from(state.evidence.0)),
            ),
            ("capture", crate::workflow::encoding::Json::Number(7)),
            ("subject_ops", super::events_json(&[(1, 7), (2, 4)])),
        ]),
        state
            .core
            .instantiate(state.evidence, 7, &super::increment_subject(7))
            .map(|_| ()),
        noble_contracts::companion::Refusal::WrongInstantiation,
        &state.baseline,
    );
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; replay_budget admits a genuine independent premise under 1900 work, appends exactly 1900 events, and delegates the resulting ExhaustedReplay assertion to control without fabricating acceptance."
)]
fn replay_budget(state: &mut super::Harness) {
    let mut metered = noble_contracts::companion::Core::new(noble_contracts::Limits {
        work: 1900,
        ..super::LIMITS
    });
    let (contract, admission, baseline) = super::checker::checked(
        &mut metered,
        &mut state.checker,
        super::INCREMENT,
        super::PROOF,
    );
    let mut long = super::retained_subject(&metered, &admission, &contract).events;
    long.extend(std::iter::repeat_n((8, 0), 1900));
    let oversized = noble_contracts::companion::observe(
        &long,
        noble_contracts::companion::InterfaceSignatures {
            input: state.observed.input_signature,
            output: state.observed.output_signature,
        },
    );
    let exhaustion = super::derivation(
        noble_contracts::companion::RuleId::ProjectV1,
        &admission,
        &oversized,
        vec![admission.evidence.expect("evidence")],
    );
    super::control(
        &mut state.controls,
        ("CONTRACT-06", "exhausted-replay-budget"),
        crate::workflow::encoding::object([
            ("work_budget", crate::workflow::encoding::Json::Number(1900)),
            (
                "observation_events",
                crate::workflow::encoding::Json::Number(
                    u64::try_from(long.len()).expect("observation event count"),
                ),
            ),
            ("premises", super::premises_json(&exhaustion.premises)),
        ]),
        metered.replay(&exhaustion).map(|_| ()),
        noble_contracts::companion::Refusal::ExhaustedReplay,
        &baseline,
    );
}
