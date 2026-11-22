//! GUARD-CASE-02's substituted binding, run against the real Core (owner
//! decision 34): the CLI cannot mint a companion whose evidence belongs to
//! another contract, so this control presents the evidence admitted for
//! `requires (eq (in x) 6)` as the eq-5 contract's companion and records the
//! refusal before any guard template is selected or executed.

const EQ_FIVE: &[u8] =
    include_bytes!("../../../../../../verification/guard-cases/fixtures/eq-i64-literal-5.contract");
const EQ_SIX: &[u8] =
    include_bytes!("../../../../../../verification/guard-cases/fixtures/eq-i64-literal-6.contract");
const TRUE_PROOF: &[u8] =
    include_bytes!("../../../../../../verification/guard-cases/fixtures/increment-true.proof.lean");

#[test]
#[ignore = "requires NOBLE_GUARD_CONTROL_RECEIPT and NOBLE_MC2_CHECKER_BINARY"]
fn exercise() {
    let Some(destination) = std::env::var_os("NOBLE_GUARD_CONTROL_RECEIPT") else {
        panic!("GUARD controls require NOBLE_GUARD_CONTROL_RECEIPT; run with --ignored and the variable set");
    };
    let binary = std::env::var_os("NOBLE_MC2_CHECKER_BINARY")
        .expect("real noble checker binary is required");
    let destination = std::path::PathBuf::from(destination);
    let mut checker = super::checker::LeanConsumer {
        binary: binary.into(),
        directory: destination
            .parent()
            .expect("receipt directory")
            .join("lean-controls"),
        source: vec![],
        sequence: 0,
        reports: vec![],
        last_report: None,
    };
    let mut core = noble_contracts::companion::Core::new(super::LIMITS);
    let (five, five_admission, baseline) =
        super::checker::checked(&mut core, &mut checker, EQ_FIVE, TRUE_PROOF);
    let (_, six_admission, _) =
        super::checker::checked(&mut core, &mut checker, EQ_SIX, TRUE_PROOF);
    let five_contract = five_admission.contract.expect("eq-5 contract");
    let five_evidence = five_admission.evidence.expect("eq-5 evidence");
    let six_evidence = six_admission.evidence.expect("eq-6 evidence");
    // Each evidence selects its own literal; the substituted one would admit 6.
    assert_eq!(
        core.evidence_guard_templates(five_evidence),
        Ok(vec![noble_contracts::companion::GuardTemplate::EqI64Literal(5)])
    );
    assert_eq!(
        core.evidence_guard_templates(six_evidence),
        Ok(vec![noble_contracts::companion::GuardTemplate::EqI64Literal(6)])
    );
    let subject = super::retained_subject(&core, &five_admission, &five);
    assert!(core.bind(five_evidence, &subject).is_ok());
    // The invocation path's first check on a companion is correspond(evidence,
    // contract); the substituted pair fails it before template selection.
    assert_eq!(core.correspond(five_evidence, five_contract), Ok(()));
    let mut controls = Vec::new();
    super::control(
        &mut controls,
        ("GUARD-CASE-02", "substituted-eq-6-binding"),
        crate::workflow::encoding::object([
            ("contract", crate::workflow::encoding::string("eq-i64-literal-5")),
            ("evidence", crate::workflow::encoding::string("eq-i64-literal-6")),
            ("guard_executed", crate::workflow::encoding::Json::Bool(false)),
        ]),
        core.correspond(six_evidence, five_contract),
        noble_contracts::companion::Refusal::CorrespondenceMismatch,
        &baseline,
    );
    let receipt = crate::workflow::encoding::object([
        (
            "schema",
            crate::workflow::encoding::string("noble-guard-core-controls/v1"),
        ),
        ("controls", crate::workflow::encoding::Json::Array(controls)),
        (
            "checker_runs",
            crate::workflow::encoding::Json::Array(checker.reports),
        ),
    ]);
    std::fs::write(&destination, receipt.encode()).expect("write real core observations");
}
