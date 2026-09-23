pub(super) fn artifact(state: &mut super::Harness) {
    let super::Harness {
        core,
        checker,
        evidence,
        observed,
        ..
    } = state;
    let evidence = *evidence;
    let (guarded_contract, guarded, _) =
        super::checker::checked(core, checker, super::GUARDED, super::GUARDED_PROOF);
    let guarded_id = guarded.evidence.expect("conditional theorem");
    let guarded_subject = super::retained_subject(core, &guarded, &guarded_contract);
    let composed = noble_contracts::companion::Core::compose_subject(observed, &guarded_subject)
        .expect("plain composition");
    assert_eq!(
        core.evidence_guard_templates(guarded_id),
        Ok(vec![noble_contracts::companion::GuardTemplate::LtI64Max])
    );
    assert_eq!(
        core.derive_compose(evidence, guarded_id, &composed),
        Err(noble_contracts::companion::Refusal::UnresolvedImplication)
    );

    // Artifact correspondence checks actual independently accepted source;
    // equal endpoints and successful ordinary compilation are insufficient.
    let session = noble_contracts::source::Session::without_test_hosts();
    let good = session
        .prepare(b"[ 1 + ]", &[], super::LIMITS)
        .expect("closed subject source");
    assert!(core
        .bind_artifact(evidence, good.submission().expect("submission"))
        .is_ok());
    let wrong = session
        .prepare(b"[ 2 + ]", &[], super::LIMITS)
        .expect("same-interface source");
    assert_eq!(
        core.bind_artifact(evidence, wrong.submission().expect("submission")),
        Err(noble_contracts::companion::Refusal::CorrespondenceMismatch)
    );
    let executed = session
        .prepare(b"41 [ 1 + ] run", &[], super::LIMITS)
        .expect("ordinary computation");
    assert_eq!(
        core.bind_artifact(evidence, executed.submission().expect("submission")),
        Err(noble_contracts::companion::Refusal::CorrespondenceMismatch)
    );
}

pub(super) fn renewal(state: &mut super::Harness) {
    let changes = [
        ("host-contract", 1_u64),
        ("semantic-revision", 2),
        ("consumer-policy", 3),
        ("current-environment-fact", 4),
    ];
    let mut index = 0;
    while index < changes.len() {
        let (variant, value) = changes[index];
        renew_one(state, variant, value);
        index += 1;
    }
    pending_completion(state);
}

fn renew_one(state: &mut super::Harness, variant: &str, value: u64) {
    let (contract, current, baseline) = super::checker::checked(
        &mut state.core,
        &mut state.checker,
        super::INCREMENT,
        super::PROOF,
    );
    let evidence = current.evidence.expect("current theorem");
    let subject = super::retained_subject(&state.core, &current, &contract);
    mutate(&mut state.core, variant, value);
    super::control(
        &mut state.controls,
        ("CONTRACT-02", variant),
        crate::workflow::encoding::object([
            (
                "mutated_context_field",
                crate::workflow::encoding::string(variant),
            ),
            ("new_value", crate::workflow::encoding::Json::Number(value)),
            (
                "evidence",
                crate::workflow::encoding::Json::Number(u64::from(evidence.0)),
            ),
        ]),
        state.core.bind(evidence, &subject).map(|_| ()),
        noble_contracts::companion::Refusal::StaleContext,
        &baseline,
    );
    assert_eq!(
        state.core.release(evidence),
        Err(noble_contracts::companion::Refusal::StaleContext)
    );
    assert_eq!(
        state
            .core
            .correspond(evidence, current.contract.expect("old contract")),
        Err(noble_contracts::companion::Refusal::StaleContext)
    );
    if variant == "semantic-revision" {
        restore_semantics(state, &contract, evidence, &subject);
    }
    revalidated(state, evidence, &subject);
}

fn mutate(core: &mut noble_contracts::companion::Core, variant: &str, value: u64) {
    match variant {
        "host-contract" => core.set_host_contract(value),
        "semantic-revision" => core.set_semantic_revision(u32::try_from(value).expect("revision")),
        "consumer-policy" => core.set_policy(u32::try_from(value).expect("policy")),
        "current-environment-fact" => core.set_environment_fact(value),
        _ => unreachable!(),
    }
}

fn restore_semantics(
    state: &mut super::Harness,
    contract: &noble_contracts::Prepared,
    evidence: noble_contracts::companion::EvidenceId,
    subject: &noble_contracts::companion::Subject,
) {
    let checks_before = state.checker.reports.len();
    let unsupported = state
        .checker
        .admit(&mut state.core, contract, super::offer(super::PROOF));
    assert_eq!(
        unsupported.outcome,
        noble_contracts::companion::Outcome::Unsupported
    );
    assert_eq!(
        unsupported.refusals,
        [noble_contracts::companion::Refusal::UnsupportedSemanticRevision]
    );
    assert_eq!(state.checker.reports.len(), checks_before);
    super::annotate_last(
        &mut state.controls,
        "revalidation_while_unsupported",
        crate::workflow::encoding::string(unsupported.refusals[0].code()),
    );
    state
        .core
        .set_semantic_revision(noble_kernel::untrusted::SEMANTIC_REVISION);
    super::annotate_last(
        &mut state.controls,
        "restored_supported_revision",
        crate::workflow::encoding::Json::Number(u64::from(
            noble_kernel::untrusted::SEMANTIC_REVISION,
        )),
    );
    assert_eq!(
        state.core.bind(evidence, subject),
        Err(noble_contracts::companion::Refusal::StaleContext)
    );
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; revalidated obtains fresh genuine evidence through checked and records its real bind result, then asserts that previous evidence remains stale; receipt encoding needs no duplicate assertions."
)]
fn revalidated(
    state: &mut super::Harness,
    previous: noble_contracts::companion::EvidenceId,
    subject: &noble_contracts::companion::Subject,
) {
    let (_, renewed, _) = super::checker::checked(
        &mut state.core,
        &mut state.checker,
        super::INCREMENT,
        super::PROOF,
    );
    let evidence = renewed.evidence.expect("rechecked exact theorem");
    super::annotate_last(
        &mut state.controls,
        "revalidated",
        crate::workflow::encoding::Json::Bool(state.core.bind(evidence, subject).is_ok()),
    );
    super::annotate_last(
        &mut state.controls,
        "renewed_evidence",
        crate::workflow::encoding::Json::Number(u64::from(evidence.0)),
    );
    assert_eq!(
        state.core.bind(previous, subject),
        Err(noble_contracts::companion::Refusal::StaleContext)
    );
}

fn pending_completion(state: &mut super::Harness) {
    let mut core = noble_contracts::companion::Core::new(super::LIMITS);
    let contract = super::prepared(super::INCREMENT);
    state.checker.source = super::INCREMENT.to_vec();
    let request = core
        .begin_admission(&contract, super::offer(super::PROOF))
        .expect("pending real proof request");
    let checks_before = state.checker.reports.len();
    let observation = state.checker.check(&request);
    assert_eq!(
        state.checker.reports.len(),
        checks_before
            .checked_add(1)
            .expect("next checker report count")
    );
    let report = state
        .checker
        .last_report
        .as_ref()
        .expect("real checker report");
    assert_eq!(super::report_text(report, "outcome"), Some("proved"));
    assert_eq!(
        report.member("independent_recheck"),
        Some(&crate::core::report::Value::Bool(true))
    );
    core.set_host_contract(1);
    let stale = core.complete_admission(request, observation);
    assert_eq!(stale.evidence, None);
    assert_eq!(
        stale.refusals,
        [noble_contracts::companion::Refusal::StaleContext]
    );
}
