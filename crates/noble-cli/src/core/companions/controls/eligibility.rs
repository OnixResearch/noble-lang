pub(super) fn exercise(state: &mut super::Harness) {
    capability(state);
    ghost_ingress(&mut state.controls);
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; capability delegates the exact LiveCapabilityInEvidence assertion to control and separately asserts the checker-run count is unchanged, covering refusal before any external consumer call."
)]
fn capability(state: &mut super::Harness) {
    let super::Harness {
        core,
        checker,
        controls,
        increment,
        baseline,
        ..
    } = state;
    let capability = noble_contracts::companion::EvidenceOffer {
        payload: noble_contracts::companion::EvidencePayload::ServiceCapability(77),
        ..super::offer(&[])
    };
    let before_checks = checker.reports.len();
    let cap_result = checker.admit(core, increment, capability);
    super::control(
        controls,
        ("CONTRACT-12", "service-capability-in-pure-evidence"),
        crate::workflow::encoding::object([
            (
                "payload_kind",
                crate::workflow::encoding::string("ServiceCapability"),
            ),
            ("handle", crate::workflow::encoding::Json::Number(77)),
        ]),
        super::admission_result(&cap_result),
        noble_contracts::companion::Refusal::LiveCapabilityInEvidence,
        baseline,
    );
    assert_eq!(
        checker.reports.len(),
        before_checks,
        "capability refusal occurs before any external checker call"
    );
}

fn ghost_ingress(controls: &mut Vec<crate::workflow::encoding::Json>) {
    let cases = [
        ("resource-in-ghost-capture", include_bytes!("../../../../../../verification/mc2/contracts/ghost-resource.contract").as_slice(), noble_contracts::DiagnosticKind::Unsupported),
        ("runtime-value-from-erased-ghost", b"(contract 1 ghost (input) (output (n I64)) (params (n I64)) (program [ (param n) ]) (requires true) (ensures true))".as_slice(), noble_contracts::DiagnosticKind::Invalid),
    ];
    let mut index = 0;
    while index < cases.len() {
        let (variant, source, expected) = cases[index];
        rejected_ghost(controls, variant, source, expected);
        index += 1;
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; rejected_ghost requires bounded preparation to fail and asserts the exact diagnostic kind once before encoding the actual refusal; duplicated assertions would not strengthen either ghost-ingress control."
)]
fn rejected_ghost(
    controls: &mut Vec<crate::workflow::encoding::Json>,
    variant: &str,
    source: &[u8],
    expected: noble_contracts::DiagnosticKind,
) {
    let rejected =
        noble_contracts::prepare(source, super::LIMITS).expect_err("illegal ghost ingress refuses");
    assert_eq!(rejected.kind, expected);
    let diagnostic = match rejected.kind {
        noble_contracts::DiagnosticKind::Invalid => "invalid",
        noble_contracts::DiagnosticKind::Unsupported => "unsupported",
        noble_contracts::DiagnosticKind::Exhausted => "exhausted",
        noble_contracts::DiagnosticKind::Internal => "internal",
    };
    let input = crate::workflow::encoding::object([(
        "source",
        crate::workflow::encoding::string(std::str::from_utf8(source).expect("fixture")),
    )]);
    let observed = crate::workflow::encoding::object([
        ("diagnostic", crate::workflow::encoding::string(diagnostic)),
        (
            "message",
            crate::workflow::encoding::string(rejected.message),
        ),
        (
            "new_certified_status",
            crate::workflow::encoding::Json::Bool(false),
        ),
        (
            "resource_ownership_duplicated",
            crate::workflow::encoding::Json::Bool(false),
        ),
    ]);
    controls.push(crate::workflow::encoding::object([
        ("case", crate::workflow::encoding::string("CONTRACT-12")),
        ("variant", crate::workflow::encoding::string(variant)),
        ("input", input),
        (
            "execution_capability",
            crate::workflow::encoding::string("none"),
        ),
        ("observed", observed),
    ]));
}

pub(super) fn refutation(state: &mut super::Harness) {
    let super::Harness { core, checker, .. } = state;
    let refutation_source =
        include_bytes!("../../../../../../verification/mc1/monotonic.noble-contract");
    checker.source = refutation_source.to_vec();
    let refuted_contract = super::prepared(refutation_source);
    let refuted = checker.admit(
        core,
        &refuted_contract,
        noble_contracts::companion::EvidenceOffer {
            class: noble_contracts::companion::EvidenceClass::LeanRefutation,
            refutation: true,
            payload: noble_contracts::companion::EvidencePayload::Declaration(
                include_bytes!("../../../../../../verification/mc1/monotonic-refutation.lean")
                    .to_vec(),
            ),
        },
    );
    assert_eq!(
        refuted.outcome,
        noble_contracts::companion::Outcome::Disproved,
        "refutation checks: {:?}",
        checker.last_report
    );
    let refutation_id = refuted.evidence.expect("checked refutation");
    let refuted_subject = super::retained_subject(core, &refuted, &refuted_contract);
    assert_eq!(
        core.bind(refutation_id, &refuted_subject),
        Err(noble_contracts::companion::Refusal::WrongPremiseClass)
    );
    assert_eq!(
        core.release(refutation_id),
        Err(noble_contracts::companion::Refusal::WrongPremiseClass)
    );
    assert_eq!(
        core.instantiate(refutation_id, 7, &super::increment_subject(7)),
        Err(noble_contracts::companion::Refusal::WrongPremiseClass)
    );
    assert_eq!(
        core.derive_compose(refutation_id, refutation_id, &refuted_subject),
        Err(noble_contracts::companion::Refusal::WrongPremiseClass)
    );
    assert_eq!(
        core.replay(&super::derivation(
            noble_contracts::companion::RuleId::ProjectV1,
            &refuted,
            &refuted_subject,
            vec![refutation_id]
        )),
        Err(noble_contracts::companion::Refusal::WrongPremiseClass)
    );
}
